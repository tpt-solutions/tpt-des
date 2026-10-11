//! Output analysis: batch means, replication/deletion, regenerative methods.

use crate::ci;
use crate::error::{check_confidence, StatsError};

/// Batch-means analysis of one long run.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchMeans {
    /// Mean of each batch.
    pub batch_means: Vec<f64>,
    /// Grand mean over all batches.
    pub grand_mean: f64,
    /// Sample variance across batch means.
    pub batch_variance: f64,
    /// Confidence interval for the mean from the batch statistics.
    pub ci: (f64, f64),
}

/// Non-overlapping batch means: split `series` into `n_batches` equal
/// chunks, average each, and form a Student-t interval across batches.
///
/// The batch count trades bias (batches too short retain correlation)
/// against variance (too few batches); a classic default is 20–30 batches.
///
/// # Errors
///
/// [`StatsError::InsufficientData`] if `series` has fewer observations than
/// batches (some batch would be empty), plus the interval's own errors.
pub fn batch_means(
    series: &[f64],
    n_batches: usize,
    confidence: f64,
) -> Result<BatchMeans, StatsError> {
    check_confidence(confidence)?;
    let n_batches = n_batches.max(2);
    let len = series.len();
    let batch_len = len / n_batches;
    if batch_len == 0 {
        return Err(StatsError::InsufficientData);
    }
    let mut batch_means = Vec::with_capacity(n_batches);
    for b in 0..n_batches {
        let chunk = &series[b * batch_len..(b + 1) * batch_len];
        batch_means.push(chunk.iter().sum::<f64>() / batch_len as f64);
    }
    let grand_mean = batch_means.iter().sum::<f64>() / batch_means.len().max(1) as f64;
    let k = batch_means.len();
    let batch_variance = if k > 1 {
        batch_means
            .iter()
            .map(|m| (m - grand_mean) * (m - grand_mean))
            .sum::<f64>()
            / (k - 1) as f64
    } else {
        0.0
    };
    let ci = ci::mean_of_samples_ci(&batch_means, confidence)?;
    Ok(BatchMeans {
        batch_means,
        grand_mean,
        batch_variance,
        ci,
    })
}

/// Replication/deletion result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReplicationDeletion {
    /// Mean across replication means.
    pub mean: f64,
    /// Student-t confidence interval across replication means.
    pub ci: (f64, f64),
    /// Number of replications used.
    pub replications: usize,
}

/// Replication/deletion: delete the first `delete` observations of every
/// replication (warm-up), average the rest per replication, then form a
/// confidence interval across the independent replication means — the
/// statistically cleanest interval because replications are independent.
///
/// # Errors
///
/// Needs at least two replications that survive deletion; see
/// [`ci::mean_of_samples_ci`].
pub fn replication_deletion(
    replications: &[Vec<f64>],
    delete: usize,
    confidence: f64,
) -> Result<ReplicationDeletion, StatsError> {
    let means: Vec<f64> = replications
        .iter()
        .filter_map(|r| {
            if r.len() > delete {
                Some(r[delete..].iter().sum::<f64>() / (r.len() - delete) as f64)
            } else {
                None
            }
        })
        .collect();
    let ci = ci::mean_of_samples_ci(&means, confidence)?;
    Ok(ReplicationDeletion {
        mean: means.iter().sum::<f64>() / means.len() as f64,
        ci,
        replications: means.len(),
    })
}

/// Regenerative (cycle) ratio estimator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regenerative {
    /// Ratio estimator of the cycle mean: ΣZ/Στ.
    pub mean: f64,
    /// Delta-method confidence interval.
    pub ci: (f64, f64),
    /// Number of complete cycles.
    pub cycles: usize,
}

/// Regenerative estimation over completed cycles: `Z_i` is the observed
/// quantity summed over cycle `i`, `τ_i` its duration. The ratio estimator
/// `ΣZ/Στ` is strongly consistent; the interval uses the delta method with
/// Student-t across cycles.
///
/// # Errors
///
/// [`StatsError::EmptySample`] with no complete cycle,
/// [`StatsError::InvalidParameter`] if the total cycle length is not positive,
/// [`StatsError::InvalidConfidence`] outside `(0, 1)`.
pub fn regenerative(
    cycle_sums: &[f64],
    cycle_lengths: &[f64],
    confidence: f64,
) -> Result<Regenerative, StatsError> {
    check_confidence(confidence)?;
    let n = cycle_sums.len().min(cycle_lengths.len());
    if n == 0 {
        return Err(StatsError::EmptySample);
    }
    let sum_z: f64 = cycle_sums[..n].iter().sum();
    let sum_t: f64 = cycle_lengths[..n].iter().sum();
    if !sum_t.is_finite() || sum_t <= 0.0 {
        return Err(StatsError::InvalidParameter);
    }
    let mean = sum_z / sum_t;

    // Per-cycle i.i.d. contributions for the delta-method interval:
    // Y_i = Z_i − mean·τ_i, estimate sd of Y, divide by Στ.
    let y: Vec<f64> = (0..n)
        .map(|i| cycle_sums[i] - mean * cycle_lengths[i])
        .collect();
    let k = n as f64;
    let half_width = if n > 1 {
        let y_mean = y.iter().sum::<f64>() / k;
        let y_var = y.iter().map(|v| (v - y_mean) * (v - y_mean)).sum::<f64>() / (k - 1.0);
        let t = ci::student_t_critical(confidence, n - 1);
        t * (y_var / k).sqrt() / sum_t
    } else {
        f64::INFINITY
    };
    Ok(Regenerative {
        mean,
        ci: (mean - half_width, mean + half_width),
        cycles: n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn batch_means_of_constant_series() {
        let series = vec![2.5; 1000];
        let bm = batch_means(&series, 20, 0.95).unwrap();
        assert!(close(bm.grand_mean, 2.5, 1e-12));
        assert!(close(bm.batch_variance, 0.0, 1e-24));
        // Degenerate interval collapses to the constant.
        assert!(close(bm.ci.0, 2.5, 1e-9));
        assert!(close(bm.ci.1, 2.5, 1e-9));
        assert_eq!(bm.batch_means.len(), 20);
    }

    #[test]
    fn batch_means_grand_mean_matches_manual() {
        let series: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let bm = batch_means(&series, 10, 0.95).unwrap();
        assert!(close(bm.grand_mean, 499.5, 1e-9));
        // Batches are equal-width means of a ramp: 49.5, 149.5, …
        assert!(close(bm.batch_means[0], 49.5, 1e-9));
        assert!(close(bm.batch_means[9], 949.5, 1e-9));
    }

    #[test]
    fn replication_deletion_across_independent_reps() {
        let reps = vec![
            vec![100.0, 0.1, 0.2, 0.3, 0.4],
            vec![100.0, 0.5, 0.6, 0.7, 0.8],
        ];
        let rd = replication_deletion(&reps, 1, 0.95).unwrap();
        assert_eq!(rd.replications, 2);
        assert!(close(rd.mean, 0.45, 1e-12));
        assert!(rd.ci.0 <= rd.mean && rd.mean <= rd.ci.1);
    }

    #[test]
    fn regenerative_ratio_estimator() {
        // Cycles: (Z, τ): mean = ΣZ/Στ = 20/10 = 2.
        let rd = regenerative(&[1.0, 4.0, 6.0, 9.0], &[1.0, 2.0, 3.0, 4.0], 0.95).unwrap();
        assert!(close(rd.mean, 2.0, 1e-12));
        assert!(rd.ci.0 < rd.mean && rd.mean < rd.ci.1);
    }
}
