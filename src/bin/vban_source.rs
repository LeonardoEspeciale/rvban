
use std::{net::{IpAddr, UdpSocket}, path::PathBuf, process::exit};
use clap::Parser;
use rvban::{VBanSampleRates, VBanBitResolution, VBanCodec};
use log::{error, debug};
use simplelog::{Config, TermLogger};

#[cfg(feature = "alsa")]
use rvban::vban_sender_alsa::VbanSender ;

#[cfg(feature = "pipewire")]
use rvban::vban_sender_pw::VbanSender;


#[derive(Parser)]
struct Cli {

    /// IP address of the receiver, e.g. 192.168.0.100 (defaults to 127.0.0.1)
    #[arg(short='i', long, default_value = "127.0.0.1")]
    peer_address : String,

    /// Port of the receiver (defaults to 6980)
    #[arg(short='p', long, default_value_t = 6980)]
    peer_port : u16,

    /// Specify a stream name (defaults to "Stream1")
    #[arg(short='n', long, value_name = "NAME", default_value = "Stream1")]
    stream_name : String,

    /// Sample rate (defaults to 48000)
    #[arg(short='r', long, default_value = "48000")]
    sample_rate : u32,

    /// Specify an IP-address if you don't want to bind to all interfaces
    #[arg(short='a', long)]
    local_addr : Option<IpAddr>,

    /// Specify a local port if you don't want the OS to choose one for you
    #[arg(short='o', long)]
    local_port : Option<u16>,

    /// Use a config file (currently not supported)
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[arg(short, long, default_value = "spotify")]
    /// Name of the audio source, i.e. pipewire target application or ALSA (loopback) device (defaults to "spotify")
    source_name : String,

    /// Encoder [Opus (default), PCM]
    #[arg(short, long, default_value = "opus")]
    encoder : String,

    /// Set a log level for terminal printouts (0 = Off, 5 = Trace, default = 3).
    #[arg(short='l', long)]
    log_level : Option<usize>,
}

fn main() {
    let cli = Cli::parse();

    let use_config = match cli.config {
        None => false,
        Some(_) => panic!("Config files are currently not supported."),
    };

    let ll = match cli.log_level {
        None => log::LevelFilter::Info,
        Some(0) => log::LevelFilter::Off,
        Some(1) => log::LevelFilter::Error,
        Some(2) => log::LevelFilter::Warn,
        Some(3) => log::LevelFilter::Info,
        Some(4) => log::LevelFilter::Debug,
        Some(5) => log::LevelFilter::Trace,
        _ => {
            println!("Log level must be between 0 and 5. Using default.");
            log::LevelFilter::Info
        }
    };

    TermLogger::init(ll, Config::default(), simplelog::TerminalMode::Stdout, simplelog::ColorChoice::Auto).unwrap();

    let peer_ip : IpAddr = match cli.peer_address.parse(){
        Ok(addr) => {
            debug!("Using {} as peer address", addr);
            addr
        }
        Err(_e) => {
            error!("{} is not a valid IP address. Example: 127.0.0.1", cli.peer_address);
            exit(1);
        }
    };

    let peer_addr = (peer_ip, cli.peer_port);


    let local_ip : IpAddr;
    let local_port : u16;
    let sample_rate : VBanSampleRates;
    let mut source_name = String::from("default");

    let encoder = match cli.encoder.as_str(){
        "PCM" | "Pcm" | "pcm" => {
            VBanCodec::VbanCodecPcm
        },
        "Opus" | "OPUS" | "opus" => {
            debug!("Using OPUS encoder.");
            VBanCodec::VbanCodecOpus(None)
        },
        _ => {
            error!("Codec not recognized.");
            exit(1)
        }
    };
    

    if use_config {
        // todo: use a config
        local_ip = "127.0.0.1".parse().unwrap();
        local_port = 0;
        sample_rate = VBanSampleRates::SampleRate48000Hz;
    } else {
        local_ip = match cli.local_addr {
            None => "0.0.0.0".parse().unwrap(),
            Some(addr) => {
                debug!("Using {addr} as address to bind to.");
                addr
            },
        };
        local_port = match cli.local_port {
            None => 0,
            Some(num) => {
                debug!("Using local UDP port {num}.");
                num
            },
        };

        sample_rate = cli.sample_rate.into();
        if sample_rate == VBanSampleRates::SampleRateNotSupported {
            error!("Sample rate not supported. Supported sample rates are 8000, 16000, 32000, 44100, 48000, 88200, 96000, 176400 and 192000 Hz.");
            exit(1);
        }
        debug!("Using sample rate of {}", sample_rate);

        source_name = cli.source_name;
       
    }

    let local_addr = (local_ip, local_port);

    let mut vbs = VbanSender::create(peer_addr, local_addr, cli.stream_name, 2, sample_rate, VBanBitResolution::VbanBitfmt16Int, source_name, encoder.into()).expect("Error while initializing.");

    loop {
        vbs.handle();
    }
}