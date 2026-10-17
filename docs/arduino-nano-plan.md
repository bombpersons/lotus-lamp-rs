# Plan: per-pixel control via an Arduino Nano

The strip's MELK-OA10 Bluetooth controller can't address individual LEDs (see the README),
so the plan is to replace it with an Arduino Nano clone driving the WS2812B data line directly,
controlled over USB serial.

## 1. Hardware (you)

1. **Cut the strip cable a few cm after the USB controller box.** Keeping the box means it can be
   rejoined later (Wago/screw connector) to get Bluetooth control back.
2. **Identify the three wires**: 5V, GND, Data. Don't trust colours; check against the labels on the
   strip's copper pads (`5V / DIN / GND`) or with a multimeter. The strip end nearest the
   controller is the input (DIN) end.
3. **Check the Nano's header pins.** If they're loose in the bag, they need soldering on. That's the one
   unavoidable soldering job. Otherwise use jumper wires.
4. **Wire it up** (Wago or screw connectors, no soldering needed):

   | Strip wire | Nano pin |
   |---|---|
   | 5V | 5V |
   | GND | GND |
   | Data | D6, ideally through a ~330 Ω resistor |

5. **Find the LED count** if it's on the listing or reel. Otherwise we'll find it in software in step 3.6.
6. **Give your user access to serial ports** (once, then log out and back in):

   ```bash
   sudo usermod -aG dialout $USER
   ```

Power comes from the Nano's USB (~450 mA safe budget), so it's still a single USB cable.

## 2. Firmware (Claude)

1. Install `arduino-cli` and the AVR core into the home directory (no sudo).
2. Write a FastLED sketch:
   - data on D6, GRB order
   - power limit `FastLED.setMaxPowerInVoltsAndMilliamps(5, 450)`
   - speaks the **Adalight** protocol at 500000 baud: header `Ada`, count hi/lo, checksum, then RGB bytes.
     Adalight works with existing tools (Hyperion, Prismatik).
   - shows a short test pattern on boot, so wiring can be checked without a host.
3. Flash it. The clone probably has a CH340 USB chip (`/dev/ttyUSB0`) and the "old bootloader"
   (`atmega328old`). Try both if the upload fails.

Expected throughput: ~150 fps at 100 LEDs, ~50 fps at 300 LEDs.

## 3. Rust API (Claude)

1. Add a serial backend to the `lotus_lamp` crate (`serialport` crate), alongside the BLE `LedStrip`:
   `PixelStrip::open(port, led_count)`, `set_pixels(&[Rgb])`, `fill`, `clear`, `set_brightness`.
2. Brightness and gamma are handled on the host, so the firmware stays a dumb frame sink.
3. Add an `examples/pixels.rs` demo: a moving dot, a rainbow chase, and a single lit pixel stepped along the strip.
4. CLI: `lotus-lamp pixels --port /dev/ttyUSB0 ...` subcommands.
5. Tests for the Adalight frame encoder (header and checksum).
6. Hardware check with you: colour order, end-to-end pixel count, power limit.

## Status

- **Done:**
  - `arduino-cli`, the AVR core and FastLED are installed.
  - The firmware (`firmware/adalight`) compiles.
  - `lotus_lamp::serial::PixelStrip` and `examples/pixels.rs` build, and their unit tests pass.
- **Not done yet:** nothing has run on hardware.
- **Firmware change from the plan:** it replies `K` after each frame, so the host never sends a frame while
  FastLED has interrupts off. Serial bytes arriving then would be lost.

Flash (try `cpu=atmega328` if the old-bootloader upload fails):

```bash
arduino-cli upload -p /dev/ttyUSB0 --fqbn arduino:avr:nano:cpu=atmega328old firmware/adalight
```

On boot the strip flashes red, green, blue. Then find the LED count:

```bash
cargo run --example pixels -- /dev/ttyUSB0 300 count
```

## Open questions

- Exact LED count (needed for the firmware buffer size; the Nano's 2 KB RAM limits it to ~500 LEDs).
- Whether you later want WiFi or network control. If so, an ESP32 running WLED is the upgrade path, and the
  Rust API could talk to it over its network protocols instead of serial.
