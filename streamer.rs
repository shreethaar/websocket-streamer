use clap::Parser;
use log::{error, info};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(author, version, about = "Streaming video from Raspberry Pi (libcamera)", long_about = None)]
struct Args {
    /// Width of the frame
    #[arg(short, long, default_value_t = 640)]
    width: u32,

    /// Height of the frame
    #[arg(short = 'H', long, default_value_t = 480)]
    height: u32,

    /// FPS from camera
    #[arg(short, long, default_value_t = 30)]
    fps: u32,

    /// Computer server IP address (TCP)
    #[arg(short, long, default_value = "10.0.0.1")]
    ip: String,

    /// Server port
    #[arg(short, long, default_value_t = 2281)]
    port: u16,

    /// Vertical flip (0 or 1)
    #[arg(short, long, default_value_t = 1)]
    vflip: u8,

    /// Horizontal flip (0 or 1)
    #[arg(long, default_value_t = 0)]
    hflip: u8,

    /// Timeout for reconnect/warmup
    #[arg(short, long, default_value_t = 1)]
    timeout: u64,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let args = Args::parse();

    loop {
        match run_stream(&args) {
            Ok(_) => {
                info!("Stream ended normally");
                break;
            }
            Err(e) => {
                error!("Error: {} — retrying in {}s...", e, args.timeout);
                thread::sleep(Duration::from_secs(args.timeout));
            }
        }
    }
}

fn run_stream(args: &Args) -> io::Result<()> {
    // -----------------------------
    // Connect to PC viewer
    // -----------------------------
    let mut stream = TcpStream::connect(format!("{}:{}", args.ip, args.port))?;
    info!("Connected to viewer at {}:{}", args.ip, args.port);

    // -----------------------------
    // Build libcamera-vid command
    // -----------------------------
    let mut cmd = Command::new("libcamera-vid");

    cmd.arg("--codec").arg("mjpeg")
        .arg("--timeout").arg("0") // run forever
        .arg("--framerate").arg(args.fps.to_string())
        .arg("--width").arg(args.width.to_string())
        .arg("--height").arg(args.height.to_string())
        .arg("-o").arg("-")        // output to stdout
        .stdout(Stdio::piped());

    if args.vflip == 1 {
        cmd.arg("--vflip");
    }
    if args.hflip == 1 {
        cmd.arg("--hflip");
    }

    info!("Starting libcamera-vid...");

    let mut child = cmd.spawn()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to start libcamera-vid: {}", e)))?;

    let mut camera_out = child.stdout.take()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to capture stdout"))?;

    // -----------------------------
    // Buffering logic
    // -----------------------------
    let mut buffer = Vec::new();
    let mut temp = [0u8; 4096];

    loop {
        let n = camera_out.read(&mut temp)?;

        if n == 0 {
            break; // camera finished
        }

        buffer.extend_from_slice(&temp[..n]);

        // JPEG ends with FF D9
        if buffer.len() >= 2 && buffer[buffer.len() - 2..] == [0xFF, 0xD9] {
            let size = buffer.len() as u32;

            // Send JPEG length
            stream.write_all(&size.to_le_bytes())?;
            // Send JPEG bytes
            stream.write_all(&buffer)?;

            buffer.clear(); // reset for next frame
        }
    }

    // End of stream marker
    stream.write_all(&0u32.to_le_bytes())?;
    Ok(())
}
