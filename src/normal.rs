//! Standard normal cumulative distribution function.

/// `Phi(x)`, the standard normal CDF. Absolute error is below 1e-14 everywhere;
/// for `|x| > 7.07` (probabilities under 1e-12) the relative error is near 1e-8.
///
/// Hart's (1968) double-precision rational approximation, in the form given by
/// G. West, "Better approximations to cumulative normal functions" (Wilmott, 2005).
///
/// ```
/// let p = flowtox::normal_cdf(1.0);
/// assert!((p - 0.841_344_746_068_542_9).abs() < 1e-14);
/// ```
pub fn normal_cdf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let z = x.abs();
    let tail = if z > 37.0 {
        0.0
    } else {
        let e = (-0.5 * z * z).exp();
        if z < 7.071_067_811_865_47 {
            let mut num = 3.526_249_659_989_11e-2 * z + 0.700_383_064_443_688;
            num = num * z + 6.373_962_203_531_65;
            num = num * z + 33.912_866_078_383;
            num = num * z + 112.079_291_497_871;
            num = num * z + 221.213_596_169_931;
            num = num * z + 220.206_867_912_376;
            let mut den = 8.838_834_764_831_84e-2 * z + 1.755_667_163_182_64;
            den = den * z + 16.064_177_579_207;
            den = den * z + 86.780_732_202_946_1;
            den = den * z + 296.564_248_779_674;
            den = den * z + 637.333_633_378_831;
            den = den * z + 793.826_512_519_948;
            den = den * z + 440.413_735_824_752;
            e * num / den
        } else {
            let mut b = z + 0.65;
            b = z + 4.0 / b;
            b = z + 3.0 / b;
            b = z + 2.0 / b;
            b = z + 1.0 / b;
            e / b / 2.506_628_274_631
        }
    };
    if x > 0.0 {
        1.0 - tail
    } else {
        tail
    }
}
