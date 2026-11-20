# flowtox: if you quote without measuring who is on the other side, you are the liquidity informed traders pick off

[![crates.io](https://img.shields.io/crates/v/flowtox.svg)](https://crates.io/crates/flowtox)
[![docs.rs](https://docs.rs/flowtox/badge.svg)](https://docs.rs/flowtox)

A spread that does not widen when the flow turns informed is a free option for whoever knows more than you. VPIN and PIN are the published measures of that (Easley, Lopez de Prado and O'Hara 2012; Easley, Kiefer, O'Hara and Paperman 1996), and in Rust they are mostly written by hand: the first 100 GitHub code-search hits for `vpin` in Rust (Nov 20 2026) include 24 trading repositories, each with its own implementation. flowtox is one implementation, checked against values worked by hand from the papers, with no default dependencies.

**The line everyone rewrites, and where it breaks: the bucket boundary.** VPIN runs on a volume clock, so a trade bigger than the space left in a bucket has to be split across buckets. We read the bucketing code of six of those Rust implementations. Two get it wrong: one drops the whole print into the current bucket and still divides by the nominal bucket size, so its "probability" can go above 1; the other throws away the part of the trade that does not fit. flowtox splits it:

```sh
cargo run --example block_print
```
```text
VPIN before the block: 0.000
VPIN after a 2,500-share buy: 0.500
completed buckets: 6
```

**Why this instead of your own `vpin.rs`.** Exact bucket splitting in every mode (tick rule, bulk volume classification, or volume you have already signed), Lee-Ready and tick-rule trade signing, and the EKOP PIN fitted by maximum likelihood, which most hand-rolled versions never get to. Every expected value in `tests/reference.rs` is worked out by hand from the definitions, not copied from flowtox's own output.

**Why today.** Zero default dependencies, `cargo add flowtox`, and the `ta` feature makes `Vpin`, `BulkVolumeClassifier` and `TickRule` implement `ta::Next`, so they drop into code built on the `ta` crate without glue.

**Who it is for:** market makers, execution and risk code in Rust that needs to know when the flow it trades against has turned toxic.

**Trade-offs, on purpose:** `f64`, not `Decimal` (toxicity is a statistic, not money); `update` returns `None` until the window is full instead of guessing; parameters are checked in `new` and bad ones are an error, never a silent default. Kyle's lambda, the Roll spread and Amihud illiquidity are not here; they live in [fin-primitives](https://docs.rs/fin-primitives).

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

More in `examples/`: `block_print` (the bucket boundary), `vpin_stream` (VPIN from a CSV of trades), `pin_fit` (PIN fit on simulated days) and `ta_next` (the `ta` feature).

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


## Docs

- API docs: [docs.rs/flowtox](https://docs.rs/flowtox)
- Formulas and references: [docs/ALGORITHMS.md](docs/ALGORITHMS.md)
- Where the code came from: [docs/PROVENANCE.md](docs/PROVENANCE.md)

## License

MIT. Carved out of [srfm-lab](https://github.com/Mattbusel/srfm-lab) (MIT). See [LICENSE](LICENSE) and [NOTICE](NOTICE).
