//! Per-pixel control of a WS2812B strip through an Arduino running `firmware/adalight`.
//!
//! ```no_run
//! use lotus_lamp::serial::{PixelStrip, Rgb};
//! let mut strip = PixelStrip::open("/dev/ttyUSB0", 60)?;
//! strip.set_brightness(128);
//! let mut frame = vec![Rgb::BLACK; strip.len()];
//! frame[0] = Rgb::new(255, 0, 0);
//! strip.show(&frame)?;
//! # Ok::<(), lotus_lamp::Error>(())
//! ```

use crate::{Error, Result};
use serialport::SerialPort;
use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

pub const BAUD: u32 = 500_000;

/// Opening the port resets the Nano; the old bootloader plus the boot test take ~3 s.
const HELLO_TIMEOUT: Duration = Duration::from_secs(6);
const ACK_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb::new(0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Hue/saturation/value, each 0..=255, using the rainbow-even hue wheel.
    pub fn from_hsv(h: u8, s: u8, v: u8) -> Self {
        let region = h / 43;
        let rem = (h - region * 43) as u16 * 6;
        let (s, v) = (s as u16, v as u16);
        let p = (v * (255 - s) / 255) as u8;
        let q = (v * (255 - (s * rem) / 255) / 255) as u8;
        let t = (v * (255 - (s * (255 - rem)) / 255) / 255) as u8;
        let v = v as u8;
        match region {
            0 => Rgb::new(v, t, p),
            1 => Rgb::new(q, v, p),
            2 => Rgb::new(p, v, t),
            3 => Rgb::new(p, q, v),
            4 => Rgb::new(t, p, v),
            _ => Rgb::new(v, p, q),
        }
    }
}

/// Encodes one Adalight frame. Panics if `pixels` is empty or longer than 65536.
pub fn encode_frame(pixels: &[Rgb], brightness: u8, out: &mut Vec<u8>) {
    assert!(!pixels.is_empty() && pixels.len() <= 1 << 16, "1..=65536 pixels per frame");
    let [hi, lo] = ((pixels.len() - 1) as u16).to_be_bytes();
    out.clear();
    out.extend_from_slice(&[b'A', b'd', b'a', hi, lo, hi ^ lo ^ 0x55]);
    let scale = |c: u8| ((c as u16 * (brightness as u16 + 1)) >> 8) as u8;
    for p in pixels {
        out.extend_from_slice(&[scale(p.r), scale(p.g), scale(p.b)]);
    }
}

pub struct PixelStrip {
    port: Box<dyn SerialPort>,
    len: usize,
    brightness: u8,
    buf: Vec<u8>,
}

impl PixelStrip {
    /// Opens the Arduino at `path` (e.g. `/dev/ttyUSB0`) driving `len` LEDs, and waits for it to boot.
    pub fn open(path: &str, len: usize) -> Result<Self> {
        let port = serialport::new(path, BAUD).timeout(Duration::from_millis(50)).open()?;
        let mut strip = Self { port, len, brightness: 255, buf: Vec::with_capacity(6 + len * 3) };
        strip.wait_for(b"Ada\n", HELLO_TIMEOUT)?;
        Ok(strip)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Global brightness applied on the host, 0..=255.
    pub fn set_brightness(&mut self, brightness: u8) {
        self.brightness = brightness;
    }

    /// Displays `pixels` (index 0 is the end nearest the Arduino) and waits until it's on the strip.
    /// Shorter frames leave the remaining LEDs off; extra pixels are dropped.
    pub fn show(&mut self, pixels: &[Rgb]) -> Result<()> {
        let pixels = &pixels[..pixels.len().min(self.len)];
        if pixels.is_empty() {
            return self.clear();
        }
        encode_frame(pixels, self.brightness, &mut self.buf);
        self.port.write_all(&self.buf)?;
        self.wait_for(b"K", ACK_TIMEOUT)
    }

    pub fn fill(&mut self, color: Rgb) -> Result<()> {
        self.show(&vec![color; self.len])
    }

    pub fn clear(&mut self) -> Result<()> {
        self.fill(Rgb::BLACK)
    }

    /// Reads until `needle` arrives, discarding anything before it.
    fn wait_for(&mut self, needle: &[u8], timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        let mut matched = 0;
        let mut byte = [0u8];
        while matched < needle.len() {
            if Instant::now() >= deadline {
                return Err(Error::Timeout(String::from_utf8_lossy(needle).trim().to_owned()));
            }
            match self.port.read(&mut byte) {
                Ok(1) if byte[0] == needle[matched] => matched += 1,
                Ok(1) => matched = usize::from(byte[0] == needle[0]),
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::TimedOut => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_header_and_checksum() {
        let mut out = Vec::new();
        encode_frame(&[Rgb::new(1, 2, 3); 300], 255, &mut out);
        // 299 = 0x012B, checksum 0x01 ^ 0x2B ^ 0x55 = 0x7F
        assert_eq!(&out[..6], b"Ada\x01\x2b\x7f");
        assert_eq!(out.len(), 6 + 900);
        assert_eq!(&out[6..9], &[1, 2, 3]);
    }

    #[test]
    fn brightness_scales_pixels() {
        let mut out = Vec::new();
        encode_frame(&[Rgb::new(255, 128, 0)], 127, &mut out);
        assert_eq!(&out[6..], &[127, 64, 0]);
        encode_frame(&[Rgb::new(255, 128, 0)], 0, &mut out);
        assert_eq!(&out[6..], &[0, 0, 0]);
    }

    #[test]
    fn hsv_primaries() {
        assert_eq!(Rgb::from_hsv(0, 255, 255), Rgb::new(255, 0, 0));
        assert_eq!(Rgb::from_hsv(0, 0, 200), Rgb::new(200, 200, 200));
    }
}
