//! Regression tests for the Phase 6 queue fixes.

use tpt_des_queue::sim::FromClosure;
use tpt_des_queue::{run_queue, Discipline, Exponential, QueueConfig};
use tpt_math_prob_core::Rng as _;

fn mm1(lambda: f64, mu: f64, discipline: Discipline) -> QueueConfig {
    QueueConfig {
        arrival: Box::new(Exponential::new(lambda).unwrap()),
        service: Box::new(Exponential::new(mu).unwrap()),
        discipline,
        capacity: None,
        priority: None,
    }
}

/// Two priority classes on an M/M/1 queue. Before the fix every job was
/// hard-coded to priority 0, so `Discipline::Priority` was exactly FIFO and
/// the two class waits were identical.
#[test]
fn priority_discipline_serves_urgent_class_first() {
    let (lambda, mu) = (0.8, 1.0);
    let two_class = |discipline| QueueConfig {
        priority: Some(Box::new(|rng| if rng.next_f64() < 0.3 { 0 } else { 1 })),
        ..mm1(lambda, mu, discipline)
    };

    let prio = run_queue(two_class(Discipline::Priority), 60_000.0, 6_000.0, 21);
    let w_urgent = prio.mean_wait_for_priority(0).expect("class 0 served");
    let w_bulk = prio.mean_wait_for_priority(1).expect("class 1 served");
    assert!(
        w_urgent < 0.6 * w_bulk,
        "urgent wait {w_urgent} should be well below bulk wait {w_bulk}"
    );

    // Non-preemptive priority is work-conserving: the overall mean wait
    // matches FIFO (Kleinrock's conservation law), even though the split
    // between classes does not.
    let fifo = run_queue(two_class(Discipline::Fifo), 60_000.0, 6_000.0, 21);
    let rel = (prio.mean_wait() - fifo.mean_wait()).abs() / fifo.mean_wait();
    assert!(rel < 0.15, "overall waits differ by {rel:.3} (relative)");
}

/// Deterministic arrivals every 1 s, service 0.75 s: the system holds one
/// job for [k, k + 0.75) and none otherwise. Measured over the window
/// [10.25, 20], the exact time-average population is 7.25 / 9.75.
///
/// Before the fix, the population integral restarted at the first event
/// after the warm-up cutoff (t = 11), so the window was 9 s with mass 6.75
/// and the average came out as 0.75.
#[test]
fn time_average_is_exact_over_warmup_window_and_closed_at_horizon() {
    let config = QueueConfig {
        arrival: Box::new(FromClosure::new("every-1s", |_| 1.0)),
        service: Box::new(FromClosure::new("0.75s", |_| 0.75)),
        discipline: Discipline::Fifo,
        capacity: None,
        priority: None,
    };
    let m = run_queue(config, 20.0, 10.25, 1);
    let expected = 7.25 / 9.75;
    assert!(
        (m.time_avg_number() - expected).abs() < 1e-9,
        "time-average {} vs exact {expected}",
        m.time_avg_number()
    );
    assert!((m.utilization() - expected).abs() < 1e-9);
}

#[test]
#[should_panic(expected = "inter-arrival sample must be finite and non-negative")]
fn nan_interarrival_sample_is_rejected() {
    let config = QueueConfig {
        arrival: Box::new(FromClosure::new("nan", |_| f64::NAN)),
        service: Box::new(Exponential::new(1.0).unwrap()),
        discipline: Discipline::Fifo,
        capacity: None,
        priority: None,
    };
    run_queue(config, 10.0, 0.0, 1);
}

#[test]
#[should_panic(expected = "service sample must be finite and non-negative")]
fn negative_service_sample_is_rejected() {
    let config = QueueConfig {
        arrival: Box::new(Exponential::new(0.5).unwrap()),
        service: Box::new(FromClosure::new("neg", |_| -1.0)),
        discipline: Discipline::Fifo,
        capacity: None,
        priority: None,
    };
    run_queue(config, 10.0, 0.0, 1);
}

#[test]
#[should_panic(expected = "warm-up must lie in [0, horizon)")]
fn warmup_not_before_horizon_is_rejected() {
    run_queue(mm1(0.5, 1.0, Discipline::Fifo), 100.0, 100.0, 1);
}

#[test]
#[should_panic(expected = "horizon must be finite and positive")]
fn nan_horizon_is_rejected() {
    run_queue(mm1(0.5, 1.0, Discipline::Fifo), f64::NAN, 0.0, 1);
}

/// Erlang C via the Erlang-B recursion (numerically stable at any c):
/// C = c·B / (c − a(1 − B)).
fn erlang_c_from_erlang_b(a: f64, c: u32) -> f64 {
    let b = tpt_des_queue::erlang_b(a, c).unwrap();
    c as f64 * b / (c as f64 - a * (1.0 - b))
}

/// The old code summed `aⁿ/n!` directly, which overflows near c ≈ 170 and
/// produced NaN for large server counts.
#[test]
fn mmc_is_finite_and_accurate_for_many_servers() {
    for &c in &[171u32, 500, 2000] {
        let a = 0.9 * c as f64;
        let r = tpt_des_queue::mmc(a, 1.0, c).unwrap();
        assert!(
            r.erlang_c.is_finite() && (0.0..=1.0).contains(&r.erlang_c),
            "c={c}"
        );
        assert!(r.p0.is_finite() && r.p0 >= 0.0, "c={c}");
        let expected = erlang_c_from_erlang_b(a, c);
        assert!(
            (r.erlang_c - expected).abs() < 1e-9,
            "c={c}: C={} expected {expected}",
            r.erlang_c
        );
        assert!((r.lq - r.erlang_c * r.rho / (1.0 - r.rho)).abs() < 1e-9 * r.lq.max(1.0));
    }
}

#[test]
fn erlang_c_with_no_traffic_is_zero() {
    assert_eq!(tpt_des_queue::erlang_c(0.0, 5), Ok(0.0));
    // Still consistent with the small-c closed form at positive load.
    let c = tpt_des_queue::erlang_c(2.0, 3).unwrap();
    assert!((c - erlang_c_from_erlang_b(2.0, 3)).abs() < 1e-12);
}

/// `n as i32` wrapped for n > i32::MAX, so `ρⁿ` came out enormous instead
/// of ~0.
#[test]
fn mm1_pn_does_not_wrap_for_huge_n() {
    let p = tpt_des_queue::mm1_pn(0.5, 1.0, 1u32 << 31).unwrap();
    assert!(p.is_finite() && p < 1e-300, "p = {p:e}");
}

#[test]
fn network_with_zero_external_node_has_zero_rate_not_error() {
    use tpt_des_queue::JacksonNetwork;
    // Node 1 receives no external traffic and no routing into it.
    let net =
        JacksonNetwork::new(&[1.0, 0.0], &[vec![0.0, 0.5], vec![0.0, 0.0]], &[2.0, 2.0]).unwrap();
    let lambda = net.node_arrivals().unwrap();
    assert!((lambda[0] - 1.0).abs() < 1e-12);
    assert_eq!(lambda[1], 0.5);
}

/// With every job in one priority class, the heap-based priority line must
/// serve in arrival order, exactly as FIFO does. The priority closure draws no
/// randomness, so both runs see the same random stream.
#[test]
fn single_class_priority_matches_fifo_exactly() {
    let (lambda, mu) = (0.7, 1.0);
    let same_class = QueueConfig {
        priority: Some(Box::new(|_| 0)),
        ..mm1(lambda, mu, Discipline::Priority)
    };
    let prio = run_queue(same_class, 20_000.0, 2_000.0, 5);
    let fifo = run_queue(mm1(lambda, mu, Discipline::Fifo), 20_000.0, 2_000.0, 5);
    assert_eq!(prio, fifo);
}

/// Under processor sharing the server is busy whenever anything is in
/// service. `utilization` used to add the job count, so it returned the mean
/// number in system (about ρ/(1−ρ) = 1.5 at ρ = 0.6) instead of ρ.
#[test]
fn processor_sharing_utilization_is_busy_fraction_not_population() {
    let (lambda, mu) = (0.6, 1.0);
    let m = run_queue(
        mm1(lambda, mu, Discipline::ProcessorSharing),
        40_000.0,
        4_000.0,
        9,
    );
    let rho = lambda / mu;
    assert!(
        (m.utilization() - rho).abs() < 0.02,
        "PS utilization {} should be ≈ ρ = {rho}",
        m.utilization()
    );
    assert!(m.utilization() <= 1.0);
}
