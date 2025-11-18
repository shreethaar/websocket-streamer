//10.19.47.196

use clap::Parser;
use log::{error, info};
use std::io::{self, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(author, version, about = "Streaming video from Raspberry Pi", long_about = None)]
struct Args {
    /// Width of frame
    #[arg(short, long, default_value_t = 640)]
    width: u32,

    /// Height of frame
    #[arg(short = 'H', long, default_value_t = 480)]
    height: u32,

    /// FPS from camera
    #[arg(short, long, default_value_t = 30)]
    fps: u32,

    /// WebSocket (computer) server IP address
    #[arg(short, long, default_value = "10.19.47.196")]
    ip: String,

    /// WebSocket (computer) server port
    #[arg(short, long, default_value_t = 2281)]
    port: u16,

    /// Flip frame vertically (0 or 1)
    #[arg(short, long, default_value_t = 1)]
    vflip: u8,

    /// Flip frame horizontally (0 or 1)
    #[arg(long, default_value_t = 0)]
    hflip: u8,

    /// Timeout for camera warmup in seconds
    #[arg(short, long, default_value_t = 1)]
    timeout: u64,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let args = Args::parse();

    loop {
        match run_stream(&args) {
            Ok(_) => {
                info!("Stream ended normally");
                break;
            }
            Err(e) => {
                error!("Error occurred: {}. Retrying in {}s...", e, args.timeout);
                thread::sleep(Duration::from_secs(args.timeout));
            }
        }
    }
}

fn run_stream(args: &Args) -> io::Result<()> {
    // Connect to server
    let mut stream = TcpStream::connect(format!("{}:{}", args.ip, args.port))?;
    info!("Connected to server successfully.");
    info
