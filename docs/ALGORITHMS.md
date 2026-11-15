# Algorithms and conventions

Every formula flowtox computes, with the choices it makes where the literature
leaves room. Expected values used in `tests/reference.rs` are derived from these
definitions by hand.

## Trade signing

**Tick rule.** For trade `t` at price `p_t`: buy if `p_t > p_{t-1}`, sell if
`p_t < p_{t-1}`, and on a zero tick repeat the most recent non-zero tick. The
first trade, and zero ticks before any non-zero tick, have no side (`None`).

**Lee-Ready (1991).** With prevailing quote `(bid, ask)` and midpoint
`m = (bid + ask) / 2`: buy if `p > m`, sell if `p < m`, tick rule if `p = m`.
Quotes that are non-finite, non-positive or crossed (`ask < bid`) are ignored
and the tick rule decides. Pass the last quote strictly before the trade; the
five-second lag in the original paper was for 1980s NYSE reporting delays.

**Bulk volume classification (ELO 2012).** For bar `i` with close-to-close
change `dP_i` and volume `V_i`:

```text
V_i^B = V_i * Phi(dP_i / sigma)
V_i^S = V_i - V_i^B
```

`sigma` is the sample (n - 1) standard deviation of the last `w` price changes
including `dP_i` (pandas `close.diff().rolling(w).std()`), or a constant. If all
`w` changes are zero the bar is split 50/50. ELO (2012) use the normal CDF;
later work (Easley, Lopez de Prado, O'Hara 2016) also uses a Student-t, which
you can get by passing your own split to `Vpin::update_classified`.

## VPIN (ELO 2012)

Signed volume is poured into buckets of exactly `V` units. Inputs that cross a
bucket boundary are split, each part keeping the input's buy share. With
`n` full buckets in the window:

```text
VPIN = (1 / n) * sum_{tau = 1..n} |V_tau^B - V_tau^S| / V
```

`Vpin::update` returns `None` until `n` buckets are full, then the current
value after every input. Volume a classifier cannot sign (the first trade under
the tick rule, the first `w` bars under bulk classification) is not bucketed.
ELO pick `V = ADV / 50` and `n = 50`, so VPIN spans about one day of volume.

## PIN (EKOP 1996)

Day likelihood for `B` buys and `S` sells, `P(k; r) = e^{-r} r^k / k!`:

```text
L = (1 - a)       P(B; eb)      P(S; es)
  + a (1 - d)     P(B; eb + mu) P(S; es)
  + a d           P(B; eb)      P(S; es + mu)

PIN = a mu / (a mu + eb + es)
```

The three terms are combined in logs with log-sum-exp and the `k!` terms
dropped during optimization (they do not depend on the parameters). This is
the numerically safe form of the Easley, Hvidkjaer and O'Hara (2010)
factorization (Lin and Ke 2011): no factorial, power or exponential of a
day count is ever formed, so counts in the tens of thousands are fine.
`PinParams::log_likelihood` and `PinFit::log_likelihood` add the factorial
terms back, so they are true log-likelihoods.

Maximization is EM. E-step: posterior probabilities `q0, q1, q2` of no event,
good news and bad news for each day. M-step (with `mu`, `eb`, `es` the current
values, `N` days):

```text
a'  = sum(q1 + q2) / N
d'  = sum(q2) / sum(q1 + q2)
Ib  = sum(q1 B) * mu / (mu + eb)          informed buys on good-news days
Is  = sum(q2 S) * mu / (mu + es)          informed sells on bad-news days
mu' = (Ib + Is) / sum(q1 + q2)
eb' = (sum(B) - Ib) / N
es' = (sum(S) - Is) / N
```

EM never lowers the likelihood. It runs from each point of the Yan and Zhang
(2012) grid (`a, d, gamma` in `{0.1, 0.3, 0.5, 0.7, 0.9}`, with
`eb = gamma * mean(B)`, `mu = (mean(B) - eb) / (a (1 - d))`,
`es = mean(S) - a d mu`, starts with `es <= 0` skipped) until the
log-likelihood changes by less than `1e-12 * (1 + |LL|)`, and the best fit
wins.

On data with no informed-trading signal (for example every day identical) the
maximum is reached on the boundary and is not unique, so PIN is not identified;
check `PinFit::params` before reading too much into `PinFit::pin`.

## References

- Easley, D., Kiefer, N., O'Hara, M., Paperman, J. (1996). Liquidity, information, and infrequently traded stocks. *Journal of Finance* 51(4).
- Easley, D., Hvidkjaer, S., O'Hara, M. (2010). Factoring information into returns. *Journal of Financial and Quantitative Analysis* 45(2).
- Easley, D., Lopez de Prado, M., O'Hara, M. (2012). Flow toxicity and liquidity in a high-frequency world. *Review of Financial Studies* 25(5).
- Lee, C., Ready, M. (1991). Inferring trade direction from intraday data. *Journal of Finance* 46(2).
- Lin, H.-W., Ke, W.-C. (2011). A computing bias in estimating the probability of informed trading. *Journal of Financial Markets* 14(4).
- Yan, Y., Zhang, S. (2012). An improved estimation method and empirical properties of the probability of informed trading. *Journal of Banking and Finance* 36(2).
- Lopez de Prado, M. (2018). *Advances in Financial Machine Learning*. Wiley.
- West, G. (2005). Better approximations to cumulative normal functions. *Wilmott Magazine*.
