//! Per-pixel demos for the Arduino backend.
//! Usage: cargo run --example pixels -- /dev/ttyUSB0 <led-count> [rainbow|dot|count]
//!
//! `count` lights one LED at a time with a running number on stdout, to find the strip's length.
use lotus_lamp::serial::{PixelStrip, Rgb};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).map_or("/dev/ttyUSB0", String::as_str);
    let len: usize = args.get(2).map_or(Ok(300), |s| s.parse())?;
    let mode = args.get(3).map_or("rainbow", String::as_str);

    let mut strip = PixelStrip::open(port, len)?;
    let mut frame = vec![Rgb::BLACK; len];
    match mode {
        "rainbow" => {
            strip.set_brightness(96);
            for t in 0.. {
                for (i, p) in frame.iter_mut().enumerate() {
                    *p = Rgb::from_hsv((i * 256 / len + t) as u8, 255, 255);
                }
                strip.show(&frame)?;
                std::thread::sleep(Duration::from_millis(16));
            }
        }
        "dot" => loop {
            for i in 0..len {
                frame.fill(Rgb::BLACK);
                frame[i] = Rgb::new(255, 255, 255);
                strip.show(&frame)?;
                std::thread::sleep(Duration::from_millis(20));
            }
        },
        "count" => {
            for i in 0..len {
                frame.fill(Rgb::BLACK);
                frame[i] = Rgb::new(0, 255, 0);
                // Every 10th LED in red so it's easy to count along the strip.
                for j in (9..=i).step_by(10) {
                    frame[j] = Rgb::new(255, 0, 0);
                }
                strip.show(&frame)?;
                println!("{}", i + 1);
                std::thread::sleep(Duration::from_millis(250));
            }
        }
        other => return Err(format!("unknown mode {other}").into()),
    }
    Ok(())
}
