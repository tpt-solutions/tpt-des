//! Regression tests for the Phase 6 statistics fixes.

use tpt_des_core::Time;
use tpt_des_stats::batch::{batch_means, regenerative};
use tpt_des_stats::ci::{mean_of_samples_ci, proportion_wilson, student_t_critical};
use tpt_des_stats::dist::{ln_gamma, normal_cdf, normal_ppf};
use tpt_des_stats::timeseries::TimeWeightedAvg;
use tpt_des_stats::StatsError;

/// The window used to be measured from t = 0 rather than from the first
/// push. Here the signal is observed from t = 2 to t = 6, so the average is
/// (1·2 + 3·2) / 4 = 2, not 8 / 6.
#[test]
fn time_weighted_window_starts_at_first_push() {
    let mut twa = TimeWeightedAvg::new();
    twa.push(Time::from_raw(2.0), 1.0).unwrap();
    twa.push(Time::from_raw(4.0), 3.0).unwrap();
    twa.push(Time::from_raw(6.0), 0.0).unwrap();
    assert!((twa.window().raw() - 4.0).abs() < 1e-12);
    assert!((twa.average() - 2.0).abs() < 1e-12);
}

/// An out-of-order push used to rewrite the clock backwards and corrupt the
/// area. It must now be rejected with no effect on the accumulator.
#[test]
fn out_of_order_push_is_rejected_without_side_effects() {
    let mut twa = TimeWeightedAvg::new();
    twa.push(Time::from_raw(5.0), 1.0).unwrap();
    twa.push(Time::from_raw(7.0), 2.0).unwrap();
    assert_eq!(
        twa.push(Time::from_raw(6.0), 0.0),
        Err(StatsError::OutOfOrder)
    );
    twa.push(Time::from_raw(9.0), 0.0).unwrap();
    // Area: 1·2 (on [5,7)) + 2·2 (on [7,9)) = 6 over a window of 4.
    assert!((twa.integral() - 6.0).abs() < 1e-12);
    assert!((twa.average() - 1.5).abs() < 1e-12);
}

#[test]
fn non_finite_push_is_rejected() {
    let mut twa = TimeWeightedAvg::new();
    assert_eq!(
        twa.push(Time::from_raw(0.0), f64::INFINITY),
        Err(StatsError::NonFinite)
    );
    assert_eq!(twa.average(), 0.0);
}

/// Φ(−5) = 2.866515718791939e-7. The old A&S 7.1.26 erf was only good to
/// about 1.5e-7 absolute, so the tail was off by ~50%.
#[test]
fn normal_cdf_tail_is_relatively_accurate() {
    let exact = 2.866_515_718_791_939e-7;
    let got = normal_cdf(-5.0);
    assert!(
        ((got - exact) / exact).abs() < 1e-9,
        "Φ(−5) = {got:e}, expected {exact:e}"
    );
    assert!((normal_cdf(0.0) - 0.5).abs() < 1e-15);
    assert!((normal_cdf(1.96) - 0.975_002_104_851_780).abs() < 1e-9);
}

#[test]
fn normal_ppf_inverts_the_accurate_cdf() {
    for &p in &[1e-6, 0.025, 0.5, 0.975, 1.0 - 1e-6] {
        let x = normal_ppf(p);
        assert!((normal_cdf(x) - p).abs() < 1e-9, "p = {p}");
    }
}

/// These used to panic. They are domain errors now, reported as NaN from
/// the math helpers.
#[test]
fn math_helpers_return_nan_outside_their_domain() {
    assert!(ln_gamma(0.0).is_nan());
    assert!(ln_gamma(-1.5).is_nan());
    assert!(normal_ppf(1.5).is_nan());
    assert!(normal_ppf(f64::NAN).is_nan());
    assert!(student_t_critical(1.5, 10).is_nan());
    assert!(student_t_critical(0.95, 0).is_nan());
}

#[test]
fn proportion_inputs_are_validated() {
    assert_eq!(proportion_wilson(1, 0, 0.95), Err(StatsError::EmptySample));
    assert_eq!(proportion_wilson(5, 3, 0.95), Err(StatsError::InvalidCount));
    assert_eq!(
        proportion_wilson(1, 3, 1.0),
        Err(StatsError::InvalidConfidence)
    );
}

/// A single observation used to yield the degenerate interval `(m, m)`,
/// which looks like a zero-width (certain) estimate. It is an error now.
#[test]
fn single_observation_has_no_interval() {
    assert_eq!(
        mean_of_samples_ci(&[3.0], 0.95),
        Err(StatsError::InsufficientData)
    );
    assert_eq!(mean_of_samples_ci(&[], 0.95), Err(StatsError::EmptySample));
    assert_eq!(
        mean_of_samples_ci(&[1.0, 2.0], 0.0),
        Err(StatsError::InvalidConfidence)
    );
}

/// A series shorter than the batch count used to return `grand_mean = 0`
/// with no batches, which is a silent wrong answer.
#[test]
fn batch_means_rejects_series_shorter_than_batch_count() {
    let short = [1.0, 2.0, 3.0];
    assert_eq!(
        batch_means(&short, 10, 0.95).err(),
        Some(StatsError::InsufficientData)
    );
}

#[test]
fn regenerative_rejects_empty_and_non_positive_cycles() {
    assert_eq!(
        regenerative(&[], &[], 0.95).err(),
        Some(StatsError::EmptySample)
    );
    assert_eq!(
        regenerative(&[1.0, 2.0], &[0.0, 0.0], 0.95).err(),
        Some(StatsError::InvalidParameter)
    );
}

/// Reference: direct window means (what `moving_average` did before the
/// prefix-sum rewrite).
fn moving_average_direct(series: &[f64], window: usize) -> Vec<f64> {
    let half = window / 2;
    (0..series.len())
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(series.len());
            series[lo..hi].iter().sum::<f64>() / (hi - lo) as f64
        })
        .collect()
}

#[test]
fn moving_average_prefix_sums_match_direct_windows() {
    let series: Vec<f64> = (0..50).map(|i| ((i * 37) % 11) as f64 + 0.5).collect();
    for window in [1usize, 2, 4, 5, 9, 60] {
        let fast = tpt_des_stats::warmup::moving_average(&series, window);
        let direct = moving_average_direct(&series, window);
        assert_eq!(fast.len(), direct.len());
        for (a, b) in fast.iter().zip(&direct) {
            assert!((a - b).abs() < 1e-12, "window {window}: {a} vs {b}");
        }
    }
}

/// Reference: MSER with a fresh variance per suffix (the pre-optimization
/// algorithm). The Welford backward pass must pick the same truncation point.
fn mser_direct(series: &[f64], stride: usize) -> usize {
    if series.len() < 4 * stride.max(1) {
        return 0;
    }
    let stride = stride.max(1);
    let batched: Vec<f64> = series
        .chunks(stride)
        .filter(|c| c.len() == stride)
        .map(|c| c.iter().sum::<f64>() / stride as f64)
        .collect();
    let sem = |data: &[f64]| -> f64 {
        if data.len() < 2 {
            return f64::INFINITY;
        }
        let n = data.len() as f64;
        let mean = data.iter().sum::<f64>() / n;
        let var = data.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0);
        (var / n).sqrt()
    };
    let mut best_d = 0usize;
    let mut best = f64::INFINITY;
    for d in 0..batched.len() / 2 {
        let s = sem(&batched[d..]);
        if s < best {
            best = s;
            best_d = d;
        }
    }
    best_d * stride
}

#[test]
fn mser_backward_welford_matches_direct_reference() {
    // Deterministic series: a decaying transient followed by noise.
    let mut state = 12345u64;
    let series: Vec<f64> = (0..600)
        .map(|i| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let noise = (state >> 11) as f64 / (1u64 << 53) as f64;
            5.0 * (-(i as f64) / 40.0).exp() + noise
        })
        .collect();
    for stride in [1usize, 5, 10] {
        assert_eq!(
            tpt_des_stats::warmup::mser_stride(&series, stride),
            mser_direct(&series, stride),
            "stride {stride}"
        );
    }
}
