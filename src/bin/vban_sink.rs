use std::{net::IpAddr, path::PathBuf, process::Command};
use simplelog::{TermLogger, Config};
use log::{info, error};
use rvban::{vban_recipient::VbanRecipient, VBanSampleRates};
use clap::{Parser};

/// VBAN Sink - by Lennard Jönsson 
/// Receive VBAN UDP streams on port 6980 (default) and play them on your ALSA audio device.
/// All credit for developing the VBAN protocol goes to vb-audio.com.



#[derive(Parser)]
struct Cli {
    /// Specify an IP-address if you don't want to bind to all interfaces
    addr : Option<IpAddr>,

    /// Specify a different port if you don't want to use port 6980
    #[arg(short, long)]
    port : Option<u16>,

    /// Use a config file
    #[arg(short, long, value_name = "file")]
    config: Option<PathBuf>,

    /// Specify a stream name if you want the application to discriminate incoming streams
    #[arg(short, long, value_name = "name")]
    stream_name : Option<String>,

    /// Prepend silence when starting playback. Supply duration in milliseconds.
    #[arg(short='x', long, value_name = "duration")]
    silence : Option<u32>,

    /// Name of the audio device that is used as a sink (default is "default")
    #[arg(short, long)]
    device_name : Option<String>,

    /// Specify a script file that is run when the playback state changes
    #[arg(short='m', long, value_name = "script")]
    command : Option<String>,

    /// Set a log level for terminal printouts (0 = Off, 5 = Trace, default = 3 (Info)).
    #[arg(short, long)]
    log_level : Option<usize>,

    /// Sample rate
    #[arg(short='r', long)]
    sample_rate : Option<u32>
}

// #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() -> Result<(), i32> {

    let cli = Cli::parse();

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

    let use_config = match cli.config {
        None => false,
        Some(_) => panic!("Config files are currently not supported."),
    };

    let addr : IpAddr;
    let port : u16;
    let stream_name : Option<String>;
    let mut device_name = String::from("default");

    let sr = match cli.sample_rate {
        None => VBanSampleRates::SampleRate48000Hz,
        Some (s) => s.into()
    };
    
    if use_config {
        // todo 
        addr = "127.0.0.1".parse().unwrap();
        port = 6980;
        stream_name = None;
    } else {
        addr = match cli.addr {
            None => "0.0.0.0".parse().unwrap(),
            Some(addr) => {
                info!("Using {addr} as address to bind to.");
                addr
            },
        };
        port = match cli.port {
            None => 6980,
            Some(num) => {
                info!("Using port {num}.");
                num
            },
        };
        stream_name = match cli.stream_name {
            None => None,
            Some(name) => {
                info!("Using {name} as stream name.");
                Some(name)
            },
        };
        device_name = match cli.device_name {
            None => String::from("default"),
            Some(name) => name,
        };
    }


    let mut vbr = match VbanRecipient::create(
    addr, port, stream_name, None, Some(sr),
    device_name, cli.silence){
        None => {
            error!("Could not create VBAN recipient.");
            return Err(-1)
        },
        Some(_vbr) => {
            _vbr
        }
    };

    match cli.command {
        None => (),
        Some(cmd) => {
            let handle = Command::new(cmd);
            vbr.set_command(handle);
        }
    }


    loop {
        vbr.handle();
    }

}
