
use std::{net::{IpAddr, UdpSocket}, process::Command, usize};
use byteorder::{ByteOrder, LittleEndian};
use opus::{Channels, Encoder};
use log::{error, info, trace};
use crate::{PipewireSource, VBanBitResolution, VBanCodec, VBanHeader, VBanSampleRates, VbanSource, VBAN_HEADER_SIZE, VBAN_PACKET_COUNTER_BYTES, VBAN_PACKET_HEADER_BYTES, VBAN_PACKET_MAX_LEN_BYTES, VBAN_PACKET_MAX_SAMPLES, VBAN_STREAM_NAME_SIZE, OPUS_BITRATE, OPUS_FRAME_SIZE};


// ****************************************
//              VBAN SENDER
// ****************************************
pub struct VbanSender {

    peer : (IpAddr, u16),

    socket : UdpSocket,

    sample_rate : VBanSampleRates,

    num_channels : u8, // 1 = one channel, unlike in the VBAN header, where 0 = one channel

    /// Definition of codec, bitwidth (16, 24, 32) and integer/float type
    sample_format : VBanBitResolution, 

    /// Stream name
    name : [u8; 16],

    nu_frame : u32,

    source : PipewireSource,

    command : Option<Command>,

    encoder : VBanCodec
}

impl VbanSender {

    /// Create a VbanSender object. 
    /// 
    /// # Arguments
    /// 
    /// * `peer` - (IpAddr, u16) - IP address and port of the receiver
    /// * `local_addr` - (IpAddr, u16) - Local IP address and port to bind to
    /// * `stream_name` - Option<String> - Name of the stream (max 16 characters)
    /// * `numch` - u8 - Number of channels (1-255)
    /// * `sample_rate` - VBanSampleRates - Sample rate of the audio stream
    /// * `format` - VBanBitResolution - Bit resolution and type of the audio
    /// * `source_name` - String - Name of the audio source (Pipewire target application or ALSA device)
    /// * `encoder` - Option<VBanCodec> - Optional codec to use (Opus or PCM)
    /// 
    /// # Returns 
    /// `Some(VbanSender)` if successful, `None` otherwise.
    /// 
    pub fn create(peer : (IpAddr, u16), local_addr : (IpAddr, u16), stream_name : String, numch : u8, sample_rate : VBanSampleRates, format : VBanBitResolution, source_name : String, encoder : u8) -> Option<Self> {

        if format != VBanBitResolution::VbanBitfmt16Int {
            error!("Only 16 bit sample resolution is supported");
            return None;
        }

        if stream_name.len() > VBAN_STREAM_NAME_SIZE {
            error!("Stream name exceeds limit of {} chars", VBAN_STREAM_NAME_SIZE);
            return None;
        }

        let mut name   = [0; 16];
        name[..stream_name.len()].copy_from_slice(stream_name.as_bytes());

        let enc = match VBanCodec::from(encoder) {
            VBanCodec::VbanCodecPcm => {
                VBanCodec::VbanCodecPcm
            }
            VBanCodec::VbanCodecOpus(None) => {
                let ch = match numch {
                    1 => Channels::Mono,
                    2 => Channels::Stereo,
                    _ => {
                        error!("Encoder OPUS does not support {} channels!", numch);
                        return None
                    }
                };
                let sr = match sample_rate {
                    VBanSampleRates::SampleRate12000Hz => 12000,
                    VBanSampleRates::SampleRate24000Hz => 24000,
                    VBanSampleRates::SampleRate48000Hz => 48000,
                    _ => {
                        error!("Encoder OPUS does not support sample rate {}!", sample_rate);
                        return None
                    }
                };
                let mut e =  Encoder::new(sr, Channels::from(ch), opus::Application::Audio).expect("Could not create encoder!");
                e.set_bitrate(opus::Bitrate::Bits(OPUS_BITRATE)).expect("Could not set bitrate of encoder");
                VBanCodec::VbanCodecOpus(Some(e))
            }
            VBanCodec::VbanCodecOpus(Some(e)) => VBanCodec::VbanCodecOpus(Some(e)),
            _ => {
                error!("Codec not supported");
                return None;
            }
        };

        let source = match PipewireSource::init(numch as u32, sample_rate.into(), Some(source_name.clone())){
            None => {
                error!("Could not create audio source");
                return None;
            }
            Some(s) => s
        };

        let result = VbanSender {

            peer,

            socket : match UdpSocket::bind(local_addr){
                Ok(sock) => {
                    trace!("Successfully created socket on {}:{}", local_addr.0, local_addr.1);
                    sock
                },
                Err(_) => {
                    error!("Could not create udp socket");
                    return None
                }
            },

            sample_rate : sample_rate,

            num_channels : numch,

            sample_format : format, 

            name : name,

            nu_frame : 0,

            source : source,

            command : None,

            encoder : enc

        };

        info!("Starting stream '{}' -  SR: {}, Ch: {}, Encoder: {}", std::str::from_utf8(&result.name).unwrap_or(""), result.sample_rate, result.num_channels, result.encoder);

        Some(result)
    }


    /// Handle one iteration of reading from source, composing a VBAN packet and sending via UDP.
    pub fn handle(&mut self){
        let mut vban_packet :[u8; VBAN_PACKET_MAX_LEN_BYTES] = [0; VBAN_PACKET_MAX_LEN_BYTES];

        // this assumes stereo ... better would be to take as much samples as possible for our given num_channels
        let mut audio_in : Vec<i16> = vec![0; VBAN_PACKET_MAX_SAMPLES * 2];

        match self.encoder {
            VBanCodec::VbanCodecPcm => (),
            VBanCodec::VbanCodecOpus(_) => audio_in.resize(OPUS_FRAME_SIZE*self.num_channels as usize, 0),
            _ => panic!("Unsupported codec in VbanSender struct")
        }

        self.source.read(&mut audio_in);

        let mut encoded = vec![0u8; audio_in.len() * 2];

        match self.encoder {
            VBanCodec::VbanCodecPcm => {
                for (idx, smp) in audio_in.iter().enumerate(){
                    LittleEndian::write_i16(&mut encoded[2* idx..], *smp);
                }
            },
            VBanCodec::VbanCodecOpus(ref mut enc) => {
                let bytes = match enc.as_mut().unwrap().encode(&audio_in, &mut encoded){
                    Ok(size) => size,
                    Err(_e) => 0
                };
                encoded.resize(bytes, 0); // this should hopefully shrink the vector
                trace!("OPUS compression: {} => {bytes} bytes", audio_in.len() * 2);
            },
            _ => panic!("Unsupported Codec in VbanSender struct")
        }

        let num_samples = audio_in.len() / self.num_channels as usize;
        trace!("Samples in packet: {}, audio_in len: {}, ch: {}", num_samples, audio_in.len(), self.num_channels);

        let mut format= self.sample_format as u8;
        match self.encoder{
            VBanCodec::VbanCodecPcm => (),
            VBanCodec::VbanCodecOpus(_) => format |= <VBanCodec as Into<u8>>::into(VBanCodec::VbanCodecOpus(None)),
            _ => ()
        }

        let hdr = VBanHeader {
            preamble : [b'V', b'B', b'A', b'N'],
            sample_rate : self.sample_rate.into(),
            num_samples : (num_samples - 1) as u8,
            num_channels : self.num_channels -1 , // 0 means one channel in VBAN
            sample_format : format,
            stream_name : self.name,
            nu_frame : self.nu_frame
        };

        trace!("Composing packet with nu_frame: {}", hdr.nu_frame);

        let hdr : [u8; VBAN_PACKET_HEADER_BYTES+VBAN_PACKET_COUNTER_BYTES] = hdr.into();

        vban_packet[..VBAN_HEADER_SIZE+VBAN_PACKET_COUNTER_BYTES].copy_from_slice(&hdr);

        if hdr.len() + encoded.len() > VBAN_PACKET_MAX_LEN_BYTES {
            error!("Constructed VBAN packet would exceed the limit of {} bytes.", VBAN_PACKET_MAX_LEN_BYTES);
            return;
        }

        trace!("Packet has an effective length of {} bytes", hdr.len() + encoded.len());

        let vban_data = &mut vban_packet[VBAN_PACKET_HEADER_BYTES+VBAN_PACKET_COUNTER_BYTES..];
        vban_data[..encoded.len()].copy_from_slice(&encoded);

        match self.socket.connect(self.peer){
            Ok(()) => (),
            Err(e) => error!("Could not connect to peer: {e}")
        }

        match self.socket.send(&vban_packet[..hdr.len()+encoded.len()]){
            Ok(bytes) => trace!("Successfully sent {bytes} bytes via socket"),
            Err(e) => error!("Error while sending data via socket: {e}")
        }

        self.nu_frame += 1;
    }


}
