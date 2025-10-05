//! VBAN specific definitions and useful implementations for the related libraries and binaries are conteined in this crate.


use core::{panic};
#[cfg(feature = "pipewire")]
use std::thread::JoinHandle;
use alsa::{pcm::*, ValueOr};
use alsa::Direction;
use byteorder::{ByteOrder, LittleEndian};
use log::{debug};
use log::{error, trace, warn};

#[cfg(feature = "pipewire")]
use pipewire::{stream::Stream, main_loop::MainLoop, properties::properties, context::Context, spa::{self, param::audio::AudioFormat}, spa::sys::{spa_format_audio_raw_build}};
use std::sync::mpsc::{channel, Receiver, Sender};

pub mod vban_recipient;
pub mod vban_sender;


const VBAN_HEADER_SIZE : usize = 4 + 1 + 1 + 1 + 1 + 16;
const VBAN_STREAM_NAME_SIZE : usize = 16;
const VBAN_PROTOCOL_MAX_SIZE : usize = 1464;
const VBAN_DATA_MAX_SIZE : usize = VBAN_PROTOCOL_MAX_SIZE - VBAN_HEADER_SIZE - VBAN_PACKET_COUNTER_BYTES;
const VBAN_CHANNELS_MAX_NB : usize = 256;
const VBAN_SAMPLES_MAX_NB : usize = 256;


const VBAN_PACKET_NUM_SAMPLES : usize = 256;  
const VBAN_PACKET_MAX_SAMPLES : usize = 256;
const VBAN_PACKET_HEADER_BYTES : usize = 24;  
const VBAN_PACKET_COUNTER_BYTES : usize = 4;  
const VBAN_PACKET_MAX_LEN_BYTES : usize = VBAN_PACKET_HEADER_BYTES + VBAN_PACKET_COUNTER_BYTES + VBAN_DATA_MAX_SIZE;


// ****************************************
//              VBAN Header
// ****************************************
struct VBanHeader {
    preamble : [u8; 4],
    sample_rate : u8,
    num_samples : u8,

    // number of channels, where 0 = one channel
    num_channels : u8,
    sample_format : u8,
    stream_name : [u8;16],
    nu_frame : u32
}

impl From<[u8; 28]> for VBanHeader {
    fn from (item: [u8; 28]) -> Self {

        // let frame_count : u32 = item[24] as u32 + (item[25] as u32) << 8 + (item[26] as u32) << 16 + (item[27] as u32) << 24;
        let frame_count  = 0;

        Self {
            preamble : item[0..4].try_into().unwrap(),
            sample_rate : item[4],
            num_samples : item[5],
            num_channels : item[6],
            sample_format : item[7],
            stream_name : [item[8], item[9], item[10], item[11], item[12], item[13], item[14], item[15], item[16], item[17], item[18], item[19], item[20], item[21], item[22], item[23]],
            nu_frame : frame_count
        }
    }
}

impl Into<[u8; VBAN_HEADER_SIZE+VBAN_PACKET_COUNTER_BYTES]> for VBanHeader {
    fn into(self) -> [u8; VBAN_HEADER_SIZE+VBAN_PACKET_COUNTER_BYTES] {
        let mut result = [0; VBAN_HEADER_SIZE+VBAN_PACKET_COUNTER_BYTES];

        result[..4].copy_from_slice(&self.preamble);
        result[4] = self.sample_rate;
        result[5] = self.num_samples;
        result[6] = self.num_channels;
        result[7] = self.sample_format;
        result[8..24].copy_from_slice(&self.stream_name);
        LittleEndian::write_u32(&mut result[24..28], self.nu_frame);

        result
    }
}

// VBan struct missing

const VBAN_SR_MASK : u8 = 0x1F;
const VBAN_SRLIST : [u32; 21] = [
    6000, 12000, 24000, 48000, 96000, 192000, 384000,
    8000, 16000, 32000, 64000, 128000, 256000, 512000,
    11025, 22050, 44100, 88200, 176400, 352800, 705600
];

// ****************************************
//           VBAN Samples Rates
// ****************************************

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum VBanSampleRates {
    SampleRate6000Hz,
    SampleRate12000Hz,
    SampleRate24000Hz,
    SampleRate48000Hz,
    SampleRate96000Hz,
    SampleRate192000Hz,
    SampleRate384000Hz,
    SampleRate8000Hz,
    SampleRate16000Hz,
    SampleRate32000Hz,
    SampleRate64000Hz,
    SampleRate128000Hz,
    SampleRate256000Hz,
    SampleRate512000Hz,
    SampleRate11025Hz,
    SampleRate22050Hz,
    SampleRate44100Hz,
    SampleRate88200Hz,
    SampleRate176400Hz,
    SampleRate352800Hz,
    SampleRate705600Hz,
    SampleRateNotSupported
}

impl std::fmt::Display for VBanSampleRates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VBanSampleRates::SampleRate6000Hz => write!(f, "{} Hz", 6000), 
            VBanSampleRates::SampleRate12000Hz => write!(f, "{} Hz", 12000), 
            VBanSampleRates::SampleRate24000Hz => write!(f, "{} Hz", 24000), 
            VBanSampleRates::SampleRate48000Hz => write!(f, "{} Hz", 48000), 
            VBanSampleRates::SampleRate96000Hz => write!(f, "{} Hz", 96000), 
            VBanSampleRates::SampleRate192000Hz => write!(f, "{} Hz", 192000), 
            VBanSampleRates::SampleRate384000Hz => write!(f, "{} Hz", 384000), 
            VBanSampleRates::SampleRate8000Hz => write!(f, "{} Hz", 8000), 
            VBanSampleRates::SampleRate16000Hz => write!(f, "{} Hz", 16000), 
            VBanSampleRates::SampleRate32000Hz => write!(f, "{} Hz", 32000), 
            VBanSampleRates::SampleRate64000Hz => write!(f, "{} Hz", 64000), 
            VBanSampleRates::SampleRate128000Hz => write!(f, "{} Hz", 128000), 
            VBanSampleRates::SampleRate256000Hz => write!(f, "{} Hz", 256000), 
            VBanSampleRates::SampleRate512000Hz => write!(f, "{} Hz", 512000), 
            VBanSampleRates::SampleRate11025Hz => write!(f, "{} Hz", 11025), 
            VBanSampleRates::SampleRate22050Hz => write!(f, "{} Hz", 22050), 
            VBanSampleRates::SampleRate44100Hz => write!(f, "{} Hz", 44100), 
            VBanSampleRates::SampleRate88200Hz => write!(f, "{} Hz", 88200), 
            VBanSampleRates::SampleRate176400Hz => write!(f, "{} Hz", 176400), 
            VBanSampleRates::SampleRate352800Hz => write!(f, "{} Hz", 352800), 
            VBanSampleRates::SampleRate705600Hz => write!(f, "{} Hz", 705600),
            VBanSampleRates::SampleRateNotSupported => write!(f, "Not supported"),
        }
    }
}

impl From<u8> for VBanSampleRates {
    fn from(item : u8) -> Self{
        match item & VBAN_SR_MASK {
            0 => VBanSampleRates::SampleRate6000Hz,
            1 => VBanSampleRates::SampleRate12000Hz,
            2 => VBanSampleRates::SampleRate24000Hz,
            3 => VBanSampleRates::SampleRate48000Hz,
            4 => VBanSampleRates::SampleRate96000Hz,
            5 => VBanSampleRates::SampleRate192000Hz,
            6 => VBanSampleRates::SampleRate384000Hz,
            7 => VBanSampleRates::SampleRate8000Hz,
            8 => VBanSampleRates::SampleRate16000Hz,
            9 => VBanSampleRates::SampleRate32000Hz,
            10 => VBanSampleRates::SampleRate64000Hz,
            11 => VBanSampleRates::SampleRate128000Hz,
            12 => VBanSampleRates::SampleRate256000Hz,
            13 => VBanSampleRates::SampleRate512000Hz,
            14 => VBanSampleRates::SampleRate11025Hz,
            15 => VBanSampleRates::SampleRate22050Hz,
            16 => VBanSampleRates::SampleRate44100Hz,
            17 => VBanSampleRates::SampleRate88200Hz,
            18 => VBanSampleRates::SampleRate176400Hz,
            19 => VBanSampleRates::SampleRate352800Hz,
            20 => VBanSampleRates::SampleRate705600Hz,
            _ => panic!("Invalid value for enum VBanSampleRates ({:b})", item)
        }
    }
}

impl Into<u8> for VBanSampleRates {
    fn into(self) -> u8 {
        match self {
            VBanSampleRates::SampleRate6000Hz => 0,
            VBanSampleRates::SampleRate12000Hz => 1,
            VBanSampleRates::SampleRate24000Hz => 2,
            VBanSampleRates::SampleRate48000Hz => 3,
            VBanSampleRates::SampleRate96000Hz => 4,
            VBanSampleRates::SampleRate192000Hz => 5,
            VBanSampleRates::SampleRate384000Hz => 6,
            VBanSampleRates::SampleRate8000Hz => 7,
            VBanSampleRates::SampleRate16000Hz => 8,
            VBanSampleRates::SampleRate32000Hz => 9,
            VBanSampleRates::SampleRate64000Hz => 10,
            VBanSampleRates::SampleRate128000Hz => 11,
            VBanSampleRates::SampleRate256000Hz => 12,
            VBanSampleRates::SampleRate512000Hz => 13,
            VBanSampleRates::SampleRate11025Hz => 14,
            VBanSampleRates::SampleRate22050Hz => 15,
            VBanSampleRates::SampleRate44100Hz => 16,
            VBanSampleRates::SampleRate88200Hz => 17,
            VBanSampleRates::SampleRate176400Hz => 18,
            VBanSampleRates::SampleRate352800Hz => 19,
            VBanSampleRates::SampleRate705600Hz => 20,
            VBanSampleRates::SampleRateNotSupported => panic!("Sample rate not supported")
        }
    }
}


impl Into<u32> for VBanSampleRates {
    fn into(self) -> u32 {
        match self {
            VBanSampleRates::SampleRate6000Hz => 6000,
            VBanSampleRates::SampleRate12000Hz => 12000,
            VBanSampleRates::SampleRate24000Hz => 24000,
            VBanSampleRates::SampleRate48000Hz => 48000,
            VBanSampleRates::SampleRate96000Hz => 96000,
            VBanSampleRates::SampleRate192000Hz => 192000,
            VBanSampleRates::SampleRate384000Hz => 384000,
            VBanSampleRates::SampleRate8000Hz => 8000,
            VBanSampleRates::SampleRate16000Hz => 16000,
            VBanSampleRates::SampleRate32000Hz => 32000,
            VBanSampleRates::SampleRate64000Hz => 64000,
            VBanSampleRates::SampleRate128000Hz => 128000,
            VBanSampleRates::SampleRate256000Hz => 256000,
            VBanSampleRates::SampleRate512000Hz => 512000,
            VBanSampleRates::SampleRate11025Hz => 11025,
            VBanSampleRates::SampleRate22050Hz => 22050,
            VBanSampleRates::SampleRate44100Hz => 44100,
            VBanSampleRates::SampleRate88200Hz => 88200,
            VBanSampleRates::SampleRate176400Hz => 176400,
            VBanSampleRates::SampleRate352800Hz => 352800,
            VBanSampleRates::SampleRate705600Hz => 705600,
            VBanSampleRates::SampleRateNotSupported => 0
        }
    }
}

impl From<u32> for VBanSampleRates {
    fn from(value: u32) -> Self {
        match value {
            6000 => VBanSampleRates::SampleRate6000Hz,
            12000 => VBanSampleRates::SampleRate12000Hz,
            24000 => VBanSampleRates::SampleRate24000Hz,
            48000 => VBanSampleRates::SampleRate48000Hz,
            96000 => VBanSampleRates::SampleRate96000Hz,
            192000 => VBanSampleRates::SampleRate192000Hz,
            384000 => VBanSampleRates::SampleRate384000Hz,
            8000 => VBanSampleRates::SampleRate8000Hz,
            16000 => VBanSampleRates::SampleRate16000Hz,
            32000 => VBanSampleRates::SampleRate32000Hz,
            64000 => VBanSampleRates::SampleRate64000Hz,
            128000 => VBanSampleRates::SampleRate128000Hz,
            256000 => VBanSampleRates::SampleRate256000Hz,
            512000 => VBanSampleRates::SampleRate512000Hz,
            11025 => VBanSampleRates::SampleRate11025Hz,
            22050 => VBanSampleRates::SampleRate22050Hz,
            44100 => VBanSampleRates::SampleRate44100Hz,
            88200 => VBanSampleRates::SampleRate88200Hz,
            176400 => VBanSampleRates::SampleRate176400Hz,
            352800 => VBanSampleRates::SampleRate352800Hz,
            705600 => VBanSampleRates::SampleRate705600Hz,
            _ => VBanSampleRates::SampleRateNotSupported
        }
    }
}


const VBAN_PROTOCOL_MASK : u8 = 0xE0;


// ****************************************
//             VBAN Protocol
// ****************************************
#[derive(Debug, PartialEq)]
enum VBanProtocol {
    VbanProtocolAudio         =   0x00,
    VbanProtocolSerial        =   0x20,
    VbanProtocolTxt           =   0x40,
    VbanProtocolService      =   0x60,
    VbanProtocolUndefined1   =   0x80,
    VbanProtocolUndefined2   =   0xA0,
    VbanProtocolUndefined3   =   0xC0,
    VbanProtocolUndefined4   =   0xE0
}

impl From<u8> for VBanProtocol {

    fn from(value: u8) -> Self {
        match value & VBAN_PROTOCOL_MASK {
            0x00 => VBanProtocol::VbanProtocolAudio,
            0x20 => VBanProtocol::VbanProtocolSerial,
            0x40 => VBanProtocol::VbanProtocolTxt,
            0x60 => VBanProtocol::VbanProtocolService,
            0x80 => VBanProtocol::VbanProtocolUndefined1,
            0xA0 => VBanProtocol::VbanProtocolUndefined2,
            0xC0 => VBanProtocol::VbanProtocolUndefined3,
            0xE0 => VBanProtocol::VbanProtocolUndefined4,
            _ => panic!("Invalid value for enum VBanProtocol ({:x})", value & VBAN_PROTOCOL_MASK),
        }
    }
}



// ****************************************
//            VBAN Bit Resolution
// ****************************************
const VBAN_BIT_RESOLUTION_MASK : u8 = 0x07;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VBanBitResolution {
    VbanBitfmt8Int = 0,
    VbanBitfmt16Int,
    VbanBitfmt24Int,
    VbanBitfmt32Int,
    VbanBitfmt32Float,
    VbanBitfmt64Float,
    VbanBitfmt12Int,
    VbanBitfmt10Int,
    VbanBitResolutionMax
}

impl From<u8> for VBanBitResolution {
    fn from(item : u8) -> Self {
        match  item & VBAN_BIT_RESOLUTION_MASK  {
            0 => VBanBitResolution::VbanBitfmt8Int,
            1 => VBanBitResolution::VbanBitfmt16Int,
            2 => VBanBitResolution::VbanBitfmt24Int,
            3 => VBanBitResolution::VbanBitfmt32Int,
            4 => VBanBitResolution::VbanBitfmt32Float,
            5 => VBanBitResolution::VbanBitfmt64Float,
            6 => VBanBitResolution::VbanBitfmt12Int,
            7 => VBanBitResolution::VbanBitfmt10Int,
            8 => VBanBitResolution::VbanBitResolutionMax,
            _ => panic!("Invalid value for enum VBanBitResolution ({item})"),
        }
    }
}

impl Into<u8> for VBanBitResolution {
    fn into(self) -> u8 {
        match self {
            VBanBitResolution::VbanBitfmt8Int => 0,
            VBanBitResolution::VbanBitfmt16Int => 1,
            VBanBitResolution::VbanBitfmt24Int => 2,
            VBanBitResolution::VbanBitfmt32Int => 3,
            VBanBitResolution::VbanBitfmt32Float => 4,
            VBanBitResolution::VbanBitfmt64Float => 5,
            VBanBitResolution::VbanBitfmt12Int => 6,
            VBanBitResolution::VbanBitfmt10Int => 7,
            VBanBitResolution::VbanBitResolutionMax => 8,
        }
    }
}

const VBAN_BIT_RESOLUTION_SIZE : [u8; 6] = [ 1, 2, 3, 4, 4, 8, ];



// ****************************************
//              VBAN Codec
// ****************************************

const _VBAN_RESERVED_MASK : u8 = 0x08;
const VBAN_CODEC_MASK : u8 = 0xF0;

#[derive(Debug)]
pub enum VBanCodec {
    VbanCodecPcm,
    VbanCodecVbca,
    VbanCodecVbcv,
    VbanCodecUndefined3,
    VbanCodecUndefined4,
    VbanCodecUndefined5,
    VbanCodecUndefined6,
    VbanCodecUndefined7,
    VbanCodecUndefined8,
    VbanCodecUndefined9,
    VbanCodecUndefined10,
    VbanCodecUndefined11,
    VbanCodecOpus(Option<opus::Encoder>),
    VbanCodecUndefined13,
    VbanCodecUndefined14,
    VbanCodecUser 
}

impl std::fmt::Display for VBanCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VBanCodec::VbanCodecPcm => write!(f, "PCM"),
            VBanCodec::VbanCodecOpus(_) => write!(f, "Opus") ,
            _ => write!(f, "Undefined")
        }
    }
}

impl From<u8> for VBanCodec {
    fn from(value: u8) -> Self {
        match value & VBAN_CODEC_MASK {
            0x00 => VBanCodec::VbanCodecPcm,
            0x10 => VBanCodec::VbanCodecVbca,
            0x20 => VBanCodec::VbanCodecVbcv,
            0x30 => VBanCodec::VbanCodecUndefined3,
            0x40 => VBanCodec::VbanCodecUndefined4,
            0x50 => VBanCodec::VbanCodecUndefined5,
            0x60 => VBanCodec::VbanCodecUndefined6,
            0x70 => VBanCodec::VbanCodecUndefined7,
            0x80 => VBanCodec::VbanCodecUndefined8,
            0x90 => VBanCodec::VbanCodecUndefined9,
            0xA0 => VBanCodec::VbanCodecUndefined10,
            0xB0 => VBanCodec::VbanCodecUndefined11,
            0xC0 => VBanCodec::VbanCodecOpus(None),
            0xD0 => VBanCodec::VbanCodecUndefined13,
            0xE0 => VBanCodec::VbanCodecUndefined14,
            0xF0 => VBanCodec::VbanCodecUser,
            _ => VBanCodec::VbanCodecUser
        }
    }
}

impl Into<u8> for VBanCodec {
    fn into(self) -> u8 {
        match self {
            VBanCodec::VbanCodecPcm => 0x00,
            VBanCodec::VbanCodecVbca => 0x10,
            VBanCodec::VbanCodecVbcv => 0x20,
            VBanCodec::VbanCodecUndefined3 => 0x30,
            VBanCodec::VbanCodecUndefined4 => 0x40,
            VBanCodec::VbanCodecUndefined5 => 0x50,
            VBanCodec::VbanCodecUndefined6 => 0x60,
            VBanCodec::VbanCodecUndefined7 => 0x70,
            VBanCodec::VbanCodecUndefined8 => 0x80,
            VBanCodec::VbanCodecUndefined9 => 0x90,
            VBanCodec::VbanCodecUndefined10 => 0xA0,
            VBanCodec::VbanCodecUndefined11 => 0xB0,
            VBanCodec::VbanCodecOpus(_) => 0xC0,
            VBanCodec::VbanCodecUndefined13 => 0xD0,
            VBanCodec::VbanCodecUndefined14 => 0xE0,
            VBanCodec::VbanCodecUser => 0xF0
        }
    }
}

#[derive (PartialEq)]
enum PlayerState {
    Idle,
    Playing,
}





// ****************************************
//             VBAN SINK 
// ****************************************
pub trait VbanSink {
    fn write(&self, buf : &[i16]);
}

// ****************************************
//             ALSA SINK 
// ****************************************

pub struct AlsaSink {
    pcm : PCM,
}

impl AlsaSink {

    pub fn init(device : &str, num_channels : Option<u32>, sample_rate : Option<u32>) -> Option<Self> {

        let sink = Self {
            pcm : {
                PCM::new(device, Direction::Playback, false).expect("Could not create PCM.")
            },
        };

        let num_channels = match num_channels {
            None => {2},
            Some(ch) => ch,
        };
        let rate = match sample_rate {
            None => 44100,
            Some(r) => r,
        };

        {
            let hwp = HwParams::any(&sink.pcm).expect("Could not get hwp.");

            hwp.set_channels(num_channels).expect("Could not set channel number.");
            hwp.set_rate(rate, ValueOr::Nearest).expect("Could not set sample rate.");
            hwp.set_format(Format::s16()).expect("Could not set sample format.");
            hwp.set_access(Access::RWInterleaved).expect("Could not set access.");
            sink.pcm.hw_params(&hwp).expect("Could not attach hwp to PCM.");
        }

        match sink.pcm.start(){
            Ok(()) => (),
            Err(errno) => {
                error!("Error starting PCM: {errno}");
                sink.pcm.drain().expect("Drain failed");
                match sink.pcm.recover(errno.errno(), true){
                    Ok(()) => (),
                    Err(errno) => error!("Recovering after failed start failed too ({errno})."),
                }
            },
        }

        // Debug
        // let ff = pcm.hw_params_current().and_then(|h| h.get_format())?;

        // {
        //     let params = sink.pcm.hw_params_current().unwrap();
        //     println!("(Debug) HwParams: {:?}", params);
        //     let sr = params.get_rate().unwrap();
        //     let nch = params.get_channels().unwrap();
        //     let fmt = params.get_format().unwrap();
        //     let bsize = params.get_buffer_size().unwrap();
        //     let psize = params.get_period_size().unwrap();
            
        //     println!("Created playback device with sr={sr}, channels={nch}, format={fmt}, period size={psize} and buffer size={bsize}.\n");
        // }

        {
            let swp = sink.pcm.sw_params_current().unwrap();
            match swp.set_start_threshold(512) {
                Ok(()) => (),
                Err(errno) => warn!("Could not set start_threshold sw parameter (error {errno})."),
            }

            let thr = swp.get_start_threshold().unwrap();

            // TODO? Set silence threshold?

        }
        Some(sink)
    }

}

impl VbanSink for AlsaSink {

    fn write(&self, buf : &[i16]){
        let io = self.pcm.io_i16().unwrap();

        match io.writei(buf){
            Err(errno) => {
                // Maybe try to investigate the pcm device here and try to reopen it (because broken pipe)

                warn!("Write did not work. Error: {errno}");
                // let state = self.pcm.state();

                match self.pcm.recover(errno.errno(), true){
                    Ok(()) => {
                        warn!("Was able to recover from error");
                        match io.writei(buf){
                            Ok(num) => trace!("Wrote {num} samples into ALSA device after recovery."),
                            Err(errno) => error!("Second attempt to write buffer failed ({errno})."),
                        }
                    },
                    Err(errno2) => error!("Could not recover from error (errno2={errno2}"),
                }
            },
            Ok(num) => trace!("Wrote {num} samples into ALSA device."),
        }

    }
}



// ****************************************
//             VBAN SOURCES
// ****************************************
pub trait VbanSource {
    fn read(&mut self, buf : &mut [i16]);
}


// ****************************************
//             ALSA SOURCE
// ****************************************

struct AlsaSource {
    pcm : PCM
}

impl AlsaSource {

    pub fn init(device : &str, num_channels : u32, sample_rate : u32) -> Option<Self> {
        let source = Self {
            pcm : PCM::new(device, Direction::Capture, false).expect("Could not create capture PCM")
        };

        {
            let hwp = HwParams::any(&source.pcm).expect("Could not get hwp.");

            hwp.set_channels(num_channels).expect("Could not set channel number.");
            hwp.set_rate(sample_rate, ValueOr::Nearest).expect("Could not set sample rate.");
            hwp.set_format(Format::s16()).expect("Could not set sample format.");
            hwp.set_access(Access::RWInterleaved).expect("Could not set access.");
            source.pcm.hw_params(&hwp).expect("Could not attach hwp to PCM.");
        }

        match source.pcm.start(){
            Ok(()) => (),
            Err(errno) => {
                warn!("Error starting PCM: {errno}");
                source.pcm.drain().expect("Drain failed");
                match source.pcm.recover(errno.errno(), true){
                    Ok(()) => (),
                    Err(errno) => error!("Recovering after failed start failed too ({errno}."),
                }
            },
        }

        {
            let swp = source.pcm.sw_params_current().unwrap();
            match swp.set_start_threshold(512) {
                Ok(()) => (),
                Err(errno) => warn!("Could not set start_threshold sw parameter (error {errno})."),
            }

            let thr = swp.get_start_threshold().unwrap();
            // todo? set silence threshold?
            debug!("Start threshold is {thr}.");
        }

        Some(source)
    }
}

impl VbanSource for AlsaSource {
    fn read(&mut self, buf : &mut [i16]) {
        let io = match self.pcm.io_i16(){
            Err(e) => {
                error!("PCM error while grabbing I/O: {e}");
                return;
            },
            Ok(io) => io
        };

        match io.readi(buf){
            Ok(frames) => trace!("PCM: read {frames} frames"),
            Err(e) => { 
                error!("PCM I/O Error: {e}");
                return;
            }
        }

    }
}

#[cfg(feature = "pipewire")]
struct PipewireSource {
    rx : Receiver<Vec<u8>>,
    remainder : Vec<u8>,
    _handle : JoinHandle<Option<()>>
}

impl PipewireSource {
    pub fn init(sample_rate: u32, target : Option<String>) -> Option<Self> {

        // create arc/mutex of self and put data into self.data in seperate thread?

        // create a channel, read from the channel in the sender::read function. implement a for loop in the ::handle to send all samples
        let (tx , rx) : (Sender<Vec<u8>>, Receiver<Vec<u8>>)= channel();

        let src = PipewireSource {
            rx,

            remainder : Vec::<u8>::new(),

            _handle : PipewireSource::get_pw_loop_handle(sample_rate, target, tx)
        };

        Some(src)

    }

    fn get_pw_loop_handle(sample_rate : u32, target : Option<String>, tx: Sender<Vec<u8>>) -> JoinHandle<Option<()>> {
        std::thread::spawn(move ||{

                let mainloop = match MainLoop::new(None){
                    Ok(theloop) => theloop,
                    Err(e) => {
                        error!("Error while creating a pipewire main loop ({e}).");
                        return None;
                    }
                };

                let context = match Context::new(&mainloop){
                    Ok(ctx) => ctx,
                    Err(e) => {
                        error!("Error while creating pipewire context: {e}.");
                        return None;
                    }
                };

                let core = match context.connect(None){
                    Ok(c) => c,
                    Err(e) => {
                        error!("Error while connecting pipewire core to context: {e}.");
                        return None;
                    }
                };

                let tgt = match target {
                    None => "".to_string(),
                    Some(str) => str
                };
        
                let stream_props = properties!{
                    *pipewire::keys::MEDIA_TYPE => "Audio",
                    *pipewire::keys::MEDIA_CATEGORY => "Capture",
                    *pipewire::keys::MEDIA_ROLE => "Music",
                    *pipewire::keys::MODULE_DESCRIPTION => "Pipewire Rust Test",
                    // *pipewire::keys::AUDIO_FORMAT => "S16LE",
                    // *pipewire::keys::AUDIO_ALLOWED_RATES => "[ 44100 ]",
                    *pipewire::keys::TARGET_OBJECT => tgt.as_str()
                };
                
                let stream = Stream::new(&core, "vban", stream_props).unwrap();
                let _handle = stream.add_local_listener().process( move |stream, _: &mut Vec<u8>| {
                    let mut buf = match stream.dequeue_buffer(){
                        None => return,
                        Some(buffer) => buffer
                    };
                    let size = buf.datas_mut()[0].chunk().size();
                    let data = Vec::from(buf.datas_mut()[0].data().unwrap());
                    let data = &data[..size as usize];
        
                    // let mut buffer = buffer.write().unwrap();
                    // buffer.resize(data.len(), 0);
                    // buffer.copy_from_slice(data);

                    let iter = data.chunks_exact(256);
                    for chunks in iter{
                        let _ = tx.send(chunks.to_vec());
                    }
        
                }).register().unwrap();
        
                
                // set up stream connection
                let mut pod_data = vec![0];
                let builder = spa::pod::builder::Builder::new(&mut pod_data);
                let mut audio_info = spa::param::audio::AudioInfoRaw::new();
                audio_info.set_format(AudioFormat::S16LE);
                audio_info.set_channels(2);
                audio_info.set_rate(sample_rate);
                unsafe {
                    spa_format_audio_raw_build(builder.as_raw_ptr(), spa::sys::SPA_PARAM_EnumFormat, &mut audio_info.as_raw());
                }
                let pod = spa::pod::Pod::from_bytes(&pod_data).unwrap();
                stream.connect(spa::utils::Direction::Input, Some(pipewire::constants::ID_ANY), pipewire::stream::StreamFlags::AUTOCONNECT, &mut [pod]).expect("Could not connect pipewire stream.");
                
                mainloop.run();

                Some(())    // is never reached
            })
            
    }

}

impl VbanSource for PipewireSource {
    fn read(&mut self, buf : &mut [i16]) {

        let bytes = buf.len() * 2;

        let mut data = match self.remainder.len() > 0 {
            false => self.rx.recv().unwrap(),
            true => {
                let d = Vec::from(self.remainder.clone());
                self.remainder.clear();
                d
            }
        };

        while data.len() < bytes{
            data.append(self.rx.recv().unwrap().as_mut());
        }

        if data.len() > bytes{
            self.remainder.append(&mut data[bytes..].to_vec());
            trace!("remainder has a new length of {} bytes", self.remainder.len());
            data = data[..bytes].to_vec();
        }

        if bytes != data.len(){
            panic!("sizes of pipewire and vban data are different: data {}, vban: {}", data.len(), buf.len()*2);
        }

        for (idx, frame) in data.chunks(2).enumerate(){
            buf[idx] = LittleEndian::read_i16(frame);
        }

        trace!("read {} bytes from pipewire", data.len());
    }
}