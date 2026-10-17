# lotus-lamp

Rust API and CLI for ELK-BLEDOM / MELK Bluetooth LE LED strips (built against a
`MELK-OA10` 5V USB WS2812B RGBIC strip). The USB lead on these strips is power only;
all control is over BLE (service `fff0`, write `fff3`, notify `fff4`).

```rust
let strip = lotus_lamp::LedStrip::connect_first(std::time::Duration::from_secs(10)).await?;
strip.power(true).await?;
strip.set_pattern(lotus_lamp::patterns::find("Magic Forward").unwrap().id).await?;
strip.set_speed(60).await?;
```

```bash
cargo run --bin lotus-lamp -- patterns
cargo run --bin lotus-lamp -- pattern "magic forward"
cargo run --bin lotus-lamp -- color ff8000
cargo run --bin lotus-lamp -- mic spectrum-1
```

## What works on the MELK-OA10

| Feature | API | Verified |
|---|---|---|
| Power, solid colour, brightness, speed | `power`, `set_color`, `set_brightness`, `set_speed` | yes |
| Classic whole-strip effects (`0x87`–`0x9c`) | `set_effect` | yes |
| 213 RGBIC patterns (multi-colour, moving) | `set_pattern`, `patterns::PATTERNS` | yes |
| On-device microphone modes + sensitivity | `set_mic_effect`, `set_mic_sensitivity` | yes |
| Wire order, pixel count (saved to flash) | `set_wire_order`, `set_pixel_count` | no |

**Per-pixel control is not available.** None of the multi-point "DIY" commands in the
official app (`0x32`–`0x34`, `0x39`, `0x40`, `0x58`) have any effect on this
firmware, and it does not answer the app's status queries.

Don't send command byte `0x87` (factory reset) or `0x24` (protect code) via `send_raw`.

Command layouts were derived from the official Lotus Lamp X app; see `src/protocol.rs`.

## License

MIT, see [LICENSE](LICENSE).
