use clap::{Parser, Subcommand};
use lotus_lamp::{patterns, BDAddr, Effect, LedStrip, MicEffect};
use std::time::Duration;

#[derive(Parser)]
#[command(about = "Control an ELK-BLEDOM / MELK Bluetooth LED strip")]
struct Cli {
    /// Bluetooth address of the strip, or the device id `scan` prints on macOS; defaults to the
    /// first one found.
    #[arg(short, long, global = true)]
    address: Option<String>,
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
    /// RGBIC pattern by name (see `patterns`) or id like 0x01.
    Pattern { name: String },
    /// List RGBIC pattern ids and names.
    Patterns,
    /// Sound-reactive mode using the controller's mic (see `mic-effects`).
    Mic { name: String },
    /// List mic effect names.
    MicEffects,
    /// Mic sensitivity 0-100.
    MicSensitivity { percent: u8 },
    /// Send raw hex bytes, e.g. 7e000503ff000000ef.
    Raw { hex: String },
}

fn kebab(camel: &str) -> String {
    let mut out = String::new();
    for (i, c) in camel.chars().enumerate() {
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
                let addr = if s.address == BDAddr::default() { &s.id } else { &s.address.to_string() };
                println!("{}  {}  rssi={:?}", addr, s.name, s.rssi);
            }
            return Ok(());
        }
        Cmd::Effects => {
            for e in Effect::ALL {
                println!("{:#04x}  {}", e as u8, kebab(&format!("{e:?}")));
            }
            return Ok(());
        }
        Cmd::Patterns => {
            for p in patterns::PATTERNS {
                println!("{:#04x}  {:<13} {}", p.id, format!("{:?}", p.category), p.name);
            }
            return Ok(());
        }
        Cmd::MicEffects => {
            for e in MicEffect::ALL {
                println!("{}", kebab(&format!("{e:?}")));
            }
            return Ok(());
        }
        _ => {}
    }

    let strip = match cli.address {
        Some(a) => LedStrip::connect_id(&a, timeout).await?,
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
        Cmd::Effect { name } => match Effect::ALL.into_iter().find(|e| kebab(&format!("{e:?}")) == name) {
            Some(e) => strip.set_effect(e).await?,
            None => match parse_hex(&name)?.as_slice() {
                &[mode] => strip.set_effect_raw(mode).await?,
                _ => return Err(format!("unknown effect {name}; run `effects`").into()),
            },
        },
        Cmd::Pattern { name } => match patterns::find(&name) {
            Some(p) => strip.set_pattern(p.id).await?,
            None => match parse_hex(&name)?.as_slice() {
                &[id] => strip.set_pattern(id).await?,
                _ => return Err(format!("unknown pattern {name}; run `patterns`").into()),
            },
        },
        Cmd::Mic { name } => match MicEffect::ALL.into_iter().find(|e| kebab(&format!("{e:?}")) == name) {
            Some(e) => strip.set_mic_effect(e).await?,
            None => return Err(format!("unknown mic effect {name}; run `mic-effects`").into()),
        },
        Cmd::MicSensitivity { percent } => strip.set_mic_sensitivity(percent).await?,
        Cmd::Raw { hex } => strip.send_raw(&parse_hex(&hex)?).await?,
        Cmd::Scan | Cmd::Effects | Cmd::Patterns | Cmd::MicEffects => unreachable!(),
    }
    // Write-without-response is queued; give it a moment to go out before dropping the link.
    tokio::time::sleep(Duration::from_millis(300)).await;
    strip.disconnect().await?;
    Ok(())
}
