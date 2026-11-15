//! VPIN next to `ta` indicators, through `ta::Next`.
//!
//! ```text
//! cargo run --example ta_next --features ta
//! ```
//!
//! One-minute bars (a seeded random walk, not market data) go to ta's RSI and
//! to a bulk-classified flowtox VPIN in the same loop. Halfway through, the walk
//! picks up a one-sided drift and VPIN rises.

use flowtox::Vpin;
use ta::indicators::RelativeStrengthIndex;
use ta::{DataItem, Next};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rsi = RelativeStrengthIndex::new(14)?;
    // 20 bars' worth of volume per bucket, VPIN over 30 buckets, sigma over 50 bars.
    let mut vpin = Vpin::with_bulk_classification(20_000.0, 30, 50)?;

    let mut state: u64 = 7;
    let mut close = 100.0_f64;
    println!("{:>5} {:>9} {:>7} {:>7}", "bar", "close", "rsi14", "vpin");
    for i in 0..2_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let shock = ((state >> 11) as f64 / (1u64 << 53) as f64) - 0.5; // uniform in [-0.5, 0.5)
        let drift = if i >= 1_000 { 0.04 } else { 0.0 };
        close += 0.1 * shock + drift;
        let bar = DataItem::builder()
            .open(close)
            .high(close + 0.05)
            .low(close - 0.05)
            .close(close)
            .volume(1_000.0)
            .build()?;
        let r = rsi.next(close);
        let v = vpin.next(&bar);
        if i % 200 == 199 {
            match v {
                Some(v) => println!("{:>5} {:>9.3} {:>7.2} {:>7.3}", i + 1, close, r, v),
                None => println!("{:>5} {:>9.3} {:>7.2} {:>7}", i + 1, close, r, "warmup"),
            }
        }
    }
    Ok(())
}
