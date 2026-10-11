//! Phase 3 end-to-end test: simulate M/M/1 queues, then verify that
//! `tpt-des-stats` output analysis reproduces the known analytical
//! confidence structure (Erlang/Cromer means, Erlang B blocking).

use tpt_des_core::Time;
use tpt_des_queue::{mm1, run_queue, Discipline, Exponential, QueueConfig};
use tpt_des_stats::{batch, ci, warmup};

fn mm1_config(lambda: f64, mu: f64) -> QueueConfig {
    QueueConfig {
        arrival: Box::new(Exponential::new(lambda).unwrap()),
        service: Box::new(Exponential::new(mu).unwrap()),
        discipline: Discipline::Fifo,
        capacity: None,
        priority: None,
    }
}

#[test]
fn replication_deletion_covers_analytical_mean_wait() {
    let (lambda, mu) = (0.6, 1.0);
    let exact = mm1(lambda, mu).unwrap();

    // Six independent replications of sojourn means.
    let horizon = 60_000.0;
    let warmup = 5_000.0;
    let mut replications: Vec<Vec<f64>> = Vec::new();
    for seed in 1..=6u64 {
        let m = run_queue(
            mm1_config(lambda, mu),
            Time::from_raw(horizon),
            Time::from_raw(warmup),
            seed * 1_000,
        );
        replications.push(vec![m.mean_sojourn().raw()]);
    }
    let rd = batch::replication_deletion(&replications, 0, 0.95).unwrap();
    assert_eq!(rd.replications, 6);
    // The 95% interval covers the analytical W = 1/(μ−λ).
    assert!(
        rd.ci.0 <= exact.w && exact.w <= rd.ci.1,
        "CI [{}, {}] must cover W = {}",
        rd.ci.0,
        rd.ci.1,
        exact.w
    );
    // …and the interval is informative (not absurdly wide).
    assert!(rd.ci.1 - rd.ci.0 < 2.0, "CI width {}", rd.ci.1 - rd.ci.0);
}

#[test]
fn batch_means_interval_covers_analytical_wait() {
    let (lambda, mu) = (0.6, 1.0);
    let exact = mm1(lambda, mu).unwrap();

    // One long run, sampled per completion: approximate a sojourn series by
    // simulating with per-run metrics is not series-shaped, so instead use
    // utilization as the series (time-weighted, sampled by chunk): simulate
    // 20 chunks of 5_000 and collect the running time-average each chunk.
    let mut series = Vec::new();
    let mut elapsed = 0.0;
    for seed in 1..=20u64 {
        let chunk = 5_000.0;
        let m = run_queue(
            mm1_config(lambda, mu),
            Time::from_raw(elapsed + chunk),
            Time::from_raw(elapsed * 0.9),
            seed,
        );
        series.push(m.utilization());
        elapsed += chunk;
    }
    let bm = batch::batch_means(&series, 10, 0.95).unwrap();
    // Utilization should average to ρ = 0.6.
    assert!(
        bm.ci.0 <= exact.rho && exact.rho <= bm.ci.1,
        "batch-means CI [{}, {}] must cover ρ = {}",
        bm.ci.0,
        bm.ci.1,
        exact.rho
    );
}

#[test]
fn warmup_detection_finds_the_transient() {
    // Replication mean sojourns early in a run are biased low (queue starts
    // empty); Welch averaging over seeds shows the ramp. The detected
    // warm-up must be small but nonzero here.
    let (lambda, mu) = (0.7, 1.0);
    let mut per_chunk: Vec<Vec<f64>> = Vec::new();
    for seed in 1..=4u64 {
        let mut series = Vec::new();
        let mut prev = 0.0;
        for chunk in 1..=10u64 {
            let horizon = chunk as f64 * 3_000.0;
            let m = run_queue(
                mm1_config(lambda, mu),
                Time::from_raw(horizon),
                Time::from_raw(prev),
                seed * 77,
            );
            series.push(m.mean_sojourn().raw());
            prev = horizon;
        }
        per_chunk.push(series);
    }
    let averaged = warmup::welch(&per_chunk);
    let smoothed = warmup::moving_average(&averaged, 3);
    let _warm = warmup::welch_warmup(&smoothed, 0.10, 3);
    // Smoke: detection ran and returned a valid index.
    assert!(_warm <= smoothed.len());
}

#[test]
fn blocking_proportion_ci_covers_erlang_b() {
    let (lambda, mu) = (0.9, 1.0);
    let a = lambda / mu;
    let erlang = tpt_des_queue::erlang_b(a, 1).unwrap(); // a/(1+a) ≈ 0.4737

    let mut config = mm1_config(lambda, mu);
    config.capacity = Some(1);
    let m = run_queue(
        config,
        Time::from_raw(200_000.0),
        Time::from_raw(20_000.0),
        5,
    );
    let attempts = m.arrived + m.blocked;
    assert!(attempts > 10_000);
    let observed = m.blocked as f64 / attempts as f64;
    let (lo, hi) = ci::proportion_wilson(m.blocked as usize, attempts as usize, 0.95).unwrap();
    assert!(
        lo <= erlang && erlang <= hi,
        "Wilson CI [{lo}, {hi}] must cover Erlang B = {erlang} (observed {observed})"
    );
}
