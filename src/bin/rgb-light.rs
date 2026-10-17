use clap::{Parser, Subcommand};
use rgb_light::{BDAddr, Effect, LedStrip};
use std::time::Duration;

#[derive(Parser)]
#[command(about = "Control an ELK-BLEDOM / MELK Bluetooth LED strip")]
struct Cli {
    /// Bluetooth address of the strip; defaults to the first one found.
    #[arg(short, long, global = true)]
    address: Option<BDAddr>,
    /// Scan timeout in seconds.
    #[arg(short, long, global = true, default_value_t = 10)]
    timeout: u64,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List strips in range.
    Scan,
    On,
    Off,
    /// Solid colour as hex, e.g. ff8000.
    Color { hex: String },
    /// 0-100.
    Brightness { percent: u8 },
    /// Effect name (see `effects`) or a raw mode byte like 0x8a.
    Effect { name: String },
    /// Effect speed 0-100.
    Speed { percent: u8 },
    /// List effect names.
    Effects,
    /// Send raw hex bytes, e.g. 7e000503ff000000ef.
    Raw { hex: String },
}

fn effect_name(e: Effect) -> String {
    let dbg = format!("{e:?}");
    let mut out = String::new();
    for (i, c) in dbg.chars().enumerate() {
        if (c.is_uppercase() || c.is_ascii_digit()) && i > 0 {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim_start_matches("0x").replace([' ', ':'], "");
    if s.len() % 2 != 0 {
        return Err(format!("odd-length hex: {s}"));
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string())).collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let timeout = Duration::from_secs(cli.timeout);

    match &cli.cmd {
        Cmd::Scan => {
            for s in LedStrip::discover(timeout).await? {
                println!("{}  {}  rssi={:?}", s.address, s.name, s.rssi);
            }
            return Ok(());
        }
        Cmd::Effects => {
            for e in Effect::ALL {
                println!("{:#04x}  {}", e as u8, effect_name(e));
            }
            return Ok(());
        }
        _ => {}
    }

    let strip = match cli.address {
        Some(a) => LedStrip::connect_address(a, timeout).await?,
        None => LedStrip::connect_first(timeout).await?,
    };

    match cli.cmd {
        Cmd::On => strip.power(true).await?,
        Cmd::Off => strip.power(false).await?,
        Cmd::Color { hex } => match parse_hex(&hex)?.as_slice() {
            &[r, g, b] => strip.set_color(r, g, b).await?,
            _ => return Err("colour must be 6 hex digits, e.g. ff8000".into()),
        },
        Cmd::Brightness { percent } => strip.set_brightness(percent).await?,
        Cmd::Speed { percent } => strip.set_speed(percent).await?,
        Cmd::Effect { name } => match Effect::ALL.into_iter().find(|e| effect_name(*e) == name) {
            Some(e) => strip.set_effect(e).await?,
            None => match parse_hex(&name)?.as_slice() {
                &[mode] => strip.set_effect_raw(mode).await?,
                _ => return Err(format!("unknown effect {name}; run `effects`").into()),
            },
        },
        Cmd::Raw { hex } => strip.send_raw(&parse_hex(&hex)?).await?,
        Cmd::Scan | Cmd::Effects => unreachable!(),
    }
    // Write-without-response is queued; give it a moment to go out before dropping the link.
    tokio::time::sleep(Duration::from_millis(300)).await;
    strip.disconnect().await?;
    Ok(())
}
