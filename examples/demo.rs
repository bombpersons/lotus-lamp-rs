//! Walks through every command so you can check them by eye.
use rgb_light::{Effect, LedStrip};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), rgb_light::Error> {
    let strip = LedStrip::connect_first(Duration::from_secs(10)).await?;
    let step = |s: &str| println!("[demo] {s}");

    strip.power(true).await?;
    strip.set_color(255, 0, 0).await?;
    for pct in [100, 50, 10, 100] {
        step(&format!("red @ {pct}%"));
        strip.set_brightness(pct).await?;
        sleep(Duration::from_secs(3)).await;
    }

    step("jump-7-colors, slow");
    strip.set_effect(Effect::Jump7Colors).await?;
    strip.set_speed(10).await?;
    sleep(Duration::from_secs(6)).await;
    step("jump-7-colors, fast");
    strip.set_speed(100).await?;
    sleep(Duration::from_secs(5)).await;

    step("off");
    strip.power(false).await?;
    sleep(Duration::from_secs(3)).await;
    step("on, green");
    strip.power(true).await?;
    strip.set_color(0, 255, 0).await?;
    sleep(Duration::from_millis(300)).await;
    strip.disconnect().await
}
