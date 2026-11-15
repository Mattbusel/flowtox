//! Probability of informed trading (PIN): the Easley, Kiefer, O'Hara and
//! Paperman (1996) model, fitted by maximum likelihood.
//!
//! Each day, with probability `alpha` an information event occurs; it is bad
//! news with probability `delta`. Uninformed buys and sells arrive as Poisson
//! processes with rates `eps_b` and `eps_s`; on event days informed traders add
//! rate `mu` to the buy side (good news) or the sell side (bad news). For a day
//! with `B` buys and `S` sells:
//!
//! ```text
//! L(B, S) = (1 - alpha)         P(B; eps_b)      P(S; eps_s)
//!         + alpha (1 - delta)   P(B; eps_b + mu) P(S; eps_s)
//!         + alpha delta         P(B; eps_b)      P(S; eps_s + mu)
//!
//! PIN = alpha mu / (alpha mu + eps_b + eps_s)
//! ```
//!
//! The likelihood is evaluated in logs with the factorials dropped and the three
//! regimes combined by log-sum-exp, the overflow-free form of the Easley,
//! Hvidkjaer and O'Hara (2010) factorization (Lin and Ke, 2011), so day counts in
//! the tens of thousands are fine. It is maximized by expectation-maximization
//! (which never decreases the likelihood) from the Yan and Zhang (2012) grid of
//! starting points, keeping the best fit; this avoids the boundary solutions a
//! single start often finds.

use crate::error::{invalid, Error, Result};

/// Buy and sell trade counts for one trading day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DailyCounts {
    /// Number of buyer-initiated trades.
    pub buys: u64,
    /// Number of seller-initiated trades.
    pub sells: u64,
}

impl DailyCounts {
    /// A day with `buys` buys and `sells` sells.
    pub fn new(buys: u64, sells: u64) -> Self {
        Self { buys, sells }
    }
}

/// Parameters of the EKOP model.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PinParams {
    /// Probability of an information event on a given day.
    pub alpha: f64,
    /// Probability that an information event is bad news.
    pub delta: f64,
    /// Arrival rate of informed trades on event days.
    pub mu: f64,
    /// Arrival rate of uninformed buys.
    pub eps_b: f64,
    /// Arrival rate of uninformed sells.
    pub eps_s: f64,
}

impl PinParams {
    /// Validated parameters: `alpha` and `delta` in `[0, 1]`, rates finite and positive.
    pub fn new(alpha: f64, delta: f64, mu: f64, eps_b: f64, eps_s: f64) -> Result<Self> {
        let unit = |x: f64| (0.0..=1.0).contains(&x);
        let rate = |x: f64| x.is_finite() && x > 0.0;
        if !unit(alpha) {
            return Err(invalid("alpha", "must be in [0, 1]"));
        }
        if !unit(delta) {
            return Err(invalid("delta", "must be in [0, 1]"));
        }
        if !rate(mu) {
            return Err(invalid("mu", "must be finite and positive"));
        }
        if !rate(eps_b) {
            return Err(invalid("eps_b", "must be finite and positive"));
        }
        if !rate(eps_s) {
            return Err(invalid("eps_s", "must be finite and positive"));
        }
        Ok(Self {
            alpha,
            delta,
            mu,
            eps_b,
            eps_s,
        })
    }

    /// `alpha mu / (alpha mu + eps_b + eps_s)`.
    pub fn pin(&self) -> f64 {
        let informed = self.alpha * self.mu;
        let denom = informed + self.eps_b + self.eps_s;
        if denom > 0.0 {
            informed / denom
        } else {
            0.0
        }
    }

    /// Full log-likelihood of `days`, factorial terms included.
    pub fn log_likelihood(&self, days: &[DailyCounts]) -> f64 {
        let k = Kernel::new(self);
        days.iter()
            .map(|d| k.day(d).0 - ln_gamma(d.buys as f64 + 1.0) - ln_gamma(d.sells as f64 + 1.0))
            .sum()
    }
}

/// Precomputed logs for one parameter vector.
struct Kernel {
    ln_no: f64,
    ln_good: f64,
    ln_bad: f64,
    ln_eb: f64,
    ln_es: f64,
    ln_ebmu: f64,
    ln_esmu: f64,
    eb: f64,
    es: f64,
    mu: f64,
}

impl Kernel {
    fn new(p: &PinParams) -> Self {
        Self {
            ln_no: (1.0 - p.alpha).ln(),
            ln_good: (p.alpha * (1.0 - p.delta)).ln(),
            ln_bad: (p.alpha * p.delta).ln(),
            ln_eb: p.eps_b.ln(),
            ln_es: p.eps_s.ln(),
            ln_ebmu: (p.eps_b + p.mu).ln(),
            ln_esmu: (p.eps_s + p.mu).ln(),
            eb: p.eps_b,
            es: p.eps_s,
            mu: p.mu,
        }
    }

    /// Log-likelihood of one day without factorials, and the posterior
    /// probabilities of (no event, good news, bad news).
    fn day(&self, d: &DailyCounts) -> (f64, [f64; 3]) {
        let b = d.buys as f64;
        let s = d.sells as f64;
        let base = -self.eb - self.es;
        let l0 = self.ln_no + base + b * self.ln_eb + s * self.ln_es;
        let l1 = self.ln_good + base - self.mu + b * self.ln_ebmu + s * self.ln_es;
        let l2 = self.ln_bad + base - self.mu + b * self.ln_eb + s * self.ln_esmu;
        let m = l0.max(l1).max(l2);
        let (e0, e1, e2) = ((l0 - m).exp(), (l1 - m).exp(), (l2 - m).exp());
        let z = e0 + e1 + e2;
        (m + z.ln(), [e0 / z, e1 / z, e2 / z])
    }
}

/// The result of [`PinEstimator::fit`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PinFit {
    /// Maximum-likelihood parameters.
    pub params: PinParams,
    /// PIN implied by `params`.
    pub pin: f64,
    /// Log-likelihood at `params`, factorial terms included.
    pub log_likelihood: f64,
    /// EM iterations used by the winning start.
    pub iterations: usize,
    /// Whether the winning start met the tolerance before the iteration cap.
    pub converged: bool,
    /// Number of starting points tried.
    pub starts: usize,
}

/// Maximum-likelihood estimator for the EKOP PIN model.
///
/// ```
/// use flowtox::{DailyCounts, PinEstimator};
/// // Quiet days with balanced flow, and every fifth day an informed buy surge.
/// let days: Vec<DailyCounts> = (0..60)
///     .map(|i| if i % 5 == 0 { DailyCounts::new(90, 41) } else { DailyCounts::new(40, 39) })
///     .collect();
/// let fit = PinEstimator::new().fit(&days).unwrap();
/// assert!((fit.params.alpha - 0.2).abs() < 0.01);
/// assert!(fit.pin > 0.05 && fit.pin < 0.15);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PinEstimator {
    max_iterations: usize,
    tolerance: f64,
}

impl Default for PinEstimator {
    fn default() -> Self {
        Self {
            max_iterations: 5_000,
            tolerance: 1e-12,
        }
    }
}

impl PinEstimator {
    /// Defaults: up to 5000 EM iterations per start, relative log-likelihood tolerance 1e-12.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cap on EM iterations per starting point.
    pub fn max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n.max(1);
        self
    }

    /// Stop when the log-likelihood changes by less than `tol * (1 + |LL|)`.
    pub fn tolerance(mut self, tol: f64) -> Self {
        self.tolerance = if tol.is_finite() && tol > 0.0 {
            tol
        } else {
            1e-12
        };
        self
    }

    /// Fit the model to `days` (at least 2 days, with at least one buy and one sell overall).
    pub fn fit(&self, days: &[DailyCounts]) -> Result<PinFit> {
        if days.len() < 2 {
            return Err(Error::InsufficientData {
                needed: 2,
                got: days.len(),
            });
        }
        let n = days.len() as f64;
        let mean_b = days.iter().map(|d| d.buys as f64).sum::<f64>() / n;
        let mean_s = days.iter().map(|d| d.sells as f64).sum::<f64>() / n;
        if mean_b <= 0.0 || mean_s <= 0.0 {
            return Err(invalid(
                "days",
                "need at least one buy and one sell in total",
            ));
        }

        const GRID: [f64; 5] = [0.1, 0.3, 0.5, 0.7, 0.9];
        let mut best: Option<(f64, PinParams, usize, bool)> = None;
        let mut starts = 0;
        for &a in &GRID {
            for &d in &GRID {
                for &g in &GRID {
                    let eb = g * mean_b;
                    let mu = (mean_b - eb) / (a * (1.0 - d));
                    let es = mean_s - a * d * mu;
                    if !(es > 0.0 && mu > 0.0) {
                        continue;
                    }
                    starts += 1;
                    let start = PinParams {
                        alpha: a,
                        delta: d,
                        mu,
                        eps_b: eb,
                        eps_s: es,
                    };
                    let (ll, p, it, conv) = self.em(days, start);
                    if ll.is_finite() && best.map_or(true, |b| ll > b.0) {
                        best = Some((ll, p, it, conv));
                    }
                }
            }
        }
        let (_, params, iterations, converged) = best.ok_or(Error::InsufficientData {
            needed: 2,
            got: days.len(),
        })?;
        Ok(PinFit {
            params,
            pin: params.pin(),
            log_likelihood: params.log_likelihood(days),
            iterations,
            converged,
            starts,
        })
    }

    /// EM from one start. Returns (LL without factorials, params, iterations, converged).
    fn em(&self, days: &[DailyCounts], mut p: PinParams) -> (f64, PinParams, usize, bool) {
        const FLOOR: f64 = 1e-12;
        let n = days.len() as f64;
        let mut ll_prev = f64::NEG_INFINITY;
        for it in 1..=self.max_iterations {
            let k = Kernel::new(&p);
            let (mut ll, mut s1, mut s2) = (0.0, 0.0, 0.0);
            let (mut b_plain, mut b_good, mut s_plain, mut s_bad) = (0.0, 0.0, 0.0, 0.0);
            for d in days {
                let (l, [q0, q1, q2]) = k.day(d);
                let (b, s) = (d.buys as f64, d.sells as f64);
                ll += l;
                s1 += q1;
                s2 += q2;
                b_plain += (q0 + q2) * b;
                b_good += q1 * b;
                s_plain += (q0 + q1) * s;
                s_bad += q2 * s;
            }
            if (ll - ll_prev).abs() <= self.tolerance * (1.0 + ll.abs()) {
                return (ll, p, it, true);
            }
            ll_prev = ll;

            // M-step. On good-news days buys are a Poisson superposition of uninformed
            // (eps_b) and informed (mu) arrivals; split them by their rate shares.
            let event = s1 + s2;
            let inf_b = b_good * p.mu / (p.mu + p.eps_b);
            let inf_s = s_bad * p.mu / (p.mu + p.eps_s);
            let mut next = p;
            next.alpha = (event / n).clamp(0.0, 1.0);
            if event > FLOOR {
                next.delta = (s2 / event).clamp(0.0, 1.0);
                next.mu = ((inf_b + inf_s) / event).max(FLOOR);
            }
            next.eps_b = ((b_plain + b_good - inf_b) / n).max(FLOOR);
            next.eps_s = ((s_plain + s_bad - inf_s) / n).max(FLOOR);
            p = next;
        }
        let ll = days.iter().map(|d| Kernel::new(&p).day(d).0).sum();
        (ll, p, self.max_iterations, false)
    }
}

/// `ln Gamma(x)` for `x > 0` (Lanczos, g = 7, n = 9; relative error near 1e-15).
pub(crate) fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // Reflection: Gamma(x) Gamma(1 - x) = pi / sin(pi x).
        let pi = core::f64::consts::PI;
        return (pi / (pi * x).sin()).abs().ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    for (i, &c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    let t = x + G + 0.5;
    0.5 * (2.0 * core::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ln_gamma_factorials() {
        let mut f = 1.0_f64;
        for k in 1..30u32 {
            f *= k as f64;
            let rel = (ln_gamma(k as f64 + 1.0) - f.ln()).abs() / f.ln().max(1.0);
            assert!(rel < 1e-13, "k={k} rel={rel}");
        }
        assert!(ln_gamma(1.0).abs() < 1e-14);
        assert!(ln_gamma(2.0).abs() < 1e-14);
    }
}
