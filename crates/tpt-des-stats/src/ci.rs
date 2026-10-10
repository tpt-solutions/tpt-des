//! Confidence intervals: means (Student-t), proportions (Wilson score),
//! quantiles (order statistics).

use crate::dist;

/// Critical Student-t value `t_{1−α/2, df}` for a two-sided interval at
/// `confidence` (e.g. 0.95 → t_{0.975, df}).
#[must_use]
pub fn student_t_critical(confidence: f64, df: usize) -> f64 {
    dist::student_t_ppf(0.5 * (1.0 + confidence), df as f64)
}

/// Student-t confidence interval for the mean of (roughly) i.i.d. samples.
#[must_use]
pub fn mean_of_samples_ci(samples: &[f64], confidence: f64) -> (f64, f64) {
    let n = samples.len();
    if n == 0 {
        return (f64::NAN, f64::NAN);
    }
    let mean = samples.iter().sum::<f64>() / n as f64;
    if n == 1 {
        return (mean, mean);
    }
    let var = samples.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1) as f64;
    let t = student_t_critical(confidence, n - 1);
    let half = t * (var / n as f64).sqrt();
    (mean - half, mean + half)
}

/// Normal-approximation confidence interval for a mean with known σ.
#[must_use]
pub fn mean_known_sigma_ci(samples: &[f64], sigma: f64, confidence: f64) -> (f64, f64) {
    let n = samples.len();
    if n == 0 {
        return (f64::NAN, f64::NAN);
    }
    let mean = samples.iter().sum::<f64>() / n as f64;
    let z = dist::normal_ppf(0.5 * (1.0 + confidence));
    let half = z * sigma / (n as f64).sqrt();
    (mean - half, mean + half)
}

/// Wilson score interval for a binomial proportion — strictly inside
/// [0, 1], unlike the naive Wald interval.
#[must_use]
pub fn proportion_wilson(successes: usize, trials: usize, confidence: f64) -> (f64, f64) {
    assert!(trials > 0, "trials must be positive");
    let n = trials as f64;
    let p_hat = successes as f64 / n;
    let z = dist::normal_ppf(0.5 * (1.0 + confidence));
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let center = (p_hat + z2 / (2.0 * n)) / denom;
    let half = (z / denom) * ((p_hat * (1.0 - p_hat) / n + z2 / (4.0 * n * n)).sqrt());
    let lo = (center - half).clamp(0.0, 1.0);
    let hi = (center + half).clamp(0.0, 1.0);
    (lo, hi)
}

/// Order-statistics confidence interval for the `p`-quantile from a sorted
/// sample. Returns `(lower_index, upper_index)` — the interval covers `p`
/// with at least `confidence` probability under i.i.d. sampling.
///
/// Indices index into the **sorted** sample; upper is exclusive.
#[must_use]
pub fn quantile_ci_sorted(sorted: &[f64], p: f64, confidence: f64) -> (usize, usize) {
    assert!(!sorted.is_empty(), "sample must be non-empty");
    assert!((0.0..=1.0).contains(&p), "quantile p in [0, 1]");
    let n = sorted.len() as f64;
    // P(X_(k) ≤ q_p) follows Binomial(k; n, p): find k− and k+ bracketing
    // the target coverage exactly by summing the binomial CDF.
    let target = 0.5 * (1.0 + confidence);
    // j = number of order statistics strictly below the quantile: find
    // smallest j with P(X_(j) > q_p) ≤ 1 − target, i.e. tail mass.
    let binom_tail = |j: usize| -> f64 {
        // P(fewer than j successes... ) computed directly:
        // tail = P(Bin(n, p) ≤ j) via logs.
        let mut acc = 0.0f64;
        let ln_p = p.ln();
        let ln_1mp = (1.0 - p).ln();
        let mut ln_choose = 0.0f64;
        for i in 0..=j.min(sorted.len()) {
            if i > 0 {
                ln_choose += ((n - (i - 1) as f64) / i as f64).ln();
            }
            acc += (ln_choose + i as f64 * ln_p + (n - i as f64) * ln_1mp).exp();
            if acc >= 1.0 {
                return 1.0;
            }
        }
        acc.min(1.0)
    };
    let mut lo = 0usize;
    while lo < sorted.len() && binom_tail(lo) < 1.0 - target {
        lo += 1;
    }
    let mut hi = sorted.len();
    while hi > lo && binom_tail(hi) > target {
        hi -= 1;
    }
    (lo.min(sorted.len()), hi.min(sorted.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn t_critical_matches_tables() {
        assert!(close(student_t_critical(0.95, 10), 2.228_139, 1e-4));
        assert!(close(student_t_critical(0.99, 5), 4.032_143, 1e-4));
    }

    #[test]
    fn mean_ci_brackets_sample_mean_and_shrinks_with_n() {
        let samples = [1.0, 2.0, 3.0, 4.0, 5.0];
        let (lo, hi) = mean_of_samples_ci(&samples, 0.95);
        assert!(lo < 3.0 && 3.0 < hi);
        let more = [1.0, 2.0, 3.0, 4.0, 5.0, 2.0, 3.0, 4.0, 3.0, 3.0];
        let (lo2, hi2) = mean_of_samples_ci(&more, 0.95);
        assert!(hi2 - lo2 < hi - lo, "interval should shrink with n");
        // Single sample degenerates.
        assert_eq!(mean_of_samples_ci(&[7.0], 0.95), (7.0, 7.0));
    }

    #[test]
    fn wilson_interval_is_well_behaved() {
        let (lo, hi) = proportion_wilson(0, 10, 0.95);
        assert!(lo.abs() < 1e-12, "lower bound at the 0 boundary: {lo}");
        assert!(hi > 0.0 && hi < 0.35);
        let (lo, hi) = proportion_wilson(5, 10, 0.95);
        assert!(lo < 0.5 && 0.5 < hi);
        // Wald would go negative here; Wilson must not.
        let (lo, _) = proportion_wilson(1, 100, 0.95);
        assert!(lo > 0.0);
    }

    #[test]
    fn quantile_ci_covers_median_of_uniform_sample() {
        // Sorted sample of 0..99: the true median (49.5) is covered by the
        // order-statistics interval by construction coverage ≈ confidence.
        let sorted: Vec<f64> = (0..100).map(f64::from).collect();
        let (lo_idx, hi_idx) = quantile_ci_sorted(&sorted, 0.5, 0.95);
        assert!(lo_idx < hi_idx);
        // For n = 100, median CI is roughly indices [40, 60].
        assert!(lo_idx <= 45 && hi_idx >= 55, "got [{lo_idx}, {hi_idx})");
    }
}
