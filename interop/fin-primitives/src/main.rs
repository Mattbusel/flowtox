//! Feed `fin-primitives` ticks into flowtox.
//!
//! ```text
//! cd interop/fin-primitives && cargo run
//! ```
//!
//! fin-primitives gives you exact-decimal `Tick`s with an aggressor side. flowtox
//! computes VPIN from them two ways: from the venue's aggressor flag
//! (`Vpin::update_side`), and from price alone with the tick rule
//! (`Vpin::update`), which is what you get from a feed without the flag. The
//! ticks are a seeded random walk, not market data; after tick 6,000 the walk
//! turns into a run of one-sided buying.

use fin_primitives::tick::Tick;
use fin_primitives::types::{NanoTimestamp, Price, Quantity, Side as FpSide, Symbol};
use flowtox::{Side, Vpin};
use rust_decimal::Decimal;

fn side_of(t: &Tick) -> Side {
    if t.is_buy_aggressor() {
        Side::Buy
    } else {
        Side::Sell
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sym = Symbol::new("ETH-USD")?;
    let mut by_flag = Vpin::new(500.0, 50)?;
    let mut by_tick = Vpin::new(500.0, 50)?;

    let mut cents: i64 = 312_000;
    let mut state: u64 = 11;
    println!(
        "{:>6} {:>10} {:>12} {:>12}",
        "tick", "price", "vpin (flag)", "vpin (tick)"
    );
    for i in 0..12_000u64 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let r = (state >> 33) % 100;
        let buy = if i < 6_000 { r < 50 } else { r < 85 };
        cents += if buy { 1 } else { -1 };
        let tick = Tick::new(
            sym.clone(),
            Price::new(Decimal::new(cents, 2))?,
            Quantity::new(Decimal::new(5 + (r as i64 % 20), 0))?,
            if buy { FpSide::Bid } else { FpSide::Ask },
            NanoTimestamp::from_secs(1_767_625_200 + i as i64),
        );

        let (price, qty) = (tick.price.to_f64(), tick.quantity.to_f64());
        let a = by_flag.update_side(side_of(&tick), qty);
        let b = by_tick.update(price, qty);
        if i % 1_000 == 999 {
            let fmt = |v: Option<f64>| v.map_or("warmup".to_string(), |v| format!("{v:.3}"));
            println!("{:>6} {:>10.2} {:>12} {:>12}", i + 1, price, fmt(a), fmt(b));
        }
    }
    Ok(())
}
