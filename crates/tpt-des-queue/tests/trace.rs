//! Trace export: per-job records agree with the aggregate metrics, and the
//! CSV carries one row per record.

use tpt_des_core::Time;
use tpt_des_queue::sim::{run_queue_traced, trace_to_csv};
use tpt_des_queue::{Discipline, Exponential, QueueConfig};

fn mm1(lambda: f64, mu: f64) -> QueueConfig {
    QueueConfig {
        arrival: Box::new(Exponential::new(lambda).unwrap()),
        service: Box::new(Exponential::new(mu).unwrap()),
        discipline: Discipline::Fifo,
        capacity: None,
        priority: None,
    }
}

#[test]
fn single_server_fifo_trace_is_ordered_and_matches_metrics() {
    let (horizon, warmup) = (5_000.0, 500.0);
    let (metrics, records) = run_queue_traced(
        mm1(0.6, 1.0),
        1,
        Time::from_raw(horizon),
        Time::from_raw(warmup),
        17,
    );

    // FIFO on one server: service starts in arrival order.
    for pair in records.windows(2) {
        assert!(pair[0].job < pair[1].job);
        assert!(pair[0].started_at <= pair[1].started_at);
    }
    for r in &records {
        assert!(r.wait().raw() >= 0.0, "negative wait in {r:?}");
        assert!(r.started_at <= r.departed_at);
        assert!(r.departed_at <= Time::from_raw(horizon));
    }

    // Records after warm-up are exactly the jobs the metrics counted.
    let counted: Vec<_> = records
        .iter()
        .filter(|r| r.arrived_at >= Time::from_raw(warmup))
        .collect();
    assert_eq!(counted.len() as u64, metrics.completed);
    let total_sojourn: f64 = counted.iter().map(|r| r.sojourn().raw()).sum();
    assert!((total_sojourn - metrics.total_sojourn.raw()).abs() < 1e-6 * total_sojourn.max(1.0));
}

#[test]
fn csv_has_a_header_and_one_row_per_record() {
    let (_, records) = run_queue_traced(
        mm1(0.5, 1.0),
        1,
        Time::from_raw(1_000.0),
        Time::from_raw(100.0),
        3,
    );
    let csv = trace_to_csv(&records);
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some("job,priority,arrived_at,started_at,departed_at,wait,sojourn")
    );
    assert_eq!(lines.count(), records.len());
}

#[test]
fn json_has_one_object_per_record() {
    use tpt_des_queue::trace_to_json;
    let (_, records) = run_queue_traced(
        mm1(0.5, 1.0),
        1,
        Time::from_raw(1_000.0),
        Time::from_raw(100.0),
        4,
    );
    let json = trace_to_json(&records);
    assert!(json.starts_with('[') && json.ends_with(']'));
    assert_eq!(json.matches('{').count(), records.len());
    assert_eq!(json.matches('}').count(), records.len());
    assert_eq!(json.matches("\"job\":").count(), records.len());
}

#[test]
fn chrome_trace_has_two_spans_per_job_and_lanes_within_servers() {
    use tpt_des_queue::trace_to_chrome;
    let servers = 2;
    let (_, records) = run_queue_traced(
        mm1(1.2, 1.0),
        servers,
        Time::from_raw(2_000.0),
        Time::from_raw(200.0),
        8,
    );
    let chrome = trace_to_chrome(&records);
    assert!(chrome.starts_with("{\"traceEvents\":["));
    assert_eq!(chrome.matches("\"ph\":\"X\"").count(), 2 * records.len());

    // Service lanes are tids ≥ 1. Peak in service never exceeds the servers,
    // so greedy packing never needs a lane beyond that.
    let max_lane = chrome
        .split("\"name\":\"service\"")
        .skip(1)
        .map(|rest| {
            let after = rest.split("\"tid\":").nth(1).unwrap();
            after.split(',').next().unwrap().parse::<usize>().unwrap()
        })
        .max()
        .unwrap();
    assert!(
        max_lane <= servers,
        "lane {max_lane} exceeds {servers} servers"
    );
}

#[test]
fn fingerprint_is_stable_per_seed_and_sensitive_to_timing() {
    use tpt_des_queue::trace_fingerprint;
    let run = |seed| {
        run_queue_traced(
            mm1(0.6, 1.0),
            1,
            Time::from_raw(2_000.0),
            Time::from_raw(200.0),
            seed,
        )
        .1
    };
    let a = run(3);
    assert_eq!(trace_fingerprint(3, &a), trace_fingerprint(3, &run(3)));
    assert_ne!(trace_fingerprint(3, &a), trace_fingerprint(4, &a));

    let mut changed = a.clone();
    changed[0].departed_at += tpt_des_core::Time::from_raw(1e-9);
    assert_ne!(trace_fingerprint(3, &a), trace_fingerprint(3, &changed));
}
