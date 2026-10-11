//! Time-series accumulators for simulation outputs.
//!
//! * [`RunningStats`] — Welford one-pass mean/variance.
//! * [`TimeWeightedAvg`] — exact time-integral average for step functions
//!   (queue length, utilization), the standard DES observation mode.
//! * [`autocorrelation`] — lag-k autocorrelation, the batch-means sanity
//!   check.

use crate::error::StatsError;
use tpt_des_core::{Seconds, Time};

/// One-pass running statistics (Welford's algorithm).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl RunningStats {
    /// Empty accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one observation.
    pub fn push(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    /// Number of observations.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Current mean (0 before any observation).
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Sample variance (0 for fewer than two observations).
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    /// Sample standard deviation.
    pub fn stddev(&self) -> f64 {
        self.variance().sqrt()
    }
}

/// Exact time-weighted average of a step function: push `(t, value)` at
/// each change; the average integrates the value over `[first_t, last_t]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TimeWeightedAvg {
    first_t: Time<Seconds>,
    last_t: Time<Seconds>,
    last_v: f64,
    area: f64,
    started: bool,
}

impl TimeWeightedAvg {
    /// Empty accumulator; the window opens at the first push.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the value `v` holding from time `t` onward.
    ///
    /// # Errors
    ///
    /// [`StatsError::NonFinite`] for a NaN or infinite value (a non-finite
    /// time cannot be built as a `Time`, so it panics at construction), and
    /// [`StatsError::OutOfOrder`] if `t` precedes the last recorded time. A
    /// rejected push leaves the accumulator unchanged.
    pub fn push(&mut self, t: Time<Seconds>, v: f64) -> Result<(), StatsError> {
        if !v.is_finite() {
            return Err(StatsError::NonFinite);
        }
        if self.started && t < self.last_t {
            return Err(StatsError::OutOfOrder);
        }
        if !self.started {
            self.first_t = t;
        } else {
            self.area += self.last_v * (t - self.last_t).raw();
        }
        self.last_t = t;
        self.last_v = v;
        self.started = true;
        Ok(())
    }

    /// The time-weighted average over the observed window (0 for an empty
    /// or single-point window).
    pub fn average(&self) -> f64 {
        if !self.started {
            return 0.0;
        }
        let span = self.window().raw();
        if span <= 0.0 {
            0.0
        } else {
            self.area / span
        }
    }

    /// Length of the observed window, from the first push to the last.
    pub fn window(&self) -> Time<Seconds> {
        if self.started {
            self.last_t - self.first_t
        } else {
            Time::ZERO
        }
    }

    /// Integral of the value over time (utilization bounds, area metrics).
    pub fn integral(&self) -> f64 {
        self.area
    }
}

/// Lag-`lag` autocorrelation of `series` (normalized by overall variance).
/// Returns 0 for constant or too-short series.
#[must_use]
pub fn autocorrelation(series: &[f64], lag: usize) -> f64 {
    let n = series.len();
    if n < lag + 2 {
        return 0.0;
    }
    let mean = series.iter().sum::<f64>() / n as f64;
    let centered: Vec<f64> = series.iter().map(|v| v - mean).collect();
    let denom: f64 = centered.iter().map(|v| v * v).sum();
    if denom <= 0.0 {
        return 0.0;
    }
    let numer: f64 = (0..n - lag).map(|i| centered[i] * centered[i + lag]).sum();
    numer / denom
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn welford_matches_direct_computation() {
        let mut rs = RunningStats::new();
        let data = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        for &v in &data {
            rs.push(v);
        }
        assert_eq!(rs.count(), 8);
        assert!(close(rs.mean(), 5.0, 1e-12));
        // Sample variance of this classic dataset is 4.571428…
        assert!(close(rs.variance(), 32.0 / 7.0, 1e-12));
        assert!(close(rs.stddev(), (32.0f64 / 7.0).sqrt(), 1e-12));
    }

    #[test]
    fn time_weighted_average_is_exact() {
        let mut twa = TimeWeightedAvg::new();
        // Queue length: 0 from t=0, 2 from t=1, 5 from t=3, 0 from t=8.
        twa.push(Time::from_raw(0.0), 0.0).unwrap();
        twa.push(Time::from_raw(1.0), 2.0).unwrap();
        twa.push(Time::from_raw(3.0), 5.0).unwrap();
        twa.push(Time::from_raw(8.0), 0.0).unwrap();
        // Integral: 2·2 + 5·5 = 29 over [0, 8].
        assert!(close(twa.integral(), 29.0, 1e-12));
        assert!(close(twa.average(), 29.0 / 8.0, 1e-12));
        assert!(close(twa.window().raw(), 8.0, 1e-12));
    }

    #[test]
    fn autocorrelation_of_alternating_series_is_negative() {
        let series: Vec<f64> = (0..1000)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let acf1 = autocorrelation(&series, 1);
        assert!(acf1 < -0.9, "alternating series lag-1 acf = {acf1}");
        let acf2 = autocorrelation(&series, 2);
        assert!(acf2 > 0.9, "alternating series lag-2 acf = {acf2}");
        // Constant series → 0 (undefined, reported as 0).
        assert_eq!(autocorrelation(&vec![3.0; 100], 1), 0.0);
    }
}
