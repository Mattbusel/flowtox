//! Tick, volume and dollar bars with a tick-rule buy/sell split.
//!
//! Information-driven bars (Lopez de Prado, *Advances in Financial Machine
//! Learning*, 2018, ch. 2) sample the tape every `N` trades, every `N` units of
//! volume or every `N` units of notional. They are the natural input for
//! [`BulkVolumeClassifier`](crate::BulkVolumeClassifier) and for bulk-mode
//! [`Vpin`](crate::Vpin). A bar closes on the trade that reaches the threshold;
//! that trade is not split, so a volume bar can hold slightly more than `N`.

use crate::classify::{Side, TickRule};
use crate::error::{invalid, Result};

/// What closes a bar.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BarKind {
    /// Close after this many trades.
    Tick(u64),
    /// Close once traded volume reaches this amount.
    Volume(f64),
    /// Close once traded notional (`price * volume`) reaches this amount.
    Dollar(f64),
}

/// A completed (or in-progress) bar.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bar {
    /// First trade price.
    pub open: f64,
    /// Highest trade price.
    pub high: f64,
    /// Lowest trade price.
    pub low: f64,
    /// Last trade price.
    pub close: f64,
    /// Total volume.
    pub volume: f64,
    /// Total notional, `sum(price * volume)`.
    pub notional: f64,
    /// Number of trades.
    pub trades: u64,
    /// Volume signed as buys by the tick rule.
    pub buy_volume: f64,
    /// Volume signed as sells by the tick rule.
    pub sell_volume: f64,
}

impl Bar {
    fn open_at(price: f64) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            volume: 0.0,
            notional: 0.0,
            trades: 0,
            buy_volume: 0.0,
            sell_volume: 0.0,
        }
    }

    /// Volume-weighted average price (`close` if the bar has no volume).
    pub fn vwap(&self) -> f64 {
        if self.volume > 0.0 {
            self.notional / self.volume
        } else {
            self.close
        }
    }

    /// `(buy - sell) / (buy + sell)` in `[-1, 1]`, 0 when nothing was signed.
    pub fn imbalance(&self) -> f64 {
        let t = self.buy_volume + self.sell_volume;
        if t > 0.0 {
            (self.buy_volume - self.sell_volume) / t
        } else {
            0.0
        }
    }
}

/// Streaming bar builder.
///
/// ```
/// use flowtox::{BarBuilder, BarKind};
/// let mut bars = BarBuilder::new(BarKind::Volume(300.0)).unwrap();
/// assert!(bars.update(10.00, 100.0).is_none());
/// assert!(bars.update(10.02, 100.0).is_none());
/// let bar = bars.update(10.01, 100.0).unwrap();
/// assert_eq!((bar.open, bar.high, bar.low, bar.close), (10.00, 10.02, 10.00, 10.01));
/// assert_eq!((bar.buy_volume, bar.sell_volume), (100.0, 100.0)); // first trade is unsigned
/// ```
#[derive(Debug, Clone)]
pub struct BarBuilder {
    kind: BarKind,
    tick: TickRule,
    current: Option<Bar>,
}

impl BarBuilder {
    /// A builder for bars of the given kind; the threshold must be positive.
    pub fn new(kind: BarKind) -> Result<Self> {
        let ok = match kind {
            BarKind::Tick(n) => n > 0,
            BarKind::Volume(v) | BarKind::Dollar(v) => v.is_finite() && v > 0.0,
        };
        if !ok {
            return Err(invalid("kind", "bar threshold must be positive"));
        }
        Ok(Self {
            kind,
            tick: TickRule::new(),
            current: None,
        })
    }

    /// Push a trade; returns the bar it completed, if any. Trades with a
    /// non-finite price or a negative or non-finite volume are ignored.
    pub fn update(&mut self, price: f64, volume: f64) -> Option<Bar> {
        if !(price.is_finite() && volume.is_finite() && volume >= 0.0) {
            return None;
        }
        let side = self.tick.classify(price);
        let bar = self.current.get_or_insert_with(|| Bar::open_at(price));
        bar.high = bar.high.max(price);
        bar.low = bar.low.min(price);
        bar.close = price;
        bar.volume += volume;
        bar.notional += price * volume;
        bar.trades += 1;
        match side {
            Some(Side::Buy) => bar.buy_volume += volume,
            Some(Side::Sell) => bar.sell_volume += volume,
            None => {}
        }
        let done = match self.kind {
            BarKind::Tick(n) => bar.trades >= n,
            BarKind::Volume(v) => bar.volume >= v,
            BarKind::Dollar(v) => bar.notional >= v,
        };
        if done {
            self.current.take()
        } else {
            None
        }
    }

    /// The bar being built, if a trade has arrived since the last close.
    pub fn current(&self) -> Option<&Bar> {
        self.current.as_ref()
    }

    /// Forget the partial bar and the tick-rule history.
    pub fn reset(&mut self) {
        self.tick.reset();
        self.current = None;
    }
}
