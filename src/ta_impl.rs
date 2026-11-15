//! `ta` crate integration (feature `ta`).
//!
//! flowtox types implement [`ta::Next`] and [`ta::Reset`], so they sit next to
//! `ta` indicators in the same loop. Inputs are either a `(price, volume)` tuple
//! or anything implementing `ta::Close + ta::Volume` (such as `ta::DataItem`).
//! Outputs stay `Option<f64>`: `None` means "still warming up", never a made-up 0.

use ta::{Close, Next, Period, Reset, Volume};

use crate::classify::{BulkVolumeClassifier, Side, TickRule};
use crate::vpin::Vpin;

impl Next<(f64, f64)> for Vpin {
    type Output = Option<f64>;
    fn next(&mut self, (price, volume): (f64, f64)) -> Option<f64> {
        self.update(price, volume)
    }
}

impl<T: Close + Volume> Next<&T> for Vpin {
    type Output = Option<f64>;
    fn next(&mut self, input: &T) -> Option<f64> {
        self.update(input.close(), input.volume())
    }
}

impl Reset for Vpin {
    fn reset(&mut self) {
        Vpin::reset(self)
    }
}

impl Period for Vpin {
    /// Number of volume buckets in the VPIN window.
    fn period(&self) -> usize {
        self.n_buckets()
    }
}

impl Next<(f64, f64)> for BulkVolumeClassifier {
    /// Signed volume `buy - sell` of the bar.
    type Output = Option<f64>;
    fn next(&mut self, (price, volume): (f64, f64)) -> Option<f64> {
        self.update(price, volume)
    }
}

impl<T: Close + Volume> Next<&T> for BulkVolumeClassifier {
    /// Signed volume `buy - sell` of the bar.
    type Output = Option<f64>;
    fn next(&mut self, input: &T) -> Option<f64> {
        self.update(input.close(), input.volume())
    }
}

impl Reset for BulkVolumeClassifier {
    fn reset(&mut self) {
        BulkVolumeClassifier::reset(self)
    }
}

impl Next<f64> for TickRule {
    type Output = Option<Side>;
    fn next(&mut self, price: f64) -> Option<Side> {
        self.classify(price)
    }
}

impl Reset for TickRule {
    fn reset(&mut self) {
        TickRule::reset(self)
    }
}
