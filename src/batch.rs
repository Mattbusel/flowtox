//! Batch versions of the streaming estimators, over plain `&[f64]` slices.
//!
//! Each function runs the matching streaming type over the input and returns one
//! output per input element, so results line up with your price column.

use crate::classify::{BulkVolumeClassifier, LeeReady, Side, TickRule};
use crate::error::{Error, Result};
use crate::vpin::Vpin;

fn same_len(a: usize, b: usize) -> Result<()> {
    if a == b {
        Ok(())
    } else {
        Err(Error::LengthMismatch {
            expected: a,
            got: b,
        })
    }
}

/// Tick-rule side of every trade.
///
/// ```
/// use flowtox::{batch, Side};
/// let s = batch::tick_rule(&[10.0, 10.1, 10.1, 10.0]);
/// assert_eq!(s, vec![None, Some(Side::Buy), Some(Side::Buy), Some(Side::Sell)]);
/// ```
pub fn tick_rule(prices: &[f64]) -> Vec<Option<Side>> {
    let mut tr = TickRule::new();
    prices.iter().map(|&p| tr.classify(p)).collect()
}

/// Lee-Ready side of every trade, given the prevailing bid and ask per trade.
pub fn lee_ready(prices: &[f64], bids: &[f64], asks: &[f64]) -> Result<Vec<Option<Side>>> {
    same_len(prices.len(), bids.len())?;
    same_len(prices.len(), asks.len())?;
    let mut lr = LeeReady::new();
    Ok(prices
        .iter()
        .zip(bids)
        .zip(asks)
        .map(|((&p, &b), &a)| lr.classify(p, b, a))
        .collect())
}

/// Bulk-volume buy share of every bar, rolling sigma over `window` price changes.
pub fn bulk_buy_fractions(closes: &[f64], window: usize) -> Result<Vec<Option<f64>>> {
    let mut bvc = BulkVolumeClassifier::new(window)?;
    Ok(closes.iter().map(|&p| bvc.buy_fraction(p)).collect())
}

/// VPIN after every trade, trades signed by the tick rule.
pub fn vpin(
    prices: &[f64],
    volumes: &[f64],
    bucket_volume: f64,
    n_buckets: usize,
) -> Result<Vec<Option<f64>>> {
    same_len(prices.len(), volumes.len())?;
    let mut v = Vpin::new(bucket_volume, n_buckets)?;
    Ok(prices
        .iter()
        .zip(volumes)
        .map(|(&p, &q)| v.update(p, q))
        .collect())
}

/// VPIN after every bar, bars classified by bulk volume classification.
pub fn vpin_bulk(
    closes: &[f64],
    volumes: &[f64],
    bucket_volume: f64,
    n_buckets: usize,
    sigma_window: usize,
) -> Result<Vec<Option<f64>>> {
    same_len(closes.len(), volumes.len())?;
    let mut v = Vpin::with_bulk_classification(bucket_volume, n_buckets, sigma_window)?;
    Ok(closes
        .iter()
        .zip(volumes)
        .map(|(&p, &q)| v.update(p, q))
        .collect())
}
