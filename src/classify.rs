//! Trade signing: who initiated each trade, buyer or seller.
//!
//! Three classifiers, from the least to the most information they need:
//!
//! * [`TickRule`]: price only. A trade above the previous price is a buy, below
//!   is a sell, and an unchanged price repeats the last non-zero tick.
//! * [`LeeReady`]: price plus the prevailing quote (Lee and Ready, 1991). Trades
//!   above the midpoint are buys, below are sells, and trades at the midpoint
//!   fall back to the tick rule.
//! * [`BulkVolumeClassifier`]: bars instead of trades (Easley, Lopez de Prado and
//!   O'Hara, 2012). A bar's volume is split into a buy share `Phi(dP / sigma)`
//!   and a sell share `1 - Phi(dP / sigma)`, where `dP` is the bar's price change
//!   and `sigma` the standard deviation of recent price changes.
//!
//! Every classifier's `update` returns the *signed volume* of the input
//! (`+volume` for a buy, `-volume` for a sell, `buy - sell` for a bulk split),
//! or `None` while it cannot classify yet.

use std::collections::VecDeque;

use crate::error::{invalid, Result};
use crate::normal::normal_cdf;

/// The aggressor side of a trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Side {
    /// Buyer-initiated.
    Buy,
    /// Seller-initiated.
    Sell,
}

impl Side {
    /// `+1.0` for [`Side::Buy`], `-1.0` for [`Side::Sell`].
    pub fn sign(self) -> f64 {
        match self {
            Side::Buy => 1.0,
            Side::Sell => -1.0,
        }
    }

    /// Split `volume` into `(buy, sell)` according to this side.
    pub fn split(self, volume: f64) -> (f64, f64) {
        match self {
            Side::Buy => (volume, 0.0),
            Side::Sell => (0.0, volume),
        }
    }
}

// ---------------------------------------------------------------------------
// Tick rule
// ---------------------------------------------------------------------------

/// Streaming tick-rule classifier.
///
/// ```
/// use flowtox::{Side, TickRule};
/// let mut tr = TickRule::new();
/// assert_eq!(tr.classify(10.00), None);            // no previous trade
/// assert_eq!(tr.classify(10.01), Some(Side::Buy)); // uptick
/// assert_eq!(tr.classify(10.01), Some(Side::Buy)); // zero tick repeats the last sign
/// assert_eq!(tr.classify(10.00), Some(Side::Sell));
/// ```
#[derive(Debug, Clone, Default)]
pub struct TickRule {
    last_price: Option<f64>,
    last_side: Option<Side>,
}

impl TickRule {
    /// A classifier with no history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Classify a trade at `price`. Non-finite prices are ignored and return `None`
    /// without touching the state.
    pub fn classify(&mut self, price: f64) -> Option<Side> {
        if !price.is_finite() {
            return None;
        }
        let side = match self.last_price {
            None => None,
            Some(prev) if price > prev => Some(Side::Buy),
            Some(prev) if price < prev => Some(Side::Sell),
            Some(_) => self.last_side,
        };
        self.last_price = Some(price);
        if side.is_some() {
            self.last_side = side;
        }
        side
    }

    /// Classify a trade and return its signed volume.
    pub fn update(&mut self, price: f64, volume: f64) -> Option<f64> {
        self.classify(price).map(|s| s.sign() * volume)
    }

    /// `true` once a non-zero tick has been seen, so every later trade gets a side.
    pub fn is_ready(&self) -> bool {
        self.last_side.is_some()
    }

    /// Forget all history.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

// ---------------------------------------------------------------------------
// Lee-Ready
// ---------------------------------------------------------------------------

/// Streaming Lee-Ready (1991) classifier: quote rule first, tick rule at the midpoint.
///
/// Pass the quote that prevailed when the trade printed. Lee and Ready lagged
/// quotes by five seconds for 1980s NYSE data; with modern timestamped feeds,
/// use the last quote strictly before the trade. If the quote is unusable
/// (non-finite, non-positive or crossed), the tick rule decides.
///
/// ```
/// use flowtox::{LeeReady, Side};
/// let mut lr = LeeReady::new();
/// assert_eq!(lr.classify(100.0, 99.5, 100.5), None);             // at the mid, no tick history
/// assert_eq!(lr.classify(100.25, 99.5, 100.5), Some(Side::Buy));  // above the mid
/// assert_eq!(lr.classify(100.0, 99.5, 100.5), Some(Side::Sell));  // at the mid, downtick
/// ```
#[derive(Debug, Clone, Default)]
pub struct LeeReady {
    tick: TickRule,
}

impl LeeReady {
    /// A classifier with no history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Classify a trade at `price` against the prevailing `bid` and `ask`.
    pub fn classify(&mut self, price: f64, bid: f64, ask: f64) -> Option<Side> {
        // The tick rule always sees the trade, so its history stays complete.
        let by_tick = self.tick.classify(price);
        if price.is_finite() && bid.is_finite() && ask.is_finite() && bid > 0.0 && ask >= bid {
            let mid = 0.5 * (bid + ask);
            if price > mid {
                return Some(Side::Buy);
            }
            if price < mid {
                return Some(Side::Sell);
            }
        }
        by_tick
    }

    /// Classify a trade and return its signed volume.
    pub fn update(&mut self, price: f64, volume: f64, bid: f64, ask: f64) -> Option<f64> {
        self.classify(price, bid, ask).map(|s| s.sign() * volume)
    }

    /// `true` once the tick-rule fallback can classify midpoint trades.
    pub fn is_ready(&self) -> bool {
        self.tick.is_ready()
    }

    /// Forget all history.
    pub fn reset(&mut self) {
        self.tick.reset();
    }
}

// ---------------------------------------------------------------------------
// Bulk volume classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Sigma {
    /// Sample standard deviation (n - 1) of the last `window` price changes,
    /// the current one included.
    Rolling {
        window: usize,
        changes: VecDeque<f64>,
    },
    Fixed(f64),
}

/// Streaming bulk volume classification (Easley, Lopez de Prado and O'Hara, 2012).
///
/// Feed one update per bar (time bar or volume bar) with the bar's closing price
/// and total volume. The buy share of the bar is `Phi(dP / sigma)` with `dP` the
/// change from the previous close. With [`BulkVolumeClassifier::new`], `sigma` is
/// the sample standard deviation of the last `window` price changes including the
/// current one (the same convention as `diff().rolling(window).std()` in pandas).
/// With [`BulkVolumeClassifier::with_sigma`] it is a constant you supply.
///
/// If every change in the window is zero, the bar is split evenly.
#[derive(Debug, Clone)]
pub struct BulkVolumeClassifier {
    sigma: Sigma,
    last_price: Option<f64>,
    last_buy_fraction: Option<f64>,
}

impl BulkVolumeClassifier {
    /// Rolling-sigma classifier. `window` (at least 2) price changes are needed
    /// before the first classification.
    pub fn new(window: usize) -> Result<Self> {
        if window < 2 {
            return Err(invalid("window", "must be at least 2"));
        }
        Ok(Self {
            sigma: Sigma::Rolling {
                window,
                changes: VecDeque::with_capacity(window + 1),
            },
            last_price: None,
            last_buy_fraction: None,
        })
    }

    /// Fixed-sigma classifier: classifies from the second bar on.
    pub fn with_sigma(sigma: f64) -> Result<Self> {
        if !(sigma.is_finite() && sigma > 0.0) {
            return Err(invalid("sigma", "must be finite and positive"));
        }
        Ok(Self {
            sigma: Sigma::Fixed(sigma),
            last_price: None,
            last_buy_fraction: None,
        })
    }

    /// Push a bar close and return the buy share of its volume, in `[0, 1]`.
    pub fn buy_fraction(&mut self, price: f64) -> Option<f64> {
        if !price.is_finite() {
            return None;
        }
        let prev = self.last_price.replace(price);
        let dp = price - prev?;
        let frac = match &mut self.sigma {
            Sigma::Fixed(s) => normal_cdf(dp / *s),
            Sigma::Rolling { window, changes } => {
                changes.push_back(dp);
                if changes.len() > *window {
                    changes.pop_front();
                }
                if changes.len() < *window {
                    return None;
                }
                let n = changes.len() as f64;
                let mean = changes.iter().sum::<f64>() / n;
                let var = changes.iter().map(|c| (c - mean) * (c - mean)).sum::<f64>() / (n - 1.0);
                let sd = var.sqrt();
                if sd > 0.0 {
                    normal_cdf(dp / sd)
                } else {
                    0.5
                }
            }
        };
        self.last_buy_fraction = Some(frac);
        Some(frac)
    }

    /// Push a bar and return `(buy_volume, sell_volume)`.
    pub fn split(&mut self, price: f64, volume: f64) -> Option<(f64, f64)> {
        let f = self.buy_fraction(price)?;
        Some((volume * f, volume * (1.0 - f)))
    }

    /// Push a bar and return its signed volume `buy - sell = volume * (2 Phi - 1)`.
    pub fn update(&mut self, price: f64, volume: f64) -> Option<f64> {
        self.split(price, volume).map(|(b, s)| b - s)
    }

    /// The buy share returned by the most recent successful update.
    pub fn last_buy_fraction(&self) -> Option<f64> {
        self.last_buy_fraction
    }

    /// `true` once the next bar will be classified.
    pub fn is_ready(&self) -> bool {
        match &self.sigma {
            Sigma::Fixed(_) => self.last_price.is_some(),
            Sigma::Rolling { window, changes } => {
                self.last_price.is_some() && changes.len() + 1 >= *window
            }
        }
    }

    /// Forget all history (the window length or fixed sigma is kept).
    pub fn reset(&mut self) {
        if let Sigma::Rolling { changes, .. } = &mut self.sigma {
            changes.clear();
        }
        self.last_price = None;
        self.last_buy_fraction = None;
    }
}
