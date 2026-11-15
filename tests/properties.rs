//! Invariants and statistical recovery checks.

use flowtox::{BarBuilder, BarKind, DailyCounts, PinEstimator, PinParams, Side, Vpin};

struct Lcg(u64);
impl Lcg {
    fn uniform(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn poisson(&mut self, lambda: f64) -> u64 {
        let u = self.uniform();
        let (mut k, mut p) = (0u64, (-lambda).exp());
        let mut cdf = p;
        while u > cdf && k < 100_000 {
            k += 1;
            p *= lambda / k as f64;
            cdf += p;
        }
        k
    }
}

fn simulate(truth: &PinParams, n: usize, seed: u64) -> Vec<DailyCounts> {
    let mut rng = Lcg(seed);
    (0..n)
        .map(|_| {
            let event = rng.uniform() < truth.alpha;
            let bad = rng.uniform() < truth.delta;
            let rb = truth.eps_b + if event && !bad { truth.mu } else { 0.0 };
            let rs = truth.eps_s + if event && bad { truth.mu } else { 0.0 };
            DailyCounts::new(rng.poisson(rb), rng.poisson(rs))
        })
        .collect()
}

#[test]
fn pin_recovers_simulated_parameters() {
    let truth = PinParams::new(0.3, 0.4, 60.0, 80.0, 70.0).unwrap();
    let days = simulate(&truth, 2_000, 7);
    let fit = PinEstimator::new().fit(&days).unwrap();
    let p = fit.params;
    assert!(fit.converged, "{fit:?}");
    assert!((p.alpha - 0.3).abs() < 0.03, "{p:?}");
    assert!((p.delta - 0.4).abs() < 0.05, "{p:?}");
    assert!((p.mu - 60.0).abs() < 3.0, "{p:?}");
    assert!((p.eps_b - 80.0).abs() < 1.5, "{p:?}");
    assert!((p.eps_s - 70.0).abs() < 1.5, "{p:?}");
    assert!(
        (fit.pin - truth.pin()).abs() < 0.01,
        "{} vs {}",
        fit.pin,
        truth.pin()
    );
    // An MLE scores at least as well as the parameters that generated the data.
    assert!(fit.log_likelihood >= truth.log_likelihood(&days));
}

#[test]
fn pin_fit_is_a_stationary_point() {
    // First-order conditions: the numerical gradient of the log-likelihood at the
    // fit is ~0 in every interior direction.
    let truth = PinParams::new(0.25, 0.6, 40.0, 50.0, 55.0).unwrap();
    let days = simulate(&truth, 500, 99);
    let fit = PinEstimator::new().fit(&days).unwrap();
    let p = fit.params;
    let ll = |q: PinParams| q.log_likelihood(&days);
    let h = 1e-6;
    let bump = |f: &dyn Fn(&mut PinParams, f64)| {
        let (mut up, mut dn) = (p, p);
        f(&mut up, h);
        f(&mut dn, -h);
        (ll(up) - ll(dn)) / (2.0 * h)
    };
    let grads = [
        bump(&|q, d| q.alpha += d),
        bump(&|q, d| q.delta += d),
        bump(&|q, d| q.mu += d),
        bump(&|q, d| q.eps_b += d),
        bump(&|q, d| q.eps_s += d),
    ];
    for g in grads {
        assert!(g.abs() < 1e-2, "gradient {grads:?} at {p:?}");
    }
}

#[test]
fn vpin_independent_of_how_trades_are_chunked() {
    // Pouring the same signed volume in one piece or many must give the same buckets.
    let mut rng = Lcg(5);
    let flow: Vec<(f64, f64)> = (0..3_000)
        .map(|_| {
            let v = 1.0 + (rng.uniform() * 40.0).floor();
            if rng.uniform() < 0.6 {
                (v, 0.0)
            } else {
                (0.0, v)
            }
        })
        .collect();
    let mut whole = Vpin::new(250.0, 20).unwrap();
    let mut split = Vpin::new(250.0, 20).unwrap();
    let (mut a, mut b) = (None, None);
    for &(buy, sell) in &flow {
        a = whole.update_classified(buy, sell);
        for _ in 0..4 {
            b = split.update_classified(buy / 4.0, sell / 4.0);
        }
    }
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!((a - b).abs() < 1e-9, "{a} vs {b}");
    assert_eq!(whole.completed_buckets(), split.completed_buckets());
}

#[test]
fn vpin_stays_in_unit_interval_and_resets() {
    let mut rng = Lcg(17);
    let mut v = Vpin::new(500.0, 30).unwrap();
    let mut p = 20.0;
    for _ in 0..50_000 {
        p += if rng.uniform() < 0.5 { 0.01 } else { -0.01 };
        if let Some(x) = v.update(p, 1.0 + rng.uniform() * 99.0) {
            assert!((0.0..=1.0).contains(&x));
        }
    }
    assert!(v.is_ready());
    // Random-walk tape with symmetric volume: low toxicity.
    assert!(v.value().unwrap() < 0.25, "{:?}", v.value());
    v.reset();
    assert!(!v.is_ready());
    assert_eq!(v.completed_buckets(), 0);
    assert_eq!(v.value(), None);
}

#[test]
fn vpin_ignores_bad_input() {
    let mut v = Vpin::new(10.0, 1).unwrap();
    v.update_side(Side::Buy, 4.0);
    assert_eq!(v.update_classified(f64::NAN, 1.0), None);
    assert_eq!(v.update_classified(-1.0, 1.0), None);
    assert_eq!(v.update(f64::NAN, 5.0), None);
    assert_eq!(v.update(10.0, -5.0), None);
    assert!((v.current_fill() - 0.4).abs() < 1e-12);
}

#[test]
fn bars_close_on_threshold() {
    let mut ticks = BarBuilder::new(BarKind::Tick(3)).unwrap();
    let out: Vec<_> = [1.0, 2.0, 3.0, 2.0, 1.0, 1.0]
        .iter()
        .filter_map(|&p| ticks.update(p, 1.0))
        .collect();
    assert_eq!(out.len(), 2);
    assert_eq!(
        (out[0].open, out[0].high, out[0].low, out[0].close),
        (1.0, 3.0, 1.0, 3.0)
    );
    assert_eq!((out[1].buy_volume, out[1].sell_volume), (0.0, 3.0));
    assert_eq!(out[1].imbalance(), -1.0);

    let mut dollars = BarBuilder::new(BarKind::Dollar(1_000.0)).unwrap();
    assert!(dollars.update(50.0, 10.0).is_none());
    let bar = dollars.update(60.0, 10.0).unwrap();
    assert_eq!(bar.notional, 1_100.0);
    assert!((bar.vwap() - 55.0).abs() < 1e-12);
    assert!(BarBuilder::new(BarKind::Volume(0.0)).is_err());
    assert!(BarBuilder::new(BarKind::Tick(0)).is_err());
}

#[cfg(feature = "ta")]
#[test]
fn ta_next_matches_update() {
    use ta::{DataItem, Next, Reset};
    let mut a = Vpin::with_bulk_classification(5_000.0, 10, 20).unwrap();
    let mut b = a.clone();
    let mut c = a.clone();
    let mut rng = Lcg(3);
    let mut close = 100.0;
    for _ in 0..500 {
        close += rng.uniform() - 0.45;
        let item = DataItem::builder()
            .open(close)
            .high(close + 1.0)
            .low(close - 1.0)
            .close(close)
            .volume(1_000.0)
            .build()
            .unwrap();
        let x = a.update(close, 1_000.0);
        assert_eq!(b.next((close, 1_000.0)), x);
        assert_eq!(c.next(&item), x);
    }
    assert!(a.is_ready());
    Reset::reset(&mut b);
    assert!(!b.is_ready());
}
