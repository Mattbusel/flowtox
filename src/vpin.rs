//! Volume-synchronized probability of informed trading (VPIN).
//!
//! Easley, Lopez de Prado and O'Hara (2012), "Flow Toxicity and Liquidity in a
//! High-frequency World", Review of Financial Studies 25(5).
//!
//! Classified volume is poured into consecutive buckets of exactly `V` units.
//! A trade that overflows a bucket is split, keeping its buy share, and the rest
//! goes to the next bucket. Over the last `n` full buckets:
//!
//! ```text
//! VPIN = sum_{tau} |V_tau^B - V_tau^S| / (n * V)
//! ```

use std::collections::VecDeque;

use crate::classify::{BulkVolumeClassifier, Side, TickRule};
use crate::error::{invalid, Result};

#[derive(Debug, Clone)]
enum Classifier {
    Tick(TickRule),
    Bulk(BulkVolumeClassifier),
}

/// Streaming VPIN estimator.
///
/// ```
/// use flowtox::Vpin;
/// // 10-unit buckets, VPIN over the last 2 buckets, trades signed by the tick rule.
/// let mut vpin = Vpin::new(10.0, 2).unwrap();
/// assert_eq!(vpin.update(100.0, 4.0), None);  // first trade: no tick yet, not bucketed
/// assert_eq!(vpin.update(101.0, 6.0), None);  // buy 6
/// assert_eq!(vpin.update(100.0, 8.0), None);  // sell 4 closes bucket 1 (6 vs 4), 4 carry over
/// let v = vpin.update(100.0, 6.0).unwrap();   // sell 6 closes bucket 2 (0 vs 10)
/// assert!((v - 0.6).abs() < 1e-12);           // (2/10 + 10/10) / 2
/// ```
#[derive(Debug, Clone)]
pub struct Vpin {
    bucket_volume: f64,
    n_buckets: usize,
    classifier: Classifier,
    cur_buy: f64,
    cur_sell: f64,
    cur_fill: f64,
    imbalances: VecDeque<f64>,
    completed: u64,
    value: Option<f64>,
}

impl Vpin {
    /// VPIN over `n_buckets` buckets of `bucket_volume` units, trades signed by
    /// the tick rule. ELO (2012) use 50 buckets per day: `bucket_volume = ADV / 50`.
    pub fn new(bucket_volume: f64, n_buckets: usize) -> Result<Self> {
        Self::build(bucket_volume, n_buckets, Classifier::Tick(TickRule::new()))
    }

    /// VPIN whose inputs are bars classified by bulk volume classification with a
    /// rolling sigma over `sigma_window` price changes, as in ELO (2012).
    pub fn with_bulk_classification(
        bucket_volume: f64,
        n_buckets: usize,
        sigma_window: usize,
    ) -> Result<Self> {
        let bvc = BulkVolumeClassifier::new(sigma_window)?;
        Self::build(bucket_volume, n_buckets, Classifier::Bulk(bvc))
    }

    fn build(bucket_volume: f64, n_buckets: usize, classifier: Classifier) -> Result<Self> {
        if !(bucket_volume.is_finite() && bucket_volume > 0.0) {
            return Err(invalid("bucket_volume", "must be finite and positive"));
        }
        if n_buckets == 0 {
            return Err(invalid("n_buckets", "must be at least 1"));
        }
        Ok(Self {
            bucket_volume,
            n_buckets,
            classifier,
            cur_buy: 0.0,
            cur_sell: 0.0,
            cur_fill: 0.0,
            imbalances: VecDeque::with_capacity(n_buckets + 1),
            completed: 0,
            value: None,
        })
    }

    /// Push a trade (tick-rule mode) or a bar (bulk mode) and return the current VPIN,
    /// or `None` until `n_buckets` buckets are full.
    ///
    /// Volume the classifier cannot sign yet (the first trade in tick-rule mode, the
    /// first `sigma_window` bars in bulk mode) is left out of the buckets.
    /// Non-finite or non-positive volumes are ignored.
    pub fn update(&mut self, price: f64, volume: f64) -> Option<f64> {
        if !(volume.is_finite() && volume > 0.0) {
            return self.value;
        }
        let split = match &mut self.classifier {
            Classifier::Tick(tr) => tr.classify(price).map(|s| s.split(volume)),
            Classifier::Bulk(bvc) => bvc.split(price, volume),
        };
        match split {
            Some((b, s)) => self.update_classified(b, s),
            None => self.value,
        }
    }

    /// Push a trade whose side you already know (exchange aggressor flag, Lee-Ready).
    pub fn update_side(&mut self, side: Side, volume: f64) -> Option<f64> {
        let (b, s) = side.split(volume);
        self.update_classified(b, s)
    }

    /// Push already-classified buy and sell volume. The two may both be non-zero
    /// (a bulk-classified bar); if the input spans several buckets, each portion
    /// keeps the input's buy share.
    pub fn update_classified(&mut self, buy_volume: f64, sell_volume: f64) -> Option<f64> {
        let ok = |v: f64| v.is_finite() && v >= 0.0;
        if !(ok(buy_volume) && ok(sell_volume)) {
            return self.value;
        }
        let total = buy_volume + sell_volume;
        if total <= 0.0 {
            return self.value;
        }
        let buy_share = buy_volume / total;
        let eps = self.bucket_volume * 1e-12;
        let mut rem = total;
        while rem > eps {
            let space = self.bucket_volume - self.cur_fill;
            if rem < space - eps {
                self.cur_buy += rem * buy_share;
                self.cur_sell += rem * (1.0 - buy_share);
                self.cur_fill += rem;
                break;
            }
            self.cur_buy += space * buy_share;
            self.cur_sell += space * (1.0 - buy_share);
            rem -= space;
            self.close_bucket();
        }
        self.value
    }

    fn close_bucket(&mut self) {
        let imbalance = ((self.cur_buy - self.cur_sell).abs() / self.bucket_volume).min(1.0);
        self.imbalances.push_back(imbalance);
        if self.imbalances.len() > self.n_buckets {
            self.imbalances.pop_front();
        }
        self.completed += 1;
        self.cur_buy = 0.0;
        self.cur_sell = 0.0;
        self.cur_fill = 0.0;
        if self.imbalances.len() == self.n_buckets {
            // Summed afresh each bucket: n is small and this keeps rounding from drifting.
            let sum: f64 = self.imbalances.iter().sum();
            self.value = Some(sum / self.n_buckets as f64);
        }
    }

    /// The current VPIN, `None` until `n_buckets` buckets are full.
    pub fn value(&self) -> Option<f64> {
        self.value
    }

    /// `true` once `n_buckets` buckets are full.
    pub fn is_ready(&self) -> bool {
        self.value.is_some()
    }

    /// Order imbalance `|V^B - V^S| / V` of each bucket in the window, oldest first.
    pub fn bucket_imbalances(&self) -> impl Iterator<Item = f64> + '_ {
        self.imbalances.iter().copied()
    }

    /// Number of buckets completed since construction or the last reset.
    pub fn completed_buckets(&self) -> u64 {
        self.completed
    }

    /// Volume in the bucket being filled, as a fraction of `bucket_volume`.
    pub fn current_fill(&self) -> f64 {
        self.cur_fill / self.bucket_volume
    }

    /// Bucket size in volume units.
    pub fn bucket_volume(&self) -> f64 {
        self.bucket_volume
    }

    /// Number of buckets in the VPIN window.
    pub fn n_buckets(&self) -> usize {
        self.n_buckets
    }

    /// Forget all history; parameters are kept.
    pub fn reset(&mut self) {
        match &mut self.classifier {
            Classifier::Tick(t) => t.reset(),
            Classifier::Bulk(b) => b.reset(),
        }
        self.cur_buy = 0.0;
        self.cur_sell = 0.0;
        self.cur_fill = 0.0;
        self.imbalances.clear();
        self.completed = 0;
        self.value = None;
    }
}
