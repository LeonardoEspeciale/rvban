
use std::{net::IpAddr, path::PathBuf, process::{exit, Command}, thread::sleep, time::Duration};
use clap::Parser;
use rvban::{vban_sender::VbanSender, VBanSampleRates, VBanBitResolution, VBanCodec};
use log::{error, info, trace, warn, debug};
use simplelog::{Config, TermLogger};

#[derive(Parser)]
struct Cli {

    /// IP address of the receiver, e.g. 192.168.0.100
    #[arg(short='i', long)]
    peer_address : String,

    /// Port of the receiver. Specify a port if you don't want to use the default port 6980.
    #[arg(short='p', long)]
    peer_port : Option<u16>,

    /// Specify an IP-address if you don't want to bind to all interfaces
    #[arg(short='l', long)]
    local_addr : Option<IpAddr>,

    /// Specify a different port if you don't want to use port 6980
    #[arg(short='o', long)]
    local_port : Option<u16>,

    /// Use a config file
    #[arg(short, long, value_name = "file")]
    config: Option<PathBuf>,

    /// Specify a stream name (defaults to Stream1)
    #[arg(short, long, value_name = "name")]
    stream_name : Option<String>,

    /// Name of the audio device that is used as a source (default is "default")
    #[arg(short, long)]
    device_name : Option<String>,

    /// Encoder (Opus, PCM)
    #[arg(short, long)]
    encoder : Option<String>,

    /// Set a log level for terminal printouts (0 = Off, 5 = Trace, default = 3).
    #[arg(short='v', long)]
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
            error!("{} is not a valid IP address", cli.peer_address);
            exit(1);
        }
    };

    let peer_port = match cli.peer_port {
        None => 6980,
        Some(port) => port
    };

    let peer_addr = (peer_ip, peer_port);


    let local_ip : IpAddr;
    let local_port : u16;
    let stream_name : Option<String>;
    let mut device_name = String::from("default");

    let encoder : VBanCodec;
    if cli.encoder.is_some(){
        encoder = match cli.encoder.unwrap().as_str(){
            "PCM" => {
                VBanCodec::VbanCodecPcm
            },
            "Opus" | "OPUS" | "opus" => {
                info!("Using OPUS encoder.");
                VBanCodec::VbanCodecOpus(None)
            },
            _ => {
                error!("Codec not recognized.");
                exit(1)
            }
        }
    } else {
        encoder = VBanCodec::VbanCodecPcm;
    }
    

    if use_config {
        // todo 
        local_ip = "127.0.0.1".parse().unwrap();
        local_port = 6980;
        stream_name = None;
    } else {
        local_ip = match cli.local_addr {
            None => "0.0.0.0".parse().unwrap(),
            Some(addr) => {
                println!("Using {addr} as address to bind to.");
                addr
            },
        };
        local_port = match cli.local_port {
            None => 6980,
            Some(num) => {
                println!("Using port {num}.");
                num
            },
        };
        stream_name = match cli.stream_name {
            None => None,
            Some(name) => {
                println!("Using {name} as stream name.");
                Some(name)
            },
        };
        device_name = match cli.device_name {
            // None => String::from("default"),
            None => String::from("pipewire"),
            Some(name) => name,
        };
    }

    let local_addr = (local_ip, local_port);

    let mut vbs = match VbanSender::create(peer_addr, local_addr, stream_name, 2, VBanSampleRates::SampleRate48000Hz, VBanBitResolution::VbanBitfmt16Int, device_name, Some(encoder)){
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