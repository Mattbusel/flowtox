//! Fit the EKOP PIN model to daily buy/sell counts.
//!
//! ```text
//! cargo run --example pin_fit
//! ```
//!
//! The counts are simulated from known parameters with a seeded Poisson sampler,
//! so you can see how close the fit gets to the truth.

use flowtox::{DailyCounts, PinEstimator, PinParams};

struct Lcg(u64);
impl Lcg {
    fn uniform(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    /// Poisson draw by inversion (fine for the rates used here).
    fn poisson(&mut self, lambda: f64) -> u64 {
        let u = self.uniform();
        let (mut k, mut p) = (0u64, (-lambda).exp());
        let mut cdf = p;
        while u > cdf && k < 10_000 {
            k += 1;
            p *= lambda / k as f64;
            cdf += p;
        }
        k
    }
}

fn main() -> Result<(), flowtox::Error> {
    let truth = PinParams::new(0.3, 0.4, 60.0, 80.0, 70.0)?;
    let mut rng = Lcg(2024);
    let days: Vec<DailyCounts> = (0..500)
        .map(|_| {
            let event = rng.uniform() < truth.alpha;
            let bad = rng.uniform() < truth.delta;
            let rb = truth.eps_b + if event && !bad { truth.mu } else { 0.0 };
            let rs = truth.eps_s + if event && bad { truth.mu } else { 0.0 };
            DailyCounts::new(rng.poisson(rb), rng.poisson(rs))
        })
        .collect();

    let fit = PinEstimator::new().fit(&days)?;
    let p = fit.params;
    println!("            alpha  delta     mu  eps_b  eps_s    PIN");
    println!(
        "truth      {:>5.3}  {:>5.3} {:>6.2} {:>6.2} {:>6.2}  {:>5.3}",
        truth.alpha,
        truth.delta,
        truth.mu,
        truth.eps_b,
        truth.eps_s,
        truth.pin()
    );
    println!(
        "estimate   {:>5.3}  {:>5.3} {:>6.2} {:>6.2} {:>6.2}  {:>5.3}",
        p.alpha, p.delta, p.mu, p.eps_b, p.eps_s, fit.pin
    );
    println!(
        "log-likelihood {:.3}, {} starts, {} EM iterations, converged: {}",
        fit.log_likelihood, fit.starts, fit.iterations, fit.converged
    );
    Ok(())
}
