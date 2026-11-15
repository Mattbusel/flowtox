# flowtox

[![crates.io](https://img.shields.io/crates/v/flowtox.svg)](https://crates.io/crates/flowtox)
[![docs.rs](https://docs.rs/flowtox/badge.svg)](https://docs.rs/flowtox)

Order-flow toxicity for Rust. It tells you how much of the flow you trade against is informed. You get streaming VPIN, bulk volume classification, Lee-Ready and tick-rule trade signing, and the EKOP PIN model fitted by maximum likelihood. There are no default dependencies.

## Install

```toml
[dependencies]
flowtox = "0.1"
```

## Example

```rust
use flowtox::Vpin;

fn main() -> Result<(), flowtox::Error> {
    // Buckets of 1,000 shares, VPIN over the last 50 buckets.
    let mut vpin = Vpin::new(1_000.0, 50)?;
    let mut price = 100.0;
    let mut last = None;
    for i in 0..10_000u32 {
        price += if i % 3 == 0 { -0.01 } else { 0.01 }; // two upticks per downtick
        last = vpin.update(price, 25.0);
    }
    // Two thirds of volume is buying, one third selling: |2/3 - 1/3| = 1/3.
    println!("VPIN = {:.3}", last.unwrap());
    Ok(())
}
```

More in `examples/`: `vpin_stream` (VPIN from a CSV of trades), `pin_fit` (PIN fit on simulated days) and `ta_next` (the `ta` feature).

## What is in it

| Type | What it measures |
|---|---|
| `Vpin` | Volume-synchronized probability of informed trading |
| `BulkVolumeClassifier` | Buy/sell split of a bar from its price change |
| `LeeReady` | Trade side from the quote, tick rule at the midpoint |
| `TickRule` | Trade side from the price change alone |
| `PinEstimator` | EKOP probability of informed trading, by maximum likelihood |
| `BarBuilder` | Tick, volume and dollar bars with a signed-volume split |

Every streaming type works the same way. `new` checks its parameters and returns a `Result`. `update(...)` returns `None` while warming up. There is also `is_ready()` and `reset()`. The `batch` module runs the same code over slices.

Features:

- `ta`: `ta::Next` and `ta::Reset` for `Vpin`, `BulkVolumeClassifier` and `TickRule`.
- `serde`: `Serialize` and `Deserialize` for parameters, fits, bars and sides.

Kyle's lambda, the Roll spread and Amihud illiquidity are not here. They live in [fin-primitives](https://docs.rs/fin-primitives).

## Docs

- API docs: [docs.rs/flowtox](https://docs.rs/flowtox)
- Formulas and references: [docs/ALGORITHMS.md](docs/ALGORITHMS.md)
- Where the code came from: [docs/PROVENANCE.md](docs/PROVENANCE.md)

## License

MIT. Carved out of [srfm-lab](https://github.com/Mattbusel/srfm-lab) (MIT). See [LICENSE](LICENSE) and [NOTICE](NOTICE).
