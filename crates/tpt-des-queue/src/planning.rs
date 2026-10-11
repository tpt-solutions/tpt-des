//! Capacity planning and simulation checks for multi-server stations.
//!
//! * [`size_for_mean_wait`] finds the fewest servers whose analytic M/M/c mean
//!   wait meets a target.
//! * [`validate_mmc`] simulates an M/M/c station and compares each metric with
//!   the analytic value. Its [`ValidationReport`] can be asserted in a
//!   `cargo test`.
//! * [`mean_wait_to_precision`] adds replications until the confidence
//!   half-width of the mean wait reaches a target.
//!
//! The simulation uses independent replications: each one is a fresh run with
//! its own seed, so the standard error comes from the spread of the
//! replication means and needs no batching.

use crate::analytic::{mmc, MmcResult};
use crate::error::QueueError;
use crate::sim::{run_queue_multi, Discipline, Exponential, QueueConfig, QueueMetrics};
use core::fmt;
use tpt_des_core::{Seconds, Time};
use tpt_des_stats::ci::student_t_critical;

/// Most servers [`size_for_mean_wait`] will try before giving up.
pub const MAX_SERVERS: u32 = 100_000;

/// Fewest replications [`mean_wait_to_precision`] runs before it checks the
/// half-width. With fewer, the standard error is too noisy to trust.
pub const MIN_REPLICATIONS: u32 = 3;

/// The result of [`size_for_mean_wait`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Sizing {
    /// The smallest server count that meets the target.
    pub servers: u32,
    /// The analytic M/M/c metrics at that server count.
    pub analytic: MmcResult,
}

/// The fewest servers for which the analytic mean queue wait is at most
/// `max_wait`, for arrivals at rate `lambda` and service rate `mu` per server.
///
/// Starts at the smallest stable server count (`⌊λ/μ⌋ + 1`) and adds servers
/// until the target is met. The wait falls with each server, so the first
/// count that meets it is the smallest.
///
/// # Errors
///
/// [`QueueError::InvalidParameter`] if `max_wait` is not finite and positive,
/// the rates are invalid, or no count up to [`MAX_SERVERS`] meets the target.
pub fn size_for_mean_wait(
    lambda: f64,
    mu: f64,
    max_wait: Time<Seconds>,
) -> Result<Sizing, QueueError> {
    if !max_wait.raw().is_finite() || max_wait.raw() <= 0.0 {
        return Err(QueueError::InvalidParameter);
    }
    if !(lambda > 0.0 && mu > 0.0) {
        return Err(QueueError::InvalidParameter);
    }
    let first = (lambda / mu).floor() as u32 + 1;
    for servers in first..=MAX_SERVERS {
        let analytic = mmc(lambda, mu, servers)?;
        if analytic.wq <= max_wait.raw() {
            return Ok(Sizing { servers, analytic });
        }
    }
    Err(QueueError::InvalidParameter)
}

/// Mean and standard error over independent replications.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct Estimate {
    /// Mean of the replication means.
    pub mean: f64,
    /// Standard error of that mean: sample standard deviation / √replications.
    /// NaN with fewer than two replications.
    pub std_error: f64,
    /// Number of replications averaged.
    pub replications: u32,
}

impl Estimate {
    fn from_samples(samples: &[f64]) -> Self {
        let n = samples.len();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let std_error = if n < 2 {
            f64::NAN
        } else {
            let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
            (var / n as f64).sqrt()
        };
        Self {
            mean,
            std_error,
            replications: n as u32,
        }
    }
}

/// One metric in a [`ValidationReport`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct ValidationRow {
    /// Metric name: `Wq`, `W`, `L`, or `rho`.
    pub metric: &'static str,
    /// Analytic value from the M/M/c formulas.
    pub analytic: f64,
    /// Simulated estimate over the replications.
    pub simulated: Estimate,
    /// Whether the simulated estimate is consistent with the analytic value.
    pub passes: bool,
}

/// Simulation versus analytic M/M/c metrics for one station.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct ValidationReport {
    /// Number of servers simulated.
    pub servers: u32,
    /// One row per metric: `Wq`, `W`, `L`, and per-server utilization `rho`.
    pub rows: Vec<ValidationRow>,
}

impl ValidationReport {
    /// True if every metric is consistent with the analytic value.
    pub fn passes(&self) -> bool {
        self.rows.iter().all(|r| r.passes)
    }

    /// Panic with the full table if any metric disagrees with the analytic
    /// value. Intended for `cargo test`.
    ///
    /// # Panics
    ///
    /// If [`passes`](Self::passes) is false.
    pub fn assert_passes(&self) {
        assert!(self.passes(), "simulation disagrees with M/M/c:\n{self}");
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "M/M/c validation, c = {}", self.servers)?;
        writeln!(
            f,
            "{:<6} {:>12} {:>12} {:>12} {:>6}",
            "metric", "analytic", "simulated", "std error", "ok"
        )?;
        for r in &self.rows {
            writeln!(
                f,
                "{:<6} {:>12.6} {:>12.6} {:>12.6} {:>6}",
                r.metric,
                r.analytic,
                r.simulated.mean,
                r.simulated.std_error,
                if r.passes { "yes" } else { "NO" }
            )?;
        }
        Ok(())
    }
}

/// Half-width multiplier for the pass band in [`validate_mmc`]: a metric
/// passes when it lies within this many standard errors, plus the relative
/// slack, of the analytic value.
const BAND_SIGMAS: f64 = 4.0;

/// Relative slack added to the statistical band in [`validate_mmc`]. It
/// covers the bias left by a finite warm-up, which the band does not model.
const RELATIVE_SLACK: f64 = 0.02;

/// Simulate an M/M/c station with `replications` independent runs and
/// compare `Wq`, `W`, `L`, and per-server `ρ` with the analytic values.
///
/// A metric passes when `|simulated − analytic| ≤ 4·SE + 0.02·|analytic|`.
/// A correct model passes with high probability for any seed, and a wrong
/// formula fails.
///
/// # Errors
///
/// [`QueueError::InvalidParameter`] for bad rates, a horizon or warm-up
/// outside `0 ≤ warmup < horizon`, or fewer than two replications.
/// [`QueueError::Unstable`] if `λ ≥ c·μ`.
pub fn validate_mmc(
    lambda: f64,
    mu: f64,
    servers: u32,
    horizon: Time<Seconds>,
    warmup: Time<Seconds>,
    replications: u32,
    seed: u64,
) -> Result<ValidationReport, QueueError> {
    let analytic = mmc(lambda, mu, servers)?;
    if replications < 2 || !(horizon.raw().is_finite() && horizon.raw() > 0.0) {
        return Err(QueueError::InvalidParameter);
    }
    if !(warmup.raw().is_finite() && (0.0..horizon.raw()).contains(&warmup.raw())) {
        return Err(QueueError::InvalidParameter);
    }

    let runs = map_replications(replications, |i| {
        let config = exponential_config(lambda, mu)?;
        Ok(run_queue_multi(
            config,
            servers as usize,
            horizon,
            warmup,
            seed.wrapping_add(u64::from(i)),
        ))
    })
    .into_iter()
    .collect::<Result<Vec<QueueMetrics>, QueueError>>()?;

    let column = |f: fn(&QueueMetrics) -> f64| runs.iter().map(f).collect::<Vec<f64>>();
    let rows = vec![
        validation_row("Wq", analytic.wq, &column(|m| m.mean_wait().raw())),
        validation_row("W", analytic.w, &column(|m| m.mean_sojourn().raw())),
        validation_row("L", analytic.l, &column(QueueMetrics::time_avg_number)),
        validation_row("rho", analytic.rho, &column(QueueMetrics::utilization)),
    ];
    Ok(ValidationReport { servers, rows })
}

/// Evaluate `f` for replication indices `0..replications`, in index order.
///
/// With the `parallel` feature the calls run on rayon's pool. Each call must
/// depend only on its index, so the result is the same either way.
#[cfg(feature = "parallel")]
fn map_replications<T, F>(replications: u32, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(u32) -> T + Sync + Send,
{
    use rayon::prelude::*;
    (0..replications).into_par_iter().map(f).collect()
}

/// Serial version of [`map_replications`], used without the `parallel` feature.
#[cfg(not(feature = "parallel"))]
fn map_replications<T, F>(replications: u32, f: F) -> Vec<T>
where
    F: Fn(u32) -> T,
{
    (0..replications).map(f).collect()
}

fn validation_row(metric: &'static str, analytic_value: f64, samples: &[f64]) -> ValidationRow {
    let simulated = Estimate::from_samples(samples);
    let band = BAND_SIGMAS * simulated.std_error + RELATIVE_SLACK * analytic_value.abs();
    ValidationRow {
        metric,
        analytic: analytic_value,
        simulated,
        passes: (simulated.mean - analytic_value).abs() <= band,
    }
}

fn exponential_config(lambda: f64, mu: f64) -> Result<QueueConfig, QueueError> {
    Ok(QueueConfig {
        arrival: Box::new(Exponential::new(lambda)?),
        service: Box::new(Exponential::new(mu)?),
        discipline: Discipline::Fifo,
        capacity: None,
        priority: None,
    })
}

/// The result of [`mean_wait_to_precision`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct PrecisionRun {
    /// Mean queue wait over the replications run.
    pub estimate: Estimate,
    /// Half-width of the Student-t confidence interval at the requested level.
    pub half_width: f64,
    /// True if the half-width reached the target. False means the replication
    /// budget ran out first.
    pub converged: bool,
}

/// Run replications of a single-queue simulation until the confidence
/// half-width of the mean queue wait is at most `target_half_width`, or until
/// `max_replications` have run.
///
/// `config` builds a fresh [`QueueConfig`] for each replication, because a
/// config owns its samplers. Replication `i` uses seed `seed + i`.
///
/// # Errors
///
/// [`QueueError::InvalidParameter`] if the target is not finite and positive,
/// `confidence` is outside `(0, 1)`, `servers` is zero, `max_replications` is
/// below [`MIN_REPLICATIONS`], or the horizon or warm-up is invalid.
///
/// # Panics
///
/// As [`run_queue_multi`] for an invalid
/// processor-sharing configuration.
#[allow(clippy::too_many_arguments)]
pub fn mean_wait_to_precision(
    mut config: impl FnMut() -> QueueConfig,
    servers: usize,
    horizon: Time<Seconds>,
    warmup: Time<Seconds>,
    target_half_width: Time<Seconds>,
    confidence: f64,
    max_replications: u32,
    seed: u64,
) -> Result<PrecisionRun, QueueError> {
    if !(target_half_width.raw().is_finite() && target_half_width.raw() > 0.0)
        || !(confidence > 0.0 && confidence < 1.0)
        || servers == 0
        || max_replications < MIN_REPLICATIONS
        || !(horizon.raw().is_finite() && horizon.raw() > 0.0)
        || !(warmup.raw().is_finite() && (0.0..horizon.raw()).contains(&warmup.raw()))
    {
        return Err(QueueError::InvalidParameter);
    }

    let mut waits = Vec::with_capacity(max_replications as usize);
    for i in 0..max_replications {
        let metrics = run_queue_multi(
            config(),
            servers,
            horizon,
            warmup,
            seed.wrapping_add(u64::from(i)),
        );
        waits.push(metrics.mean_wait().raw());
        if (waits.len() as u32) < MIN_REPLICATIONS {
            continue;
        }
        let estimate = Estimate::from_samples(&waits);
        let df = waits.len() - 1;
        let half_width = student_t_critical(confidence, df) * estimate.std_error;
        if half_width <= target_half_width.raw() {
            return Ok(PrecisionRun {
                estimate,
                half_width,
                converged: true,
            });
        }
    }

    let estimate = Estimate::from_samples(&waits);
    let half_width = student_t_critical(confidence, waits.len() - 1) * estimate.std_error;
    Ok(PrecisionRun {
        estimate,
        half_width,
        converged: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_des_core::Time;

    #[test]
    fn sizing_finds_the_fewest_servers() {
        // λ = 4, μ = 1: at least 5 servers for stability. Check the result is
        // minimal by confirming one fewer server misses the target.
        let target = 0.2;
        let s = size_for_mean_wait(4.0, 1.0, Time::from_raw(target)).unwrap();
        assert!(s.analytic.wq <= target);
        assert!(s.servers >= 5);
        let fewer = mmc(4.0, 1.0, s.servers - 1);
        match fewer {
            Ok(r) => assert!(
                r.wq > target,
                "{} servers also meet the target",
                s.servers - 1
            ),
            Err(e) => assert_eq!(e, QueueError::Unstable),
        }
    }

    #[test]
    fn sizing_rejects_bad_targets() {
        assert_eq!(
            size_for_mean_wait(1.0, 1.0, Time::from_raw(0.0)),
            Err(QueueError::InvalidParameter)
        );
        assert_eq!(
            size_for_mean_wait(1.0, 1.0, Time::from_raw(-1.0)),
            Err(QueueError::InvalidParameter)
        );
    }

    #[test]
    fn validation_passes_for_a_correct_model() {
        // ρ = 0.8 on three servers: a regime where the wait is non-trivial.
        let report = validate_mmc(
            2.4,
            1.0,
            3,
            Time::from_raw(20_000.0),
            Time::from_raw(2_000.0),
            6,
            3,
        )
        .unwrap();
        assert_eq!(report.rows.len(), 4);
        report.assert_passes();
    }

    #[test]
    fn validation_report_is_reproducible_and_ordered() {
        // With the `parallel` feature the replications run on a pool; the report
        // must not depend on scheduling.
        let a = validate_mmc(
            2.4,
            1.0,
            3,
            Time::from_raw(10_000.0),
            Time::from_raw(1_000.0),
            4,
            9,
        )
        .unwrap();
        let b = validate_mmc(
            2.4,
            1.0,
            3,
            Time::from_raw(10_000.0),
            Time::from_raw(1_000.0),
            4,
            9,
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(
            map_replications(6, |i| i * 3),
            (0..6).map(|i| i * 3).collect::<Vec<u32>>()
        );
    }

    #[test]
    fn validation_flags_a_wrong_analytic_value() {
        // Compare a 3-server simulation against the analytic value for 1
        // server: the report must say no.
        let mut report = validate_mmc(
            0.8,
            1.0,
            3,
            Time::from_raw(20_000.0),
            Time::from_raw(2_000.0),
            4,
            5,
        )
        .unwrap();
        let wrong = mmc(0.8, 1.0, 1).unwrap().wq;
        let row = report.rows.iter_mut().find(|r| r.metric == "Wq").unwrap();
        row.analytic = wrong;
        row.passes = (row.simulated.mean - wrong).abs()
            <= BAND_SIGMAS * row.simulated.std_error + RELATIVE_SLACK * wrong.abs();
        assert!(!row.passes, "a 3-server run should not match 1-server Wq");
    }

    #[test]
    fn precision_driver_reaches_the_target_or_reports_it_did_not() {
        let mk = || QueueConfig {
            arrival: Box::new(Exponential::new(0.5).unwrap()),
            service: Box::new(Exponential::new(1.0).unwrap()),
            discipline: Discipline::Fifo,
            capacity: None,
            priority: None,
        };
        let loose = mean_wait_to_precision(
            mk,
            1,
            Time::from_raw(5_000.0),
            Time::from_raw(500.0),
            Time::from_raw(0.5),
            0.95,
            20,
            1,
        )
        .unwrap();
        assert!(loose.converged);
        assert!(loose.half_width <= 0.5);

        // An unreachable target with a tiny budget must say it did not converge.
        let tight = mean_wait_to_precision(
            mk,
            1,
            Time::from_raw(500.0),
            Time::from_raw(50.0),
            Time::from_raw(1e-9),
            0.95,
            3,
            1,
        )
        .unwrap();
        assert!(!tight.converged);
        assert_eq!(tight.estimate.replications, 3);
    }
}
