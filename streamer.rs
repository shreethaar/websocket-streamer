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
    info!("Starting broadcast to {}.", args.ip);

    // Build raspistill command for capturing JPEG frames
    let vflip = if args.vflip == 1 { "-vf" } else { "" };
    let hflip = if args.hflip == 1 { "-hf" } else { "" };
    
    let mut cmd = Command::new("raspistill");
    cmd.arg("-w").arg(args.width.to_string())
        .arg("-h").arg(args.height.to_string())
        .arg("-fps").arg(args.fps.to_string())
        .arg("-t").arg("0") // Run indefinitely
        .arg("-o").arg("-") // Output to stdout
        .arg("-n") // No preview
        .stdout(Stdio::piped());
    
    if !vflip.is_empty() {
        cmd.arg(vflip);
    }
    if !hflip.is_empty() {
        cmd.arg(hflip);
    }

    // Wait for camera warmup
    thread::sleep(Duration::from_secs(args.timeout));

    // Start the camera process
    let mut child = cmd.spawn()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to start camera: {}", e)))?;

    let mut stdout = child.stdout.take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to capture stdout"))?;

    // Buffer for reading JPEG data
    let mut buffer = Vec::new();
    let mut temp_buf = [0u8; 4096];

    loop {
        // Read frame data
        match stdout.read(&mut temp_buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                buffer.extend_from_slice(&temp_buf[..n]);
                
                // Check if we have a complete JPEG (ends with FFD9)
                if buffer.len() >= 2 && buffer[buffer.len()-2..] == [0xFF, 0xD9] {
                    // Send frame size as 4-byte little-endian integer
                    let size = buffer.len() as u32;
                    stream.write_all(&size.to_le_bytes())?;
                    stream.flush()?;
                    
                    // Send frame data
                    stream.write_all(&buffer)?;
                    
                    // Clear buffer for next frame
                    buffer.clear();
                }
            }
            Err(e) => return Err(e),
        }
    }

    // Send termination signal (size = 0)
    stream.write_all(&0u32.to_le_bytes())?;
    
    Ok(())
}
