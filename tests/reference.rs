//! Numeric reference checks. Every expected value below is worked out by hand
//! from the definitions in the papers (the arithmetic is in the comments), not
//! taken from flowtox's own output. Normal CDF values are from
//! `0.5 * erfc(-z / sqrt(2))`.

use flowtox::{
    batch, normal_cdf, BulkVolumeClassifier, DailyCounts, LeeReady, PinEstimator, PinParams, Side,
    TickRule, Vpin,
};

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn normal_cdf_table_values() {
    let table = [
        (0.0, 0.5),
        (1.0, 0.841_344_746_068_542_8),
        (-1.96, 0.024_997_895_148_220_435),
        (3.0, 0.998_650_101_968_369_9),
        (-0.5, 0.308_537_538_725_986_94),
        (0.25, 0.598_706_325_682_923_7),
        (-7.5, 3.190_891_672_910_92e-14),
    ];
    for (z, p) in table {
        let got = normal_cdf(z);
        // Absolute error under 1e-14 everywhere; in the far tail (|z| > 7.07,
        // Hart's continued-fraction branch) the relative error is near 1e-8.
        let tol: f64 = if z < -7.0 { p * 1e-7 } else { 1e-14 };
        assert!(close(got, p, tol), "Phi({z}) = {got}, want {p}");
    }
    assert_eq!(normal_cdf(40.0), 1.0);
    assert_eq!(normal_cdf(-40.0), 0.0);
    assert!(normal_cdf(f64::NAN).is_nan());
}

#[test]
fn tick_rule_sequence() {
    // 10.0 (no history), 10.1 up, 10.1 zero tick after an uptick, 10.0 down,
    // 10.0 zero tick after a downtick, 10.2 up.
    let mut tr = TickRule::new();
    let sides: Vec<_> = [10.0, 10.1, 10.1, 10.0, 10.0, 10.2]
        .iter()
        .map(|&p| tr.classify(p))
        .collect();
    use Side::*;
    assert_eq!(
        sides,
        vec![
            None,
            Some(Buy),
            Some(Buy),
            Some(Sell),
            Some(Sell),
            Some(Buy)
        ]
    );
    // A tie straight after the first trade has no prior sign to repeat.
    let mut tr = TickRule::new();
    assert_eq!(tr.classify(5.0), None);
    assert_eq!(tr.classify(5.0), None);
    assert!(!tr.is_ready());
    assert_eq!(tr.update(5.5, 3.0), Some(3.0));
    assert!(tr.is_ready());
}

#[test]
fn lee_ready_quote_then_tick() {
    // Quote 9.98 / 10.02, mid 10.00.
    let mut lr = LeeReady::new();
    assert_eq!(lr.classify(10.01, 9.98, 10.02), Some(Side::Buy)); // above mid
    assert_eq!(lr.classify(9.99, 9.98, 10.02), Some(Side::Sell)); // below mid
                                                                  // At the mid: tick rule, 9.99 -> 10.00 is an uptick.
    assert_eq!(lr.classify(10.00, 9.98, 10.02), Some(Side::Buy));
    // At the mid again with no price change: zero tick repeats Buy.
    assert_eq!(lr.classify(10.00, 9.98, 10.02), Some(Side::Buy));
    // Crossed quote is ignored: tick rule, 10.00 -> 9.97 is a downtick.
    assert_eq!(lr.classify(9.97, 10.05, 9.95), Some(Side::Sell));
    // A trade above the ask is still above the mid: buy.
    assert_eq!(lr.update(10.03, 7.0, 9.98, 10.02), Some(7.0));

    let sides = batch::lee_ready(&[1.0, 1.1], &[0.9, 1.1], &[1.0, 1.3]).unwrap(); // mids 0.95, 1.2
    assert_eq!(sides, vec![Some(Side::Buy), Some(Side::Sell)]);
    assert!(batch::lee_ready(&[1.0], &[0.9, 1.0], &[1.1]).is_err());
}

#[test]
fn bulk_volume_classification_hand_values() {
    // Closes 100, 101, 99, 100 -> changes +1, -2, +1. Window 2, sample sd (n - 1):
    //   bar 3: changes {+1, -2}, mean -0.5, var = (1.5^2 + 1.5^2) / 1 = 4.5, sd = 2.12132034
    //          z = -2 / 2.12132034 = -0.94280904, Phi(z) = 0.17288929307558012
    //   bar 4: changes {-2, +1}, same sd, z = 0.47140452, Phi(z) = 0.6813240558830315
    let mut bvc = BulkVolumeClassifier::new(2).unwrap();
    assert_eq!(bvc.buy_fraction(100.0), None);
    assert!(!bvc.is_ready());
    assert_eq!(bvc.buy_fraction(101.0), None); // one change, window needs two
    assert!(bvc.is_ready());
    let f3 = bvc.split(99.0, 1_000.0).unwrap();
    assert!(close(f3.0, 172.889_293_075_580_12, 1e-9), "{f3:?}");
    assert!(close(f3.1, 827.110_706_924_419_9, 1e-9), "{f3:?}");
    let signed = bvc.update(100.0, 1_000.0).unwrap();
    // 1000 * (2 * 0.6813240558830315 - 1) = 362.648111766063
    assert!(close(signed, 362.648_111_766_063, 1e-9), "{signed}");

    let b = batch::bulk_buy_fractions(&[100.0, 101.0, 99.0, 100.0], 2).unwrap();
    assert_eq!(b[0], None);
    assert_eq!(b[1], None);
    assert!(close(b[2].unwrap(), 0.172_889_293_075_580_12, 1e-12));
    assert!(close(b[3].unwrap(), 0.681_324_055_883_031_5, 1e-12));

    // Fixed sigma 0.5: change +0.125 -> z = 0.25 -> Phi = 0.5987063256829237.
    let mut fixed = BulkVolumeClassifier::with_sigma(0.5).unwrap();
    assert_eq!(fixed.buy_fraction(10.0), None);
    assert!(close(
        fixed.buy_fraction(10.125).unwrap(),
        0.598_706_325_682_923_7,
        1e-14
    ));

    // Flat window: split evenly.
    let mut flat = BulkVolumeClassifier::new(3).unwrap();
    for _ in 0..3 {
        flat.buy_fraction(7.0);
    }
    assert_eq!(flat.buy_fraction(7.0), Some(0.5));
}

#[test]
fn vpin_tick_rule_hand_trace() {
    // V = 10, n = 2.
    //   (100, 4): first trade, cannot be signed, not bucketed
    //   (101, 6): buy 6                      bucket A = B6
    //   (100, 8): sell 8: 4 close A (B6 S4), |6 - 4| / 10 = 0.2; 4 carry: bucket B = S4
    //   (100, 6): zero tick, sell 6: B = S10, closes, |0 - 10| / 10 = 1.0
    //             VPIN = (0.2 + 1.0) / 2 = 0.6
    //   (102, 15): buy 15: 10 close C (B10), 1.0; window {B, C} -> VPIN 1.0; 5 carry
    let mut v = Vpin::new(10.0, 2).unwrap();
    let out: Vec<_> = [
        (100.0, 4.0),
        (101.0, 6.0),
        (100.0, 8.0),
        (100.0, 6.0),
        (102.0, 15.0),
    ]
    .iter()
    .map(|&(p, q)| v.update(p, q))
    .collect();
    assert_eq!(out[..3], [None, None, None]);
    assert!(close(out[3].unwrap(), 0.6, 1e-12));
    assert!(close(out[4].unwrap(), 1.0, 1e-12));
    assert_eq!(v.completed_buckets(), 3);
    assert!(close(v.current_fill(), 0.5, 1e-12));
    let window: Vec<f64> = v.bucket_imbalances().collect();
    assert_eq!(window, vec![1.0, 1.0]);

    // Same tape through the batch function.
    let b = batch::vpin(
        &[100.0, 101.0, 100.0, 100.0, 102.0],
        &[4.0, 6.0, 8.0, 6.0, 15.0],
        10.0,
        2,
    )
    .unwrap();
    assert_eq!(b, out);
}

#[test]
fn vpin_balanced_alternating_flow() {
    // srfm-lab's trade_flow_analysis::test_vpin_mixed fed 10 buys and 10 sells,
    // alternating, into 4 buckets of 5 units and expected VPIN near 0. Each bucket
    // of 5 alternating unit trades holds 3 of one side and 2 of the other, so the
    // exact VPIN is |3 - 2| / 5 = 0.2: the expectation was wrong, not the sum.
    let mut v = Vpin::new(5.0, 4).unwrap();
    let mut last = None;
    for _ in 0..10 {
        v.update_side(Side::Buy, 1.0);
        last = v.update_side(Side::Sell, 1.0);
    }
    assert!(close(last.unwrap(), 0.2, 1e-12), "{last:?}");

    // With an even bucket (4 units, 5 buckets) every bucket is 2 vs 2: VPIN = 0.
    let mut v = Vpin::new(4.0, 5).unwrap();
    let mut last = None;
    for _ in 0..10 {
        v.update_side(Side::Buy, 1.0);
        last = v.update_side(Side::Sell, 1.0);
    }
    assert!(close(last.unwrap(), 0.0, 1e-12), "{last:?}");

    // One-sided flow: VPIN = 1.
    let mut v = Vpin::new(3.0, 5).unwrap();
    let mut last = None;
    for _ in 0..20 {
        last = v.update_side(Side::Sell, 1.0);
    }
    assert!(close(last.unwrap(), 1.0, 1e-12));
}

#[test]
fn vpin_bulk_split_across_buckets() {
    // A bulk-classified bar of 25 units with buy share 0.8 into 10-unit buckets:
    // two full buckets of 8 / 2 (imbalance 0.6 each), 5 units carried at 4 / 1.
    let mut v = Vpin::new(10.0, 2).unwrap();
    let out = v.update_classified(20.0, 5.0);
    assert!(close(out.unwrap(), 0.6, 1e-12));
    // 15 more units, all sells: 5 close the carried bucket (4 / 6 -> 0.2),
    // 10 fill the next (0 / 10 -> 1.0). Window {0.2, 1.0} -> 0.6.
    let out = v.update_classified(0.0, 15.0);
    assert!(close(out.unwrap(), 0.6, 1e-12));
    let w: Vec<f64> = v.bucket_imbalances().collect();
    assert!(close(w[0], 0.2, 1e-12) && close(w[1], 1.0, 1e-12), "{w:?}");
}

#[test]
fn vpin_bulk_mode_hand_trace() {
    // Bulk mode, V = 1000, n = 1, sigma window 2, bars of 1000 units.
    // Closes 100, 101, 99, 100: the first two bars warm the classifier up,
    // bar 3 has buy share 0.17288929 (see the BVC test): |2 * 0.17288929 - 1| = 0.65422141
    // bar 4 has buy share 0.68132406:                     |2 * 0.68132406 - 1| = 0.36264811
    let r = batch::vpin_bulk(&[100.0, 101.0, 99.0, 100.0], &[1_000.0; 4], 1_000.0, 1, 2).unwrap();
    assert_eq!(r[..2], [None, None]);
    assert!(
        close(r[2].unwrap(), 0.654_221_413_848_839_8, 1e-12),
        "{r:?}"
    );
    assert!(close(r[3].unwrap(), 0.362_648_111_766_063, 1e-12), "{r:?}");
}

#[test]
fn pin_log_likelihood_one_day_by_hand() {
    // alpha = 0.5, delta = 0.5, mu = 2, eps_b = eps_s = 1, one day with B = 3, S = 1:
    //   no event : 0.50 * e^-1 1^3 / 3! * e^-1 1^1 / 1!  = e^-2 / 12
    //   good news: 0.25 * e^-3 3^3 / 3! * e^-1 1^1 / 1!  = 27/24 e^-4
    //   bad news : 0.25 * e^-1 1^3 / 3! * e^-3 3^1 / 1!  =  3/24 e^-4
    //   L = e^-2 / 12 + 1.25 e^-4,  ln L = -3.376334377326781
    //   PIN = 0.5 * 2 / (0.5 * 2 + 1 + 1) = 1/3
    let p = PinParams::new(0.5, 0.5, 2.0, 1.0, 1.0).unwrap();
    let ll = p.log_likelihood(&[DailyCounts::new(3, 1)]);
    assert!(close(ll, -3.376_334_377_326_781, 1e-12), "{ll}");
    assert!(close(p.pin(), 1.0 / 3.0, 1e-15));
    assert!(PinParams::new(1.2, 0.5, 1.0, 1.0, 1.0).is_err());
    assert!(PinParams::new(0.2, 0.5, 0.0, 1.0, 1.0).is_err());
}

#[test]
fn pin_fit_identical_days_reaches_poisson_bound() {
    // Every day is (B, S) = (30, 20). Each mixture component is a product of two
    // Poisson pmfs, and a Poisson pmf at k is largest when its rate is k, so no
    // parameter vector can beat 40 * [ln P(30; 30) + ln P(20; 20)]. (The maximum
    // is not unique: alpha = 0, or good news every day with eps_b + mu = 30, both
    // attain it, so PIN itself is not identified on such data.)
    let days = vec![DailyCounts::new(30, 20); 40];
    let fit = PinEstimator::new().fit(&days).unwrap();
    // The fitted day rates reproduce the sample means.
    let p = fit.params;
    let mean_b =
        (1.0 - p.alpha * (1.0 - p.delta)) * p.eps_b + p.alpha * (1.0 - p.delta) * (p.eps_b + p.mu);
    let mean_s = (1.0 - p.alpha * p.delta) * p.eps_s + p.alpha * p.delta * (p.eps_s + p.mu);
    assert!(
        close(mean_b, 30.0, 1e-3) && close(mean_s, 20.0, 1e-3),
        "{p:?}"
    );
    // And the maximum is the Poisson(30) x Poisson(20) likelihood of 40 such days.
    let poisson_ll =
        |k: f64, l: f64| k * l.ln() - l - (1..=k as u64).map(|i| (i as f64).ln()).sum::<f64>();
    let want = 40.0 * (poisson_ll(30.0, 30.0) + poisson_ll(20.0, 20.0));
    assert!(
        close(fit.log_likelihood, want, 1e-6),
        "{} vs {want}",
        fit.log_likelihood
    );
}

#[test]
fn errors_are_reported() {
    assert!(Vpin::new(0.0, 50).is_err());
    assert!(Vpin::new(f64::NAN, 50).is_err());
    assert!(Vpin::new(100.0, 0).is_err());
    assert!(Vpin::with_bulk_classification(100.0, 10, 1).is_err());
    assert!(BulkVolumeClassifier::with_sigma(-1.0).is_err());
    assert!(PinEstimator::new().fit(&[DailyCounts::new(1, 1)]).is_err());
    assert!(PinEstimator::new()
        .fit(&[DailyCounts::new(0, 4), DailyCounts::new(0, 2)])
        .is_err());
    let e = Vpin::new(0.0, 1).unwrap_err();
    assert_eq!(
        e.to_string(),
        "invalid parameter `bucket_volume`: must be finite and positive"
    );
}
