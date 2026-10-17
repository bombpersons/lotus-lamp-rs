//! Sends each hex frame given on the command line and prints any replies.
//! Usage: cargo run --example probe -- 7e0737ffffff00ffef [...]
use futures::StreamExt;
use lotus_lamp::LedStrip;
use std::time::Duration;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let strip = LedStrip::connect_first(Duration::from_secs(10)).await?;
    let mut notes = strip.notifications().await?;
    tokio::spawn(async move {
        while let Some(n) = notes.next().await {
            println!("  notify {}", hex(&n));
        }
    });
    for arg in std::env::args().skip(1) {
        let (frame, wait) = match arg.split_once('@') {
            Some((f, ms)) => (f.to_owned(), ms.parse()?),
            None => (arg, 800),
        };
        let bytes: Vec<u8> = (0..frame.len()).step_by(2).map(|i| u8::from_str_radix(&frame[i..i + 2], 16)).collect::<Result<_, _>>()?;
        println!("-> {} ({} bytes)", hex(&bytes), bytes.len());
        strip.send_raw(&bytes).await?;
        tokio::time::sleep(Duration::from_millis(wait)).await;
        println!("  fff3 = {}", hex(&strip.read_raw().await?));
    }
    strip.disconnect().await?;
    Ok(())
}
