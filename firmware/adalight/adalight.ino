// Adalight receiver for a WS2812B strip on an Arduino Nano.
//
// Frame: 'A' 'd' 'a' <count-1 hi> <count-1 lo> <hi ^ lo ^ 0x55> then count * RGB bytes.
// Replies 'K' once each frame is on the strip.
// The host (lotus_lamp::PixelStrip) owns brightness and gamma; this sketch just displays frames.

#include <FastLED.h>

#ifndef NUM_LEDS
#define NUM_LEDS 300  // upper bound; frames for fewer LEDs are fine
#endif
#define DATA_PIN 6
#define BAUD 500000
#define MAX_MILLIAMPS 450  // what the Nano's USB port can safely supply

CRGB leds[NUM_LEDS];

static uint8_t readByte() {
  while (!Serial.available()) {}
  return Serial.read();
}

static void bootTest() {
  const CRGB colors[] = {CRGB::Red, CRGB::Green, CRGB::Blue};
  for (const CRGB &c : colors) {
    fill_solid(leds, NUM_LEDS, c);
    FastLED.show();
    delay(300);
  }
  FastLED.clear(true);
}

void setup() {
  FastLED.addLeds<WS2812B, DATA_PIN, GRB>(leds, NUM_LEDS);
  FastLED.setMaxPowerInVoltsAndMilliamps(5, MAX_MILLIAMPS);
  bootTest();
  Serial.begin(BAUD);
  Serial.print("Ada\n");  // lets hosts detect the device
}

void loop() {
  // Sync on the magic word one byte at a time so a dropped byte can't wedge us.
  if (readByte() != 'A') return;
  if (readByte() != 'd') return;
  if (readByte() != 'a') return;
  uint8_t hi = readByte(), lo = readByte(), chk = readByte();
  if (chk != (hi ^ lo ^ 0x55)) return;

  uint16_t count = ((uint16_t)hi << 8 | lo) + 1;
  for (uint16_t i = 0; i < count; i++) {
    uint8_t r = readByte(), g = readByte(), b = readByte();
    if (i < NUM_LEDS) leds[i] = CRGB(r, g, b);
  }
  for (uint16_t i = count; i < NUM_LEDS; i++) leds[i] = CRGB::Black;
  // show() disables interrupts, so serial bytes arriving meanwhile are lost.
  // Ack afterwards so the host knows it's safe to send the next frame.
  FastLED.show();
  Serial.write('K');
}
