
use std::{net::{IpAddr, UdpSocket}, process::Command, str::from_utf8, time::{ Duration, Instant}, usize};
use byteorder::{ByteOrder, LittleEndian};
use opus::{Channels, Encoder};
use log::{debug};
use log::{error, info, trace, warn};
use crate::{AlsaSource, PlayerState, VBanBitResolution, VBanCodec, VBanHeader, VBanSampleRates, VbanSource, VBAN_HEADER_SIZE, VBAN_PACKET_COUNTER_BYTES, VBAN_PACKET_HEADER_BYTES, VBAN_PACKET_MAX_LEN_BYTES, VBAN_PACKET_MAX_SAMPLES, VBAN_STREAM_NAME_SIZE};

// OPUS
/// Number of samples per channel per opus packet, may be one of 120, 240, 480, 960, 1920, 2880
/// VBAN only allows a maximum of 256 samples per packet though
const OPUS_FRAME_SIZE : usize = 240; 
const OPUS_BITRATE : i32 = 320000;



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

    state : PlayerState,

    source : Option<AlsaSource>,

    /// Name of the audio system source
    source_name : String,

    command : Option<Command>,

    encoder : Option<Encoder>
}

impl VbanSender {

    pub fn create(peer : (IpAddr, u16), local_addr : (IpAddr, u16), stream_name : Option<String>, numch : u8, sample_rate : VBanSampleRates, format : VBanBitResolution, source_name : String, encoder : Option<VBanCodec>) -> Option<Self> {

        if format != VBanBitResolution::VbanBitfmt16Int {
            error!("Only 16 bit sample resolution is supported");
            return None;
        }

        if stream_name.clone().is_some_and(|n| n.len() > VBAN_STREAM_NAME_SIZE){
            warn!("Stream name exceeds limit of {} chars", VBAN_STREAM_NAME_SIZE);
            return None;
        }

        let mut name   = [0; 16];
        
        match stream_name {
            None => {
                let default_name = "Stream1";
                for (idx, ch) in default_name.as_bytes().iter().enumerate(){
                    name[idx] = *ch;
                }
            }
            Some(custom_name) => {
                for (idx, ch) in name.iter_mut().enumerate(){
                    *ch = custom_name.as_bytes()[idx];
                } 
            }
        }

        let enc = match encoder {
            None => None,
            Some(VBanCodec::VbanCodecPcm) => None,
            Some(VBanCodec::VbanCodecOpus) => {
                let ch = match numch {
                    1 => Channels::Mono,
                    2 => Channels::Stereo,
                    _ => {
                        error!("Encoder OPUS does not support {} channels!", numch);
                        return None
                    }
                };
                let mut e =  Encoder::new(sample_rate.into(), Channels::from(ch), opus::Application::Audio).expect("Could not create encoder!");
                e.set_bitrate(opus::Bitrate::Bits(OPUS_BITRATE)).expect("Could not set bitrate of encoder");
                Some(e)
            }
            _ => {
                error!("Requested codec is not implemented.");
                return None;
            }
        };
        
        let result = VbanSender {

            peer,

            socket : match UdpSocket::bind(local_addr){
                Ok(sock) => {
                    trace!("Successfully created socket");
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

            state : PlayerState::Idle,

            source : None,

            source_name,

            command : None,

            encoder : enc

        };

        Some(result)
    }

    pub fn handle(&mut self){
        let mut vban_packet :[u8; VBAN_PACKET_MAX_LEN_BYTES] = [0; VBAN_PACKET_MAX_LEN_BYTES];

        // this assumes stereo ... better would be to take as much samples as possible for our given num_channels
        let mut audio_in : Vec<i16> = vec![0; VBAN_PACKET_MAX_SAMPLES * 2];

        if self.state == PlayerState::Idle {

            self.source = match AlsaSource::init(&self.source_name, self.num_channels as u32, self.sample_rate.into()){
                None => {
                    error!("Could not create alsa source");
                    return;
                }
                Some(s) => {
                    self.state = PlayerState::Playing;
                    Some(s)
                }
            }

        }

        let source = self.source.as_mut().unwrap();

        match self.encoder {
            None => (),
            Some(_) => audio_in.resize(OPUS_FRAME_SIZE*self.num_channels as usize, 0),
        }

        source.read(&mut audio_in);

        let mut encoded = vec![0u8; audio_in.len() * 2];

        match self.encoder.as_mut() {
            None => {
                for (idx, smp) in audio_in.iter().enumerate(){
                    LittleEndian::write_i16(&mut encoded[2* idx..], *smp);
                }
            },
            Some(enc) => {
                let bytes = match enc.encode(&audio_in, &mut encoded){
                    Ok(size) => size,
                    Err(_e) => 0
                };
                encoded.resize(bytes, 0); // this should hopefully shrink the vector
                debug!("Size of encoded is {bytes} after encoding");
            }
        }

        let num_samples = ((audio_in.len() / self.num_channels as usize) - 1) as u8;
        debug!("num_Samples={num_samples}");

        let mut format= self.sample_format as u8;
        match self.encoder{
            None => (),
            Some(_) => {
                format |= VBanCodec::VbanCodecOpus as u8;
            }
        }

        let hdr = VBanHeader {
            preamble : [b'V', b'B', b'A', b'N'],
            sample_rate : self.sample_rate.into(),
            num_samples : num_samples,
            num_channels : self.num_channels -1 , // 0 means one channel in VBAN
            sample_format : format,
            stream_name : self.name,
            nu_frame : self.nu_frame
        };

        let hdr : [u8; VBAN_PACKET_HEADER_BYTES+VBAN_PACKET_COUNTER_BYTES] = hdr.into();

        vban_packet[..VBAN_HEADER_SIZE+VBAN_PACKET_COUNTER_BYTES].copy_from_slice(&hdr);

        if hdr.len() + encoded.len() > VBAN_PACKET_MAX_LEN_BYTES {
            error!("Constructed VBAN packet would exceed the limit of {} bytes.", VBAN_PACKET_MAX_LEN_BYTES);
            return;
        }

        debug!("Packet has an effective length of {} bytes", hdr.len() + encoded.len());

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
