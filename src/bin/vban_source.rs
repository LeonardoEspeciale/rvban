
use std::{net::{IpAddr, UdpSocket}, path::PathBuf, process::exit};
use clap::Parser;
use rvban::{vban_sender::VbanSender, VBanSampleRates, VBanBitResolution, VBanCodec};
use log::{error, debug};
use simplelog::{Config, TermLogger};

#[derive(Parser)]
struct Cli {

    /// IP address of the receiver, e.g. 192.168.0.100
    #[arg(short='i', long, default_value = "127.0.0.1")]
    peer_address : String,

    /// Port of the receiver. Specify a port if you don't want to use the default port 6980.
    #[arg(short='p', long, default_value_t = 6980)]
    peer_port : u16,

    /// Specify a different stream name
    #[arg(short='n', long, value_name = "NAME", default_value = "Stream1")]
    stream_name : String,

    /// Sample rate
    #[arg(short='r', long, default_value = "48000")]
    sample_rate : u32,

    /// Specify an IP-address if you don't want to bind to all interfaces
    #[arg(short='l', long)]
    local_addr : Option<IpAddr>,

    /// Specify a different port if you don't want to use port 6980
    #[arg(short='o', long)]
    local_port : Option<u16>,

    /// Use a config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[arg(short, long)]
    /// Name of the audio source, i.e. pipewire target application or ALSA (loopback) device
    source_name : Option<String>,

    /// Encoder (Opus, PCM)
    #[arg(short, long, default_value = "opus")]
    encoder : String,

    /// Set a log level for terminal printouts (0 = Off, 5 = Trace, default = 3).
    #[arg(short='v', long)]
    log_level : Option<usize>,

    #[arg(short, long)]
    /// An audio backend to use (currently supported: alsa, pipewire)
    backend : Option<String>
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
        Some(1) => log::LevelFilter::Trace,
        Some(2) => log::LevelFilter::Debug,
        Some(3) => log::LevelFilter::Info,
        Some(4) => log::LevelFilter::Warn,
        Some(5) => log::LevelFilter::Error,
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
        local_port = 6980;
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
            None => {
                let mut port = 40101;
                let mut tries = 0;
                loop{
                    if UdpSocket::bind((local_ip, port)).is_err(){
                        if tries < 20 {
                            debug!("Port {} cannot be used for UDP. Trying with different port...", port);
                            port += 10;
                            tries += 1;
                        } else {
                            error!("Giving up after {tries} tries to find an open UDP port to bind to");
                            exit(-1)
                        }
                        continue;
                    } else {
                        break port;
                    }
                }
            },
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

        source_name = match cli.source_name {
            None => "spotify".to_string(),
            Some(str) => str
        };
       
    }

    let local_addr = (local_ip, local_port);

    let mut vbs = match VbanSender::create(peer_addr, local_addr, cli.stream_name, 2, sample_rate, VBanBitResolution::VbanBitfmt16Int, source_name, encoder.into()){
        None => {
            println!("Error: Could not create VBAN Sender");
            exit(1)
        }
        Some(sender) => sender
    };

    loop {
        vbs.handle();
    }

}