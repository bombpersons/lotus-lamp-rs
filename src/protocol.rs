//! Byte-level encoding for the ELK-BLEDOM / MELK family of BLE LED controllers.
//!
//! Every command is a 9-byte frame: `7E 00 <cmd> <5 payload bytes> EF`.
//! These builders are pure so they can be tested without hardware.

use uuid::{uuid, Uuid};

pub const SERVICE_UUID: Uuid = uuid!("0000fff0-0000-1000-8000-00805f9b34fb");
pub const WRITE_CHAR_UUID: Uuid = uuid!("0000fff3-0000-1000-8000-00805f9b34fb");
pub const NOTIFY_CHAR_UUID: Uuid = uuid!("0000fff4-0000-1000-8000-00805f9b34fb");

/// Name prefixes these controllers advertise under.
pub const KNOWN_NAME_PREFIXES: &[&str] = &["MELK", "ELK-BLE", "LEDBLE", "ELK-BULB", "ELK-LAMPL"];

/// MELK firmware ignores commands until it has seen this handshake.
pub const MELK_INIT: &[&[u8]] = &[
    &[0x7E, 0x07, 0x83],
    &[0x7E, 0x04, 0x04, 0xF0, 0x00, 0x01, 0xFF, 0x00, 0xEF],
];

pub type Frame = [u8; 9];

/// Built-in animated modes. Speed is set separately with [`speed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Effect {
    JumpRgb = 0x87,
    Jump7Colors = 0x88,
    CrossfadeRgb = 0x89,
    Crossfade7Colors = 0x8A,
    CrossfadeRed = 0x8B,
    CrossfadeGreen = 0x8C,
    CrossfadeBlue = 0x8D,
    CrossfadeYellow = 0x8E,
    CrossfadeCyan = 0x8F,
    CrossfadeMagenta = 0x90,
    CrossfadeWhite = 0x91,
    CrossfadeRedGreen = 0x92,
    CrossfadeRedBlue = 0x93,
    CrossfadeGreenBlue = 0x94,
    Blink7Colors = 0x95,
    BlinkRed = 0x96,
    BlinkGreen = 0x97,
    BlinkBlue = 0x98,
    BlinkYellow = 0x99,
    BlinkCyan = 0x9A,
    BlinkMagenta = 0x9B,
    BlinkWhite = 0x9C,
}

impl Effect {
    pub const ALL: [Effect; 22] = [
        Effect::JumpRgb,
        Effect::Jump7Colors,
        Effect::CrossfadeRgb,
        Effect::Crossfade7Colors,
        Effect::CrossfadeRed,
        Effect::CrossfadeGreen,
        Effect::CrossfadeBlue,
        Effect::CrossfadeYellow,
        Effect::CrossfadeCyan,
        Effect::CrossfadeMagenta,
        Effect::CrossfadeWhite,
        Effect::CrossfadeRedGreen,
        Effect::CrossfadeRedBlue,
        Effect::CrossfadeGreenBlue,
        Effect::Blink7Colors,
        Effect::BlinkRed,
        Effect::BlinkGreen,
        Effect::BlinkBlue,
        Effect::BlinkYellow,
        Effect::BlinkCyan,
        Effect::BlinkMagenta,
        Effect::BlinkWhite,
    ];
}

fn frame(cmd: u8, payload: [u8; 5]) -> Frame {
    let [a, b, c, d, e] = payload;
    [0x7E, 0x00, cmd, a, b, c, d, e, 0xEF]
}

pub fn power(on: bool) -> Frame {
    if on {
        frame(0x04, [0xF0, 0x00, 0x01, 0xFF, 0x00])
    } else {
        frame(0x04, [0x00, 0x00, 0x00, 0xFF, 0x00])
    }
}

pub fn color(r: u8, g: u8, b: u8) -> Frame {
    frame(0x05, [0x03, r, g, b, 0x00])
}

/// `percent` is clamped to 0..=100.
pub fn brightness(percent: u8) -> Frame {
    frame(0x01, [percent.min(100), 0, 0, 0, 0])
}

pub fn effect(mode: Effect) -> Frame {
    effect_raw(mode as u8)
}

/// Any mode byte, for experimenting with firmware modes not in [`Effect`].
pub fn effect_raw(mode: u8) -> Frame {
    frame(0x03, [mode, 0x03, 0, 0, 0])
}

/// `percent` is clamped to 0..=100.
pub fn speed(percent: u8) -> Frame {
    frame(0x02, [percent.min(100), 0, 0, 0, 0])
}

/// Colour-temperature mode (only meaningful on strips with W channels). Values clamped to 0..=100.
pub fn white(warm: u8, cold: u8) -> Frame {
    frame(0x05, [0x02, warm.min(100), cold.min(100), 0, 0])
}

/// The official app's framing: `7E 07 <cmd> <params, unused slots FF> 00 EF`.
fn app_frame(cmd: u8, params: &[u8]) -> Frame {
    let mut f = [0x7E, 0x07, cmd, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xEF];
    f[3..3 + params.len()].copy_from_slice(params);
    f
}

/// RGBIC ("symphony") pattern, by id from [`crate::patterns::PATTERNS`].
pub fn pattern(id: u8) -> Frame {
    app_frame(0x03, &[id, 0x06])
}

/// Sound-reactive modes driven by the controller's own microphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MicEffect {
    Energy1 = 0x80,
    Rhythm1 = 0x81,
    Spectrum1 = 0x82,
    Scroll1 = 0x83,
    Energy2 = 0x84,
    Rhythm2 = 0x85,
    Spectrum2 = 0x86,
    Scroll2 = 0x87,
}

impl MicEffect {
    pub const ALL: [MicEffect; 8] = [
        MicEffect::Energy1,
        MicEffect::Rhythm1,
        MicEffect::Spectrum1,
        MicEffect::Scroll1,
        MicEffect::Energy2,
        MicEffect::Rhythm2,
        MicEffect::Spectrum2,
        MicEffect::Scroll2,
    ];
}

pub fn mic_effect(effect: MicEffect) -> Frame {
    app_frame(0x03, &[effect as u8, 0x04])
}

/// Microphone sensitivity, clamped to 0..=100.
pub fn mic_sensitivity(percent: u8) -> Frame {
    app_frame(0x06, &[percent.min(100)])
}

/// Order the strip's chips expect colour bytes in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireOrder {
    Rgb,
    Rbg,
    Grb,
    Gbr,
    Brg,
    Bgr,
}

/// Saved in the controller's flash. Untested on hardware.
pub fn wire_order(order: WireOrder) -> Frame {
    let (r, g, b) = (1, 2, 3);
    let seq = match order {
        WireOrder::Rgb => [r, g, b],
        WireOrder::Rbg => [r, b, g],
        WireOrder::Grb => [g, r, b],
        WireOrder::Gbr => [g, b, r],
        WireOrder::Brg => [b, r, g],
        WireOrder::Bgr => [b, g, r],
    };
    app_frame(0x81, &seq)
}

/// Number of LEDs on the strip (the app allows 10..=1000). Saved in the controller's flash.
/// Untested on hardware.
pub fn pixel_count(count: u16) -> Frame {
    let [lo, hi] = count.to_le_bytes();
    app_frame(0x21, &[lo, hi, 0x00])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(f: Frame) -> String {
        f.iter().map(|b| format!("{b:02x}")).collect()
    }

    // These frames were confirmed against the MELK-OA10 hardware.
    #[test]
    fn verified_frames() {
        assert_eq!(hex(power(true)), "7e0004f00001ff00ef");
        assert_eq!(hex(color(0xff, 0, 0)), "7e000503ff000000ef");
        assert_eq!(hex(color(0, 0xff, 0)), "7e00050300ff0000ef");
        assert_eq!(hex(color(0, 0, 0xff)), "7e0005030000ff00ef");
        assert_eq!(hex(power(false)), "7e0004000000ff00ef");
        assert_eq!(hex(brightness(50)), "7e00013200000000ef");
        assert_eq!(hex(effect(Effect::Jump7Colors)), "7e00038803000000ef");
        assert_eq!(hex(speed(10)), "7e00020a00000000ef");
        assert_eq!(hex(pattern(0x01)), "7e07030106ffff00ef");
        assert_eq!(hex(mic_effect(MicEffect::Spectrum1)), "7e07038204ffff00ef");
        assert_eq!(hex(mic_sensitivity(100)), "7e070664ffffff00ef");
    }

    // Byte layouts taken from the official app, not yet seen working on hardware.
    #[test]
    fn app_derived_frames() {
        assert_eq!(hex(wire_order(WireOrder::Grb)), "7e0781020103ff00ef");
        assert_eq!(hex(pixel_count(300)), "7e07212c0100ff00ef");
    }

    #[test]
    fn clamps_percentages() {
        assert_eq!(hex(brightness(250)), "7e00016400000000ef");
        assert_eq!(hex(speed(250)), "7e00026400000000ef");
    }
}
