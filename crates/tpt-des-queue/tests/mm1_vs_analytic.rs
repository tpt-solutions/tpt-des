//! Phase 1 integration test: an M/M/1 discrete-event simulation must match
//! the analytical (Erlang/Cromer) steady-state formulas, across the stable
//! regime, with the state verified by `proptest`.
//!
//! Also covers M/M/1/K blocking versus the Erlang B formula, and the M/M/1
//! processor-sharing insensitivity result.

use proptest::prelude::*;
use tpt_des_core::Time;
use tpt_des_queue::{mm1, mm1_pn, run_queue, Discipline, Exponential, QueueConfig};
use tpt_des_verify::strategies::stable_queue_params;

fn mm1_config(lambda: f64, mu: f64, discipline: Discipline) -> QueueConfig {
    QueueConfig {
        arrival: Box::new(Exponential::new(lambda).unwrap()),
        service: Box::new(Exponential::new(mu).unwrap()),
        discipline,
        capacity: None,
        priority: None,
    }
}

/// Independent replications averaged for the steady-state checks. One run is
/// too noisy near ρ = 0.95 to support a tight tolerance; the mean of five
/// independent seeds is not.
const REPLICATIONS: u64 = 5;

/// Mean of `metrics` over `REPLICATIONS` seeds, as (Wq, W, L, ρ).
fn replicated_means(
    lambda: f64,
    mu: f64,
    discipline: Discipline,
    horizon: f64,
    warmup: f64,
    seed: u64,
) -> [f64; 4] {
    let mut sums = [0.0; 4];
    for i in 0..REPLICATIONS {
        let m = run_queue(
            mm1_config(lambda, mu, discipline),
            Time::from_raw(horizon),
            Time::from_raw(warmup),
            seed.wrapping_add(i),
        );
        // Need enough completions in each replication for a meaningful sample.
        assert!(m.completed > 1_000, "too few completions: {}", m.completed);
        sums[0] += m.mean_wait().raw();
        sums[1] += m.mean_sojourn().raw();
        sums[2] += m.time_avg_number();
        sums[3] += m.utilization();
    }
    sums.map(|s| s / REPLICATIONS as f64)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn mm1_simulation_matches_analytical_wq(
        (lambda, mu) in stable_queue_params(),
        seed in any::<u64>(),
    ) {
        // Near ρ → 1 the relaxation time grows like 1/(1−√ρ)² and waits are
        // heavily autocorrelated, so scale the run length with the load.
        let rho = lambda / mu;
        let load_scale = 1.0 / (1.0 - rho.sqrt()).max(1e-3);
        let horizon = 40_000.0 * load_scale.min(25.0);
        let warmup = horizon / 10.0;
        let [wq, w, l, util] =
            replicated_means(lambda, mu, Discipline::Fifo, horizon, warmup, seed);
        let exact = mm1(lambda, mu).unwrap();

        // Tolerance on the replication mean: 10% for the waiting and number
        // metrics, 5% for utilization, which is a much less noisy estimator.
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-9);
        prop_assert!(rel(wq, exact.wq) < 0.10,
            "Wq sim {} vs analytic {} (lambda={}, mu={})", wq, exact.wq, lambda, mu);
        prop_assert!(rel(w, exact.w) < 0.10, "W sim {} vs analytic {}", w, exact.w);
        prop_assert!(rel(l, exact.l) < 0.10, "L sim {} vs analytic {}", l, exact.l);
        prop_assert!(rel(util, exact.rho) < 0.05,
            "utilization sim {} vs analytic {}", util, exact.rho);
    }

    #[test]
    fn mm1_bounded_queue_blocks_per_erlang_b(
        (lambda, mu) in stable_queue_params(),
        seed in any::<u64>(),
    ) {
        // K = 1: system holds one job; blocking = Erlang B at offered load a.
        let k = 1usize;
        let mut config = mm1_config(lambda, mu, Discipline::Fifo);
        config.capacity = Some(k);
        let horizon = 40_000.0;
        let metrics = run_queue(config, Time::from_raw(horizon), Time::from_raw(horizon / 10.0), seed);
        let a = lambda / mu;
        let b = tpt_des_queue::erlang_b(a, k as u32).unwrap();

        let attempts = metrics.arrived + metrics.blocked;
        prop_assert!(attempts > 1_000);
        let observed = metrics.blocked as f64 / attempts as f64;
        prop_assert!((observed - b).abs() < 0.05,
            "blocking sim {observed} vs Erlang B {b} (a={a})");
    }

    #[test]
    fn mm1_stationary_distribution_holds(
        (lambda, mu) in stable_queue_params(),
    ) {
        // The geometric mass decays, and the first 50 terms almost exhaust
        // the total probability: Σ_{n<50} (1−ρ)ρⁿ = 1 − ρ⁵⁰.
        let rho = lambda / mu;
        let p0 = mm1_pn(lambda, mu, 0).unwrap();
        let p1 = mm1_pn(lambda, mu, 1).unwrap();
        let p2 = mm1_pn(lambda, mu, 2).unwrap();
        prop_assert!((p0 - (1.0 - rho)).abs() < 1e-12);
        prop_assert!((p1 - rho * p0).abs() < 1e-12);
        prop_assert!(p1 > p2);
        let mass: f64 = (0..50).map(|n| mm1_pn(lambda, mu, n).unwrap()).sum();
        prop_assert!((mass - (1.0 - rho.powi(50))).abs() < 1e-9);
    }
}

#[test]
fn processor_sharing_matches_mm1_mean_sojourn() {
    // M/M/1-PS has the same mean sojourn as M/M/1-FCFS (insensitivity).
    let (lambda, mu) = (0.6, 1.0);
    let exact = mm1(lambda, mu).unwrap();
    let [_, w, _, _] = replicated_means(
        lambda,
        mu,
        Discipline::ProcessorSharing,
        40_000.0,
        4_000.0,
        7,
    );
    assert!(
        (w - exact.w).abs() / exact.w < 0.10,
        "PS sojourn {} vs FCFS analytic {}",
        w,
        exact.w
    );
}

#[test]
fn fifo_lifo_agree_on_mean_sojourn() {
    // For M/M/1 with a single class, every work-conserving non-preemptive
    // discipline shares the same mean sojourn time (conservation law).
    // Priority is checked separately: with one class it is FIFO, and with
    // several classes it must differ per class (see tests/regressions.rs).
    let (lambda, mu) = (0.6, 1.0);
    let exact = mm1(lambda, mu).unwrap();
    for d in [Discipline::Fifo, Discipline::Lifo] {
        let [_, w, _, _] = replicated_means(lambda, mu, d, 40_000.0, 4_000.0, 11);
        assert!(
            (w - exact.w).abs() / exact.w < 0.10,
            "{d:?}: sojourn {} vs analytic {}",
            w,
            exact.w
        );
    }
}

#[test]
fn deterministic_replay_reproduces_metrics() {
    let config = || mm1_config(0.7, 1.0, Discipline::Fifo);
    let a = run_queue(
        config(),
        Time::from_raw(10_000.0),
        Time::from_raw(1_000.0),
        123,
    );
    let b = run_queue(
        config(),
        Time::from_raw(10_000.0),
        Time::from_raw(1_000.0),
        123,
    );
    assert_eq!(a, b);
    let c = run_queue(
        config(),
        Time::from_raw(10_000.0),
        Time::from_raw(1_000.0),
        124,
    );
    assert_ne!(a, c);
}
