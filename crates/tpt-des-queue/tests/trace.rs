//! Trace export: per-job records agree with the aggregate metrics, and the
//! CSV carries one row per record.

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
    let (metrics, records) = run_queue_traced(mm1(0.6, 1.0), 1, horizon, warmup, 17);

    // FIFO on one server: service starts in arrival order.
    for pair in records.windows(2) {
        assert!(pair[0].job < pair[1].job);
        assert!(pair[0].started_at <= pair[1].started_at);
    }
    for r in &records {
        assert!(r.wait() >= 0.0, "negative wait in {r:?}");
        assert!(r.started_at <= r.departed_at);
        assert!(r.departed_at <= horizon);
    }

    // Records after warm-up are exactly the jobs the metrics counted.
    let counted: Vec<_> = records.iter().filter(|r| r.arrived_at >= warmup).collect();
    assert_eq!(counted.len() as u64, metrics.completed);
    let total_sojourn: f64 = counted.iter().map(|r| r.sojourn()).sum();
    assert!((total_sojourn - metrics.total_sojourn).abs() < 1e-6 * total_sojourn.max(1.0));
}

#[test]
fn csv_has_a_header_and_one_row_per_record() {
    let (_, records) = run_queue_traced(mm1(0.5, 1.0), 1, 1_000.0, 100.0, 3);
    let csv = trace_to_csv(&records);
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some("job,priority,arrived_at,started_at,departed_at,wait,sojourn")
    );
    assert_eq!(lines.count(), records.len());
}
