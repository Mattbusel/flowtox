# Changelog

All notable changes to this crate are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the crate uses
[Semantic Versioning](https://semver.org/).

## [0.1.1] - 2026-11-20

### Added
- `examples/block_print.rs`: a 2,500-share print into 1,000-share buckets, split
  across buckets so VPIN stays a probability.

### Changed
- README opens with the problem flowtox takes a side on (unmeasured toxicity is a
  free option for informed traders) and the bucket boundary most hand-rolled VPIN
  code gets wrong; Cargo description and keywords match. No code change.

## [0.1.0] - 2026-11-15

First release, carved out of the `microstructure-engine` crate of
[srfm-lab](https://github.com/Mattbusel/srfm-lab) (MIT). See `NOTICE` and
`docs/PROVENANCE.md` for what was taken and what was rewritten.

### Added
- `Vpin`: streaming VPIN (Easley, Lopez de Prado, O'Hara 2012) with exact
  bucket splitting, in tick-rule mode (`Vpin::new`), bulk-classification mode
  (`Vpin::with_bulk_classification`) or fed pre-signed volume
  (`update_side`, `update_classified`).
- `BulkVolumeClassifier`: buy share `Phi(dP / sigma)` with a rolling or fixed sigma.
- `LeeReady` and `TickRule` trade classifiers.
- `PinEstimator`: EKOP (1996) PIN by maximum likelihood, EM from the Yan-Zhang
  (2012) grid of starting points, overflow-free log-likelihood.
- `BarBuilder`: tick, volume and dollar bars with a signed-volume split.
- `batch` module: the same estimators over `&[f64]` slices.
- `normal_cdf`: dependency-free standard normal CDF (Hart 1968).
- Feature `ta`: `ta::Next` / `ta::Reset` / `ta::Period` impls for `ta` 0.5.
- Feature `serde`: `Serialize` / `Deserialize` for parameters, fits, bars and sides.
- Reference tests with hand-computed expected values, and an example feeding
  `fin-primitives` ticks into `Vpin` (`interop/fin-primitives`).

[0.1.0]: https://github.com/Mattbusel/flowtox/releases/tag/v0.1.0
