//! Control ELK-BLEDOM / MELK Bluetooth LE LED strips.
//!
//! ```no_run
//! # async fn demo() -> Result<(), rgb_light::Error> {
//! use rgb_light::{Effect, LedStrip};
//! use std::time::Duration;
//!
//! let strip = LedStrip::connect_first(Duration::from_secs(10)).await?;
//! strip.power(true).await?;
//! strip.set_color(255, 80, 0).await?;
//! strip.set_brightness(60).await?;
//! strip.set_pattern(rgb_light::patterns::find("Magic Forward").unwrap().id).await?;
//! strip.set_speed(30).await?;
//! strip.disconnect().await?;
//! # Ok(()) }
//! ```

pub mod patterns;
pub mod protocol;

pub use btleplug::api::BDAddr;
pub use patterns::{Pattern, PATTERNS};
pub use protocol::{Effect, MicEffect, WireOrder};

use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::time::Duration;
use tokio::time::{sleep, Instant};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bluetooth error: {0}")]
    Ble(#[from] btleplug::Error),
    #[error("no bluetooth adapter found")]
    NoAdapter,
    #[error("no LED strip found within the scan timeout")]
    NotFound,
    #[error("device has no fff3 write characteristic; not an ELK-BLEDOM/MELK controller?")]
    MissingCharacteristic,
}

pub type Result<T> = std::result::Result<T, Error>;

/// A strip seen during a scan.
#[derive(Debug, Clone)]
pub struct FoundStrip {
    pub name: String,
    pub address: BDAddr,
    pub rssi: Option<i16>,
    peripheral: Peripheral,
}

impl FoundStrip {
    pub async fn connect(self) -> Result<LedStrip> {
        LedStrip::open(self.peripheral).await
    }
}

/// A connected strip. Dropping it does not disconnect; call [`LedStrip::disconnect`].
pub struct LedStrip {
    peripheral: Peripheral,
    write_char: Characteristic,
    notify_char: Option<Characteristic>,
}

async fn default_adapter() -> Result<Adapter> {
    let manager = Manager::new().await?;
    manager.adapters().await?.into_iter().next().ok_or(Error::NoAdapter)
}

fn is_strip_name(name: &str) -> bool {
    protocol::KNOWN_NAME_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// Scans until `timeout`, calling `done` after each poll; stops early when it returns true.
async fn scan(timeout: Duration, mut done: impl FnMut(&[FoundStrip]) -> bool) -> Result<Vec<FoundStrip>> {
    let adapter = default_adapter().await?;
    adapter.start_scan(ScanFilter::default()).await?;
    let deadline = Instant::now() + timeout;
    let mut found = Vec::new();
    loop {
        found.clear();
        for p in adapter.peripherals().await? {
            let Some(props) = p.properties().await? else { continue };
            let Some(name) = props.local_name else { continue };
            if is_strip_name(&name) {
                found.push(FoundStrip { name: name.trim().to_owned(), address: p.address(), rssi: props.rssi, peripheral: p });
            }
        }
        if done(&found) || Instant::now() >= deadline {
            break;
        }
        sleep(Duration::from_millis(250)).await;
    }
    adapter.stop_scan().await?;
    Ok(found)
}

impl LedStrip {
    /// Lists every strip advertising during `timeout`.
    pub async fn discover(timeout: Duration) -> Result<Vec<FoundStrip>> {
        scan(timeout, |_| false).await
    }

    /// Connects to the first strip that shows up.
    pub async fn connect_first(timeout: Duration) -> Result<Self> {
        let found = scan(timeout, |f| !f.is_empty()).await?;
        found.into_iter().next().ok_or(Error::NotFound)?.connect().await
    }

    /// Connects to the strip with this Bluetooth address.
    pub async fn connect_address(address: BDAddr, timeout: Duration) -> Result<Self> {
        let found = scan(timeout, |f| f.iter().any(|s| s.address == address)).await?;
        found.into_iter().find(|s| s.address == address).ok_or(Error::NotFound)?.connect().await
    }

    async fn open(peripheral: Peripheral) -> Result<Self> {
        // BlueZ often aborts the first LE connection attempt to these controllers.
        const ATTEMPTS: u32 = 4;
        for attempt in 1..=ATTEMPTS {
            if peripheral.is_connected().await? {
                break;
            }
            match peripheral.connect().await {
                Ok(()) => break,
                Err(e) if attempt == ATTEMPTS => return Err(e.into()),
                Err(_) => sleep(Duration::from_millis(500 * attempt as u64)).await,
            }
        }
        peripheral.discover_services().await?;
        let chars = peripheral.characteristics();
        let find = |uuid| chars.iter().find(|c| c.uuid == uuid).cloned();
        let write_char = find(protocol::WRITE_CHAR_UUID).ok_or(Error::MissingCharacteristic)?;
        let notify_char = find(protocol::NOTIFY_CHAR_UUID);
        let strip = Self { peripheral, write_char, notify_char };
        for cmd in protocol::MELK_INIT {
            strip.send_raw(cmd).await?;
        }
        Ok(strip)
    }

    pub fn address(&self) -> BDAddr {
        self.peripheral.address()
    }

    /// Writes an arbitrary command. Useful for probing undocumented firmware features.
    pub async fn send_raw(&self, bytes: &[u8]) -> Result<()> {
        self.peripheral.write(&self.write_char, bytes, WriteType::WithoutResponse).await?;
        Ok(())
    }

    /// Reads the fff3 characteristic, where the firmware leaves replies to query commands.
    pub async fn read_raw(&self) -> Result<Vec<u8>> {
        Ok(self.peripheral.read(&self.write_char).await?)
    }

    /// Subscribes to fff4 and yields each notification payload.
    pub async fn notifications(&self) -> Result<Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>> {
        if let Some(c) = &self.notify_char {
            self.peripheral.subscribe(c).await?;
        }
        let uuid = protocol::NOTIFY_CHAR_UUID;
        Ok(self.peripheral.notifications().await?.filter_map(move |n| async move { (n.uuid == uuid).then_some(n.value) }).boxed())
    }

    pub async fn power(&self, on: bool) -> Result<()> {
        self.send_raw(&protocol::power(on)).await
    }

    pub async fn set_color(&self, r: u8, g: u8, b: u8) -> Result<()> {
        self.send_raw(&protocol::color(r, g, b)).await
    }

    /// 0..=100.
    pub async fn set_brightness(&self, percent: u8) -> Result<()> {
        self.send_raw(&protocol::brightness(percent)).await
    }

    pub async fn set_effect(&self, effect: Effect) -> Result<()> {
        self.send_raw(&protocol::effect(effect)).await
    }

    /// Selects a firmware mode by raw byte, including ones not in [`Effect`].
    pub async fn set_effect_raw(&self, mode: u8) -> Result<()> {
        self.send_raw(&protocol::effect_raw(mode)).await
    }

    /// Starts an RGBIC pattern (multiple colours moving along the strip). See [`PATTERNS`].
    pub async fn set_pattern(&self, id: u8) -> Result<()> {
        self.send_raw(&protocol::pattern(id)).await
    }

    /// Starts a sound-reactive mode using the controller's built-in microphone.
    pub async fn set_mic_effect(&self, effect: MicEffect) -> Result<()> {
        self.send_raw(&protocol::mic_effect(effect)).await
    }

    /// Microphone sensitivity, 0..=100.
    pub async fn set_mic_sensitivity(&self, percent: u8) -> Result<()> {
        self.send_raw(&protocol::mic_sensitivity(percent)).await
    }

    /// Saved to the controller's flash; only change this if colours come out swapped.
    pub async fn set_wire_order(&self, order: WireOrder) -> Result<()> {
        self.send_raw(&protocol::wire_order(order)).await
    }

    /// Saved to the controller's flash; sets how many LEDs patterns span.
    pub async fn set_pixel_count(&self, count: u16) -> Result<()> {
        self.send_raw(&protocol::pixel_count(count)).await
    }

    /// Pattern and effect speed, 0..=100.
    pub async fn set_speed(&self, percent: u8) -> Result<()> {
        self.send_raw(&protocol::speed(percent)).await
    }

    /// Warm/cold white, each 0..=100.
    pub async fn set_white(&self, warm: u8, cold: u8) -> Result<()> {
        self.send_raw(&protocol::white(warm, cold)).await
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.peripheral.disconnect().await?;
        Ok(())
    }
}
