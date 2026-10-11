//! Closed-form queuing formulas.
//!
//! All functions take rates in *any* consistent time unit (e.g. jobs per
//! second) and return times in the reciprocal unit. Every formula validates
//! its inputs and refuses unstable systems ([`QueueError::Unstable`]).

use crate::error::QueueError;

/// Results for an M/M/1 queue.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Mm1Result {
    /// Utilization `ρ = λ/μ`.
    pub rho: f64,
    /// Mean number in system `L`.
    pub l: f64,
    /// Mean queue length `Lq`.
    pub lq: f64,
    /// Mean sojourn time `W`.
    pub w: f64,
    /// Mean waiting time in queue `Wq`.
    pub wq: f64,
    /// Probability the system is empty, `1 − ρ`.
    pub p0: f64,
}

/// Steady-state metrics of an M/M/1 queue with arrival rate `lambda` and
/// service rate `mu` (both > 0, `lambda < mu`).
///
/// # Examples
///
/// ```
/// use tpt_des_queue::mm1;
///
/// let r = mm1(0.75, 1.0).unwrap();
/// assert!((r.rho - 0.75).abs() < 1e-12);
/// assert!((r.l - 3.0).abs() < 1e-12);        // L = ρ/(1−ρ)
/// assert!((r.w - 4.0).abs() < 1e-12);        // W = 1/(μ−λ)
/// ```
pub fn mm1(lambda: f64, mu: f64) -> Result<Mm1Result, QueueError> {
    if lambda <= 0.0 || mu <= 0.0 || !lambda.is_finite() || !mu.is_finite() {
        return Err(QueueError::InvalidParameter);
    }
    if lambda >= mu {
        return Err(QueueError::Unstable);
    }
    let rho = lambda / mu;
    let one_minus = 1.0 - rho;
    let l = rho / one_minus;
    let lq = rho * rho / one_minus;
    let w = 1.0 / (mu - lambda);
    let wq = rho / (mu - lambda);
    Ok(Mm1Result {
        rho,
        l,
        lq,
        w,
        wq,
        p0: one_minus,
    })
}

/// Steady-state probability of `n` jobs in an M/M/1 queue: `P_n = (1−ρ)·ρⁿ`.
pub fn mm1_pn(lambda: f64, mu: f64, n: u32) -> Result<f64, QueueError> {
    let r = mm1(lambda, mu)?;
    // powf rather than powi: `n as i32` wraps for n > i32::MAX.
    Ok(r.p0 * r.rho.powf(n as f64))
}

/// Results for an M/M/c queue with `c` identical servers.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct MmcResult {
    /// Per-server utilization `ρ = a/c` with offered load `a = λ/μ`.
    pub rho: f64,
    /// Offered load `a = λ/μ` (in erlangs).
    pub offered_load: f64,
    /// Mean number in system `L`.
    pub l: f64,
    /// Mean queue length `Lq`.
    pub lq: f64,
    /// Mean sojourn time `W`.
    pub w: f64,
    /// Mean waiting time in queue `Wq`.
    pub wq: f64,
    /// Probability the system is empty.
    pub p0: f64,
    /// Probability an arrival must wait (Erlang C formula).
    pub erlang_c: f64,
}

/// Steady-state metrics of an M/M/c queue: `c ≥ 1` servers, arrival rate
/// `lambda`, per-server rate `mu`, with `λ < c·μ`.
///
/// # Examples
///
/// ```
/// use tpt_des_queue::mmc;
///
/// // λ = 1.6, μ = 1, c = 2: a known textbook configuration.
/// let r = mmc(1.6, 1.0, 2).unwrap();
/// assert!((r.rho - 0.8).abs() < 1e-12);
/// assert!(r.erlang_c > 0.0 && r.erlang_c < 1.0);
/// assert!((r.l - (r.lq + 1.6)).abs() < 1e-9); // L = Lq + a
/// ```
pub fn mmc(lambda: f64, mu: f64, c: u32) -> Result<MmcResult, QueueError> {
    if lambda <= 0.0 || mu <= 0.0 || c == 0 || !lambda.is_finite() || !mu.is_finite() {
        return Err(QueueError::InvalidParameter);
    }
    let a = lambda / mu;
    if a >= c as f64 {
        return Err(QueueError::Unstable);
    }
    let rho = a / c as f64;
    let (erlang_c, p0) = erlang_c_and_p0(a, c);
    let lq = erlang_c * rho / (1.0 - rho);
    let l = lq + a;
    let wq = lq / lambda;
    let w = wq + 1.0 / mu;
    Ok(MmcResult {
        rho,
        offered_load: a,
        l,
        lq,
        w,
        wq,
        p0,
        erlang_c,
    })
}

/// Results for an M/G/1 queue (Pollaczek–Khinchine).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Mg1Result {
    /// Utilization `ρ = λ·E[S]`.
    pub rho: f64,
    /// Mean number in system `L`.
    pub l: f64,
    /// Mean queue length `Lq`.
    pub lq: f64,
    /// Mean sojourn time `W`.
    pub w: f64,
    /// Mean waiting time in queue `Wq`.
    pub wq: f64,
}

/// Steady-state metrics of an M/G/1 queue via the Pollaczek–Khinchine
/// formula, from arrival rate `lambda`, mean service time `mean_service`
/// `E[S]`, and second moment of service `second_moment` `E[S²]`.
///
/// # Examples
///
/// ```
/// use tpt_des_queue::mg1;
///
/// // For M/M/1 (exponential service), E[S²] = 2/μ² and P-K reduces to M/M/1.
/// let (lambda, mu) = (0.5, 1.0);
/// let pk = mg1(lambda, 1.0 / mu, 2.0 / (mu * mu)).unwrap();
/// let mm1r = tpt_des_queue::mm1(lambda, mu).unwrap();
/// assert!((pk.wq - mm1r.wq).abs() < 1e-12);
/// ```
pub fn mg1(lambda: f64, mean_service: f64, second_moment: f64) -> Result<Mg1Result, QueueError> {
    if lambda <= 0.0
        || mean_service <= 0.0
        || second_moment <= 0.0
        || !lambda.is_finite()
        || !mean_service.is_finite()
        || !second_moment.is_finite()
    {
        return Err(QueueError::InvalidParameter);
    }
    let rho = lambda * mean_service;
    if rho >= 1.0 {
        return Err(QueueError::Unstable);
    }
    let lq = lambda * lambda * second_moment / (2.0 * (1.0 - rho));
    let wq = lq / lambda;
    let w = wq + mean_service;
    let l = lq + rho;
    Ok(Mg1Result { rho, l, lq, w, wq })
}

/// Erlang B: the blocking probability of an M/G/c/c loss system (c trunks,
/// no waiting room) at offered load `a` (in erlangs).
///
/// Computed by the stable downward recursion `B(0) = 1`,
/// `B(k) = a·B(k−1) / (k + a·B(k−1))`, which is exact and immune to the
/// overflow of the raw factorial formula.
///
/// # Examples
///
/// ```
/// use tpt_des_queue::erlang_b;
///
/// // 1 trunk at offered load 1: blocks exactly half the calls.
/// assert!((erlang_b(1.0, 1).unwrap() - 0.5).abs() < 1e-12);
/// ```
pub fn erlang_b(a: f64, c: u32) -> Result<f64, QueueError> {
    if a < 0.0 || !a.is_finite() || c == 0 {
        return Err(QueueError::InvalidParameter);
    }
    let mut b = 1.0;
    for k in 1..=c {
        b = a * b / (k as f64 + a * b);
    }
    Ok(b)
}

/// Erlang C: the probability that an arrival in an M/M/c queue must wait,
/// at offered load `a` with `c` servers (`a < c`).
pub fn erlang_c(a: f64, c: u32) -> Result<f64, QueueError> {
    if a < 0.0 || !a.is_finite() || c == 0 {
        return Err(QueueError::InvalidParameter);
    }
    // No offered traffic: nobody waits.
    if a == 0.0 {
        return Ok(0.0);
    }
    if a >= c as f64 {
        return Err(QueueError::Unstable);
    }
    Ok(erlang_c_and_p0(a, c).0)
}

/// Erlang C and the empty-system probability `P0` for `0 < a < c`, computed
/// in log space so that large `c` cannot overflow `aⁿ/n!`.
///
/// `ln(aⁿ/n!)` is built incrementally; the normalizer
/// `Σ_{n<c} aⁿ/n! + a^c/(c!(1−ρ))` is summed with the usual log-sum-exp
/// shift, so every term is `exp(x)` with `x ≤ 0`.
fn erlang_c_and_p0(a: f64, c: u32) -> (f64, f64) {
    let rho = a / c as f64;
    let ln_a = a.ln();
    let mut log_terms = Vec::with_capacity(c as usize + 1);
    let mut lt = 0.0;
    log_terms.push(lt);
    for n in 1..=c {
        lt += ln_a - (n as f64).ln();
        log_terms.push(lt);
    }
    let ln_tail = log_terms[c as usize] - (1.0 - rho).ln();
    let head = &log_terms[..c as usize];
    let shift = head.iter().copied().fold(ln_tail, f64::max);
    let sum: f64 = head.iter().map(|&x| (x - shift).exp()).sum::<f64>() + (ln_tail - shift).exp();
    let ln_total = shift + sum.ln();
    let p0 = (-ln_total).exp();
    let erlang_c = (ln_tail - ln_total).exp();
    (erlang_c, p0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn mm1_textbook_values() {
        let r = mm1(2.0, 3.0).unwrap();
        assert!(close(r.rho, 2.0 / 3.0, 1e-12));
        assert!(close(r.l, 2.0, 1e-12));
        assert!(close(r.lq, 4.0 / 3.0, 1e-12));
        assert!(close(r.w, 1.0, 1e-12));
        assert!(close(r.wq, 2.0 / 3.0, 1e-12));
        assert!(close(r.p0, 1.0 / 3.0, 1e-12));
    }

    #[test]
    fn mm1_rejects_unstable_and_invalid() {
        assert_eq!(mm1(3.0, 3.0), Err(QueueError::Unstable));
        assert_eq!(mm1(4.0, 3.0), Err(QueueError::Unstable));
        assert_eq!(mm1(-1.0, 3.0), Err(QueueError::InvalidParameter));
        assert_eq!(mm1(f64::NAN, 3.0), Err(QueueError::InvalidParameter));
    }

    #[test]
    fn mmc_matches_mm1_for_single_server() {
        let (lambda, mu) = (0.7, 1.3);
        let one = mm1(lambda, mu).unwrap();
        let c1 = mmc(lambda, mu, 1).unwrap();
        assert!(close(c1.lq, one.lq, 1e-9));
        assert!(close(c1.erlang_c, one.rho, 1e-9)); // P(wait) = ρ for M/M/1
    }

    #[test]
    fn mmc_probability_mass_sums_to_one() {
        // P0 + P(wait)·1/(1−ρ) identity: L = Lq + a must hold for a sweep.
        for c in [1u32, 2, 3, 7] {
            let r = mmc(0.9 * c as f64, 1.0, c).unwrap();
            assert!(close(r.l, r.lq + r.offered_load, 1e-9));
            assert!(close(r.w, r.wq + 1.0, 1e-9));
            assert!(r.erlang_c <= 1.0 && r.erlang_c >= 0.0);
        }
    }

    #[test]
    fn pk_reduces_to_mm1_for_exponential_service() {
        let (lambda, mu) = (0.6, 1.0);
        let pk = mg1(lambda, 1.0 / mu, 2.0 / (mu * mu)).unwrap();
        let exact = mm1(lambda, mu).unwrap();
        assert!(close(pk.lq, exact.lq, 1e-12));
        assert!(close(pk.w, exact.w, 1e-12));
    }

    #[test]
    fn erlang_b_matches_direct_formula() {
        // Direct: B = (a^c/c!) / Σ_{k=0}^{c} a^k/k!
        let a = 4.0;
        let c = 6u32;
        let mut terms = Vec::new();
        let mut term = 1.0;
        terms.push(term);
        for k in 1..=c {
            term *= a / k as f64;
            terms.push(term);
        }
        let direct = terms[c as usize] / terms.iter().sum::<f64>();
        assert!(close(erlang_b(a, c).unwrap(), direct, 1e-12));
    }

    #[test]
    fn erlang_c_converges_to_small_wait_for_many_servers() {
        let p = erlang_c(10.0, 20).unwrap();
        assert!(p < 0.01, "Erlang C at ρ=0.5, c=20 should be tiny, got {p}");
    }
}
