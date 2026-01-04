
use std::{net::{IpAddr, UdpSocket}, process::Command, str::from_utf8, time::{ Duration, Instant}, usize};
use byteorder::{ByteOrder, LittleEndian};
use opus::{Channels, Decoder};
use log::{debug};
use log::{trace, error, info, warn};
use crate::{VBanSampleRates, VBanBitResolution,VBAN_STREAM_NAME_SIZE, PlayerState, AlsaSink, VBAN_PACKET_MAX_LEN_BYTES, VBanCodec, VBanProtocol, VBanHeader, VBAN_PACKET_HEADER_BYTES, VBAN_PACKET_COUNTER_BYTES, VBAN_SRLIST, VbanSink};


pub struct VbanRecipient {

    socket : UdpSocket,

    sample_rate : Option<VBanSampleRates>,

    num_channels : Option<u8>,

    /// Definition of bitwidth (16, 24, 32) and integer/float type
    sample_format : Option<VBanBitResolution>, 

    stream_name : Option<[u8;VBAN_STREAM_NAME_SIZE]>,

    nu_frame : u32,

    state : PlayerState,

    timer : Instant,

    sink : Option<AlsaSink>,

    sink_name : String,

    silence : u32,

    command : Option<Command>,

    decoder : Option<Decoder>
}

impl VbanRecipient {

    pub fn create(ip_addr : IpAddr, port: u16, stream_name : Option<String>, numch : Option<u8>, sample_rate : Option<VBanSampleRates>, sink_name : String, silence : Option<u32>) -> Option<Self> {

        let sn: Option<[u8; 16]> = match stream_name {
            None => None,
            Some(name) => {
                if name.len() > VBAN_STREAM_NAME_SIZE {
                    dbg!("Stream name exceeds the limit of {} characters", VBAN_STREAM_NAME_SIZE);
                    return None;
                }
                let mut sn: [u8; 16] = [0u8; 16];
                for (idx, b) in name.bytes().enumerate(){
                    if idx >= VBAN_STREAM_NAME_SIZE {
                        break;
                    }
                    sn[idx] = b;
                }
                Some(sn)
            }
        };
        
        let to_addr = (ip_addr, port);
        let result  = VbanRecipient{
            socket :  match UdpSocket::bind(to_addr){
                Ok(sock) => sock,
                Err(_) => {
                    dbg!("Could not create socket");
                    return None;
                },
            },
            
            sample_rate : sample_rate,
            
            num_channels : numch,
            
            sample_format : None,
            
            stream_name : sn,

            nu_frame : 0,
            
            state : PlayerState::Idle,

            timer : Instant::now(),

            sink : None,

            sink_name,

            silence : match silence {
                None => 0,
                Some(val) => val,
            },

            command : None,

            decoder : None
        };

        result.socket.set_read_timeout(Some(Duration::new(1, 0))).expect("Could not set timeout of socket");

        info!("VBAN recepipient ready. Waiting for incoming audio packets...");
        Some(result)
    }
    

    pub fn handle(&mut self){
        let mut buf :[u8; VBAN_PACKET_MAX_LEN_BYTES] = [0; VBAN_PACKET_MAX_LEN_BYTES];
        
        // close PCM after 2 seconds of not receiving any audio data
        if self.state == PlayerState::Playing && self.timer.elapsed().as_secs() > 2 {
            self.state = PlayerState::Idle;
            
            match &self.sink{
                None => error!("Something's wrong. Expected to find a pcm but it is unitialized."),
                Some(sink) => {
                    match sink.pcm.drain(){
                        Err(errno) => error!("Error while draining pcm: {errno}"),
                        Ok(()) => (),
                    }
                    match sink.pcm.drop(){
                        Err(errno) => error!("Error while closing pcm: {errno}"),
                        Ok(()) => debug!("Audio device released"),
                    }
                    self.sink = None;
                }
            }
            match &mut self.command {
                None => (),
                Some(cmd) => _ = cmd.arg("playback_stopped").output(),
            }
        }

        let packet = self.socket.recv_from(&mut buf);
        
        let size = match packet {
            Ok((size, _addr)) => {
                size
            },
            _ => return,
        };

        trace!("UDP packet len {} from {}", size, packet.unwrap().1);

        if buf[..4] == *b"VBAN" {
            
            let head : [u8; 28] = buf[0..28].try_into().unwrap();
            let head = VBanHeader::from(head);
            
            self.sample_format = Some(head.sample_format.into());
            
            let num_samples: u16 = head.num_samples as u16 + 1;
            if num_samples > crate::VBAN_SAMPLES_MAX_NB {
                debug!("Number of samples exceeds maximum of {} (found {}).", crate::VBAN_SAMPLES_MAX_NB, num_samples);
                return;
            }

            let bits_per_sample = crate::VBAN_BIT_RESOLUTION_SIZE[self.sample_format.unwrap() as usize];
            let codec = VBanCodec::from(head.sample_format);
            let protocol = VBanProtocol::from(head.sample_rate);
            let name_incoming : &str = from_utf8(&head.stream_name).unwrap();

            trace!("VBAN - #smp {}, bps {}, codec {}, name {}", num_samples, bits_per_sample, codec, name_incoming);
            
            if protocol != VBanProtocol::VbanProtocolAudio {
                debug!("Discarding packet with protocol {:?} because it is not supported.", protocol);
                return;
            }
            match codec {
                VBanCodec::VbanCodecPcm => (),
                VBanCodec::VbanCodecOpus(_) => (),
                _ => {
                    error!("Any codecs other than PCM and OPUS are not supported (found {:?}).", codec);
                    return;
                }

            }
            if bits_per_sample != 2{
                error!("Bitwidth other than 16 bits not supported (found {}).", bits_per_sample * 8);
                return;
            }
            
            let sr : VBanSampleRates  = head.sample_rate.into();

            if head.num_channels > ( crate::VBAN_CHANNELS_MAX_NB - 1) as u8 {
                debug!("Number of channels exceeds maximum of {}.", crate::VBAN_CHANNELS_MAX_NB);
                return;
            }
            self.num_channels = Some(head.num_channels + 1);

            match self.stream_name {
                None => (),
                Some(name) => {
                    if from_utf8(&name).unwrap() != name_incoming {
                        debug!("Discarding packet because stream names don't match (found {name_incoming}.");
                        return;
                    }
                }
            }

            let audio_data : Vec<u8> = Vec::from(&buf[VBAN_PACKET_HEADER_BYTES + VBAN_PACKET_COUNTER_BYTES..size]);
            let mut to_sink : Vec<i16>;
            let mut left : i16 = 0;
            let mut right : i16 = 0;

            match codec{
                VBanCodec::VbanCodecPcm => {
                    to_sink = vec![0; audio_data.len() / bits_per_sample as usize];

                    for (idx, _smp) in audio_data.iter().enumerate() {
                        if idx % 2 == 1 {
                            continue;
                        }

                        if idx == audio_data.len() - 1 {
                            break;
                        }

                        let amplitude_le = LittleEndian::read_i16(&audio_data[idx..idx+2]);

                        if idx % 4 == 0 {
                            if amplitude_le > left {
                                left = amplitude_le;
                            }
                        } else {
                            if amplitude_le > right {
                                right = amplitude_le;
                            }
                        }

                        to_sink[idx / 2] = amplitude_le;
                    }
                }

                VBanCodec::VbanCodecOpus(_) => {
                    if self.decoder.is_none(){

                        let opus_ch = match self.num_channels.unwrap() {
                            1 => Channels::Mono,
                            2 => Channels::Stereo,
                            _ => {
                                error!("Error: Opus cannot handle {} channels", self.num_channels.unwrap());
                                return;
                            }
                        };

                        self.decoder = match Decoder::new(sr.into(), opus_ch){
                            Ok(d) => Some(d),
                            Err(e) => {
                                error!("Error while trying to create an opus decoder: {e}");
                                return;
                            }
                        };
                    }

                    let dec = self.decoder.as_mut().unwrap();
                    let opus_num_samples = dec.get_nb_samples(&audio_data).unwrap(); // TODO: needs proper error handling

                    to_sink = vec![0; 2 * num_samples as usize];
                    dec.decode(&audio_data, &mut to_sink, false).unwrap();

                    for (idx, ampl) in to_sink.iter().enumerate(){
                        if idx % 2 == 0 {
                            if *ampl > left {
                                left = *ampl;
                            }
                        } else {
                            if *ampl > right {
                                right = *ampl;
                            }
                        }
                    }

                }

                _ => return // we've already caught that case above
            }

            self.timer = Instant::now();
            if self.state == PlayerState::Idle {
                match &self.sink {
                    Some(_sink) => error!("Something's wrong. Sink is Some() although it should be None"),
                    None => {
                        self.sample_rate = Some(sr);
                        self.sink = match AlsaSink::init(&self.sink_name, Some(self.num_channels() as u32), Some(self.sample_rate())){
                            None => {
                                warn!("Could not grab audio device");
                                return
                            },
                            Some(sink) => {
                                trace!("Successfully initialized ALSA device with {} channels at {} Hz", self.num_channels(), self.sample_rate());
                                Some(sink)
                            }
                        };

                        info!("Connected to stream {}: \nSR: {} \t Ch: {} \t BPS: {} \t Codec: {}\n", name_incoming, self.sample_rate(), self.num_channels(), self.bits_per_sample(), codec);

                        /* Push silence before the data */
                        let silence_buf = vec![0i16; (self.sample_rate() / 1000 * self.silence) as usize];
                        self.sink.as_mut().unwrap().write(&silence_buf);
                    }
                }
                match &mut self.command {
                    None => (),
                    Some(cmd) => _ = cmd.arg("playback_started").output(),
                }
                self.state = PlayerState::Playing;
            } else {
                if sr != self.sample_rate.unwrap(){
                    self.sample_rate = Some(sr);
                    let sink = self.sink.as_mut().unwrap();
                    let _ = sink.pcm.drain();
                    self.sink = Some(AlsaSink::init(&self.sink_name, Some(self.num_channels() as u32), Some(self.sample_rate())).expect("Could not create audio device with the required specs."));
                }
            }
            let sink = self.sink.as_mut().unwrap();
            sink.write(&to_sink);
            // println!("\x1B[1ALeft {:.4}, Right {:.4} (from {num_samples} samples)", (left as f32 / i16::MAX as f32), (right as f32 / i16::MAX as f32));
        } else{
            debug!("Got UDP packet that is not VBAN");
        }
    }


    // SETTER
    pub fn set_command(&mut self, cmd : Command){
        self.command = Some(cmd);
    }

    // GETTER
    fn sample_rate(&self) -> u32 {
        VBAN_SRLIST[self.sample_rate.unwrap() as usize]
    }

    fn bits_per_sample(&self) -> u8 {
        self.sample_format.unwrap() as u8 + 1
    }

    fn num_channels(&self) -> u8 {
        self.num_channels.unwrap() as u8
    }


}

