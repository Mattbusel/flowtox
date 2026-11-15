//! # flowtox
//!
//! Order-flow toxicity for Rust: how much of the flow you are trading against is
//! informed. Streaming estimators with no default dependencies.
//!
//! | Type | What it measures | Reference |
//! |---|---|---|
//! | [`Vpin`] | Volume-synchronized probability of informed trading | Easley, Lopez de Prado, O'Hara (2012) |
//! | [`BulkVolumeClassifier`] | Buy/sell split of a bar from its price change | Easley, Lopez de Prado, O'Hara (2012) |
//! | [`LeeReady`] | Trade side from the quote, tick rule at the midpoint | Lee, Ready (1991) |
//! | [`TickRule`] | Trade side from the price change alone | |
//! | [`PinEstimator`] | EKOP probability of informed trading, by maximum likelihood | Easley, Kiefer, O'Hara, Paperman (1996) |
//! | [`BarBuilder`] | Tick, volume and dollar bars with a signed-volume split | Lopez de Prado (2018) |
//!
//! Streaming types follow one shape: a constructor that validates its
//! parameters and returns [`Result`], `update(...) -> Option<f64>` (`None` while
//! warming up), `is_ready()` and `reset()`. The [`batch`] module runs the same
//! code over slices.
//!
//! ```
//! use flowtox::Vpin;
//!
//! // Buckets of 1,000 shares, VPIN over the last 50 buckets.
//! let mut vpin = Vpin::new(1_000.0, 50)?;
//! let mut price = 100.0;
//! let mut last = None;
//! for i in 0..10_000u32 {
//!     price += if i % 3 == 0 { -0.01 } else { 0.01 }; // two upticks per downtick
//!     last = vpin.update(price, 25.0);
//! }
//! // Two thirds of volume is buying, one third selling: |2/3 - 1/3| = 1/3.
//! assert!((last.unwrap() - 1.0 / 3.0).abs() < 0.02);
//! # Ok::<(), flowtox::Error>(())
//! ```
//!
//! ## Features
//!
//! * `ta`: [`ta::Next`](https://docs.rs/ta/0.5/ta/trait.Next.html) and `ta::Reset`
//!   for [`Vpin`], [`BulkVolumeClassifier`] and [`TickRule`], with `(price, volume)`
//!   tuples or any `ta::Close + ta::Volume` item as input.
//! * `serde`: `Serialize` and `Deserialize` for parameters, fits, bars and sides.
//!
//! ## What is not here
//!
//! Kyle's lambda, the Roll spread and Amihud illiquidity are already in
//! [`fin-primitives`](https://docs.rs/fin-primitives) (`microstructure` module);
//! flowtox covers the toxicity side that crate does not.
//!
//! ## Provenance
//!
//! Carved out of the `microstructure-engine` crate of
//! [srfm-lab](https://github.com/Mattbusel/srfm-lab) (MIT), with the VPIN bucket
//! logic, bulk volume classification and PIN estimator rewritten and checked
//! against hand-computed values. See `NOTICE` and `docs/PROVENANCE.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod bars;
pub mod batch;
pub mod classify;
mod error;
mod normal;
pub mod pin;
pub mod vpin;

#[cfg(feature = "ta")]
mod ta_impl;

pub use bars::{Bar, BarBuilder, BarKind};
pub use classify::{BulkVolumeClassifier, LeeReady, Side, TickRule};
pub use error::{Error, Result};
pub use normal::normal_cdf;
pub use pin::{DailyCounts, PinEstimator, PinFit, PinParams};
pub use vpin::Vpin;
