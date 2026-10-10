//! Warm-up (initial-transient) detection.
//!
//! Two complementary heuristics over one or more replications of an output
//! series (e.g. mean queue length sampled at departures):
//!
//! * [`welch`] — average the replications pointwise, smooth with a
//!   moving average, and take the first point where the smoothed series
//!   settles within a relative band of the grand mean ([`welch_warmup`]).
//! * [`mser`] — the marginal standard error rule: truncate the series at
//!   every candidate point, compute the standard error of the remaining
//!   mean, and pick the truncation minimizing it. [`mser_stride`] applies
//!   the classical batching (MSER-5) variant; [`msir`] is an alias kept for
//!   the spec's naming.

use alloc::vec;
use alloc::vec::Vec;

/// Pointwise average of several equal-length replications.
///
/// Replications of differing lengths are truncated to the shortest.
#[must_use]
pub fn welch(replications: &[Vec<f64>]) -> Vec<f64> {
    if replications.is_empty() {
        return Vec::new();
    }
    let len = replications.iter().map(Vec::len).min().unwrap_or(0);
    let mut out = vec![0.0; len];
    for series in replications {
        for (o, &v) in out.iter_mut().zip(series) {
            *o += v;
        }
    }
    let k = replications.len() as f64;
    out.iter_mut().for_each(|v| *v /= k);
    out
}

/// Centered moving average with `window` width (odd windows are natural;
/// the window is truncated at the edges).
#[must_use]
pub fn moving_average(series: &[f64], window: usize) -> Vec<f64> {
    if series.is_empty() || window == 0 {
        return Vec::new();
    }
    let half = (window / 2) as isize;
    series
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let i = i as isize;
            let lo = (i - half).max(0) as usize;
            let hi = ((i + half + 1) as usize).min(series.len());
            let slice = &series[lo..hi];
            slice.iter().sum::<f64>() / slice.len() as f64
        })
        .collect()
}

/// First index where the smoothed series stays within `frac` (relative) of
/// the grand mean for `sustained` consecutive points.
#[must_use]
pub fn welch_warmup(smoothed: &[f64], frac: f64, sustained: usize) -> usize {
    if smoothed.is_empty() {
        return 0;
    }
    let mean = smoothed.iter().sum::<f64>() / smoothed.len() as f64;
    let band = mean.abs() * frac;
    for start in 0..smoothed.len() {
        let ok = smoothed[start..]
            .iter()
            .take(sustained.min(smoothed.len() - start))
            .all(|&v| (v - mean).abs() <= band.max(1e-12));
        if ok && smoothed.len() - start >= sustained.min(smoothed.len()) {
            return start;
        }
    }
    0
}

/// Variance of the sample mean of `data` (σ²/n with sample variance).
fn variance_of_mean(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return f64::INFINITY;
    }
    let n = data.len() as f64;
    let mean = data.iter().sum::<f64>() / n;
    let var = data.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0);
    var / n
}

/// MSER: truncation index minimizing the standard error of the truncated
/// mean. Returns the deletion count `d` (data beyond `d` is kept).
///
/// Correlated observations (the norm in DES) underestimate this variance —
/// which is why the batched variant [`mser_stride`] is preferred in
/// practice.
#[must_use]
pub fn mser(series: &[f64]) -> usize {
    mser_stride(series, 1)
}

/// MSER with batching: observations are pre-averaged in strides of `stride`
/// (MSER-5 uses 5) before the rule is applied, which decorrelates them.
///
/// The name `msir` in spec/todo is a common transcription of this family of
/// rules; this function is exposed under both spellings.
#[must_use]
pub fn mser_stride(series: &[f64], stride: usize) -> usize {
    if series.len() < 4 * stride.max(1) {
        return 0;
    }
    let stride = stride.max(1);
    // Pre-average in strides.
    let batched: Vec<f64> = series
        .chunks(stride)
        .filter(|c| c.len() == stride)
        .map(|c| c.iter().sum::<f64>() / stride as f64)
        .collect();

    let mut best_d = 0usize;
    let mut best_sem = f64::INFINITY;
    for d in 0..batched.len() / 2 {
        let sem = variance_of_mean(&batched[d..]).sqrt();
        if sem < best_sem {
            best_sem = sem;
            best_d = d;
        }
    }
    best_d * stride
}

/// Alias for [`mser_stride`] under the spec's "MSIR" naming.
#[must_use]
pub fn msir(series: &[f64], stride: usize) -> usize {
    mser_stride(series, stride)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn welch_averages_replications() {
        let reps = vec![vec![1.0, 2.0, 3.0], vec![3.0, 4.0, 5.0], vec![10.0, 10.0]];
        let averaged = welch(&reps);
        // Third replication truncates to length 2.
        assert_eq!(averaged.len(), 2);
        assert!(close(averaged[0], 14.0 / 3.0, 1e-12));
        assert!(close(averaged[1], 16.0 / 3.0, 1e-12));
    }

    #[test]
    fn warmup_detected_on_transient_series() {
        // Ramp (transient) for 100 points, then stationary noise around 5.
        let mut series = Vec::new();
        for i in 0..100 {
            series.push(50.0 - i as f64 * 0.5);
        }
        for i in 0..400 {
            series.push(5.0 + if i % 2 == 0 { 0.1 } else { -0.1 });
        }
        let smoothed = moving_average(&series, 21);
        let warmup = welch_warmup(&smoothed, 0.05, 50);
        // Warm-up must fall in (or near) the transient part, not later.
        assert!(
            warmup <= 150,
            "warm-up detected at {warmup}, expected within the ramp region"
        );
    }

    #[test]
    fn mser_finds_truncation_of_transient() {
        // 100 points of extreme transient (zeros), then stable 5s.
        let mut series = vec![0.0; 100];
        series.extend(vec![5.0; 500]);
        let d = mser(&series);
        // MSER should delete roughly the transient part.
        assert!((50..=200).contains(&d), "MSER deleted {d}, expected ≈ 100");
    }

    #[test]
    fn msir_alias_matches_mser_stride() {
        let series: Vec<f64> = (0..300).map(|i| i as f64).collect();
        assert_eq!(msir(&series, 5), mser_stride(&series, 5));
    }

    #[test]
    fn stationary_series_has_small_warmup() {
        let series = vec![3.0; 500];
        assert!(mser(&series) <= 10);
    }
}
