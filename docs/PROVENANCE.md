# Provenance: what came from srfm-lab and what changed

flowtox started as the order-flow part of the `microstructure-engine` crate in
[srfm-lab](https://github.com/Mattbusel/srfm-lab) (MIT), commit
`8cbfcb0551d44e3ed1c20bdae9dc805d61521c62`, plus the Python cross-check in
`lib/math/microstructure.py`. This file lists what was kept, what was fixed and
what was left behind, so anyone comparing the two codebases knows why the
numbers differ.

## The two failing srfm-lab tests

At that commit `cargo test -p microstructure-engine` passes 66 of 68 tests.
Both failures are wrong expectations, not wrong code:

| Test | Expected | Actual | Why the expectation is wrong |
|---|---|---|---|
| `trade_flow_analysis::test_vpin_mixed` | VPIN near 0 | 0.2 | 10 buys and 10 sells, alternating, poured into 4 buckets of 5 units. Each bucket holds 3 of one side and 2 of the other, so every bucket's imbalance is exactly `|3 - 2| / 5 = 0.2`. |
| `streaming_stats::streaming_stats_basic` | std 2.0 | 2.138 | 2.0 is the population (divide by n) standard deviation of `[2, 4, 4, 4, 5, 5, 7, 9]`; `StreamingStats::std` returns the sample (n - 1) value, `sqrt(32 / 7)`. |

flowtox keeps the first case as a reference test with the correct answer (0.2),
and adds the even-bucket version (4-unit buckets, VPIN exactly 0) next to it.
`StreamingStats` is not part of flowtox.

## Rewritten

- **VPIN bucket filling** (`order_flow.rs::Vpin::update`). The proportional
  split across bucket boundaries was right, but the first trade (which the tick
  rule cannot sign) still added its volume to the bucket total without adding it
  to either side, pulling that bucket's imbalance toward 0, and invalid
  parameters panicked via `assert!` (with a hard minimum of 5 buckets). flowtox
  keeps unsigned volume out of the buckets, tracks fill separately, closes
  buckets with a relative tolerance and returns `Result` from constructors. The
  test `vpin_independent_of_how_trades_are_chunked` checks that pouring a trade
  in one piece or four gives identical buckets.
- **Bulk volume classification** (`microstructure.py::bulk_volume_classification`).
  The Python version set `sigma = |dP|`, so `z = dP / |dP|` was always +1 or -1
  and every bar got a buy share of 0.841 or 0.159. ELO (2012) standardize by the
  standard deviation of price changes across bars; flowtox uses the rolling
  sample standard deviation (or a fixed sigma you pass).
- **PIN estimation** (`pin.rs::PinEstimator`). The original M-step estimated
  `mu` from all buys on good-news days and all sells on bad-news days, which
  includes the uninformed arrivals, and estimated `eps_b` / `eps_s` from
  regimes where the informed rate was mixed in; the update is not the EM
  maximizer of the EKOP likelihood. `PinParams::log_likelihood` also multiplied
  a log-pmf by a pmf in the no-event term (a missing `.exp()`), and the test
  only checked that PIN landed in `[0, 1]`. flowtox splits each Poisson superposition by its rate
  shares (the proper EM step), runs it from the Yan and Zhang (2012) grid of
  starts and keeps the best likelihood. Tests check recovery of simulated
  parameters, that the fit beats the true parameters' likelihood, and that the
  numerical gradient vanishes at the fit.
- **Lee-Ready** (`microstructure.py::lee_ready`). Ported as is (quote rule, tick
  rule at the midpoint), plus handling of crossed or missing quotes.
- **Tick rule**. Unchanged logic; the first trade now returns `None` instead of
  `0`, and the Python version's "first trade is a buy" default is gone.
- **Bars** (`tick_bars.rs`). Tick, volume and dollar bars kept. The fixed-threshold
  "imbalance bars" were dropped: they are not the expected-imbalance bars of
  Lopez de Prado (2018), and shipping them under that name would mislead.

## Left out on purpose

- Kyle's lambda, the Roll spread, Amihud illiquidity and order-book imbalance:
  [fin-primitives](https://docs.rs/fin-primitives) already has
  `KyleLambda`, `RollImpliedSpread`, `AmihudIlliquidity` and `OrderImbalance`
  in its `microstructure` module.
- `liquidity_metrics`, `market_regime_signals`, `streaming_stats`: lab glue.
- Dependencies: srfm-lab's crate pulled in `rayon`, `nalgebra`, `chrono`,
  `anyhow`, `statrs`, `serde_json` and `thiserror`, and built a `cdylib`.
  flowtox has no default dependencies; `serde` and `ta` are optional features.
