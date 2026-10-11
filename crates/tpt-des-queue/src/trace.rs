//! Trace exporters for completed jobs.
//!
//! * [`trace_to_json`]: a JSON array with one object per job.
//! * [`trace_to_chrome`]: the Chrome Trace Event format, which loads in
//!   `chrome://tracing` and Perfetto. Each job gets a `wait` span on lane 0 and
//!   a `service` span on a lane of its own, so overlapping service is visible.
//! * [`trace_fingerprint`]: a 64-bit digest of a seed and its records, for
//!   checking that two runs replayed the same way.
//!
//! The JSON is written by hand, so these exporters need no serde feature.

use crate::sim::JobRecord;
use core::fmt::Write as _;
use std::string::String;
use std::vec::Vec;

/// Render job records as a JSON array. Times are in the simulation's time
/// unit (seconds), and every value is a finite number, so the output is valid
/// JSON.
pub fn trace_to_json(records: &[JobRecord]) -> String {
    let mut out = String::from("[");
    for (i, r) in records.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"job\":{},\"priority\":{},\"arrived_at\":{},\"started_at\":{},\"departed_at\":{}}}",
            r.job,
            r.priority,
            r.arrived_at.raw(),
            r.started_at.raw(),
            r.departed_at.raw()
        );
    }
    out.push(']');
    out
}

/// Render job records in the Chrome Trace Event format, with times in
/// microseconds.
///
/// Service spans are packed onto lanes greedily by start time, so the number
/// of service lanes is the peak number of jobs in service at once (the server
/// count, for a run without processor sharing).
pub fn trace_to_chrome(records: &[JobRecord]) -> String {
    // Greedy interval packing: the first lane whose last span has ended.
    let mut order: Vec<usize> = (0..records.len()).collect();
    order.sort_by(|&a, &b| records[a].started_at.cmp(&records[b].started_at));
    let mut lane_end: Vec<f64> = Vec::new();
    let mut lane_of = std::vec![0usize; records.len()];
    for &i in &order {
        let start = records[i].started_at.raw();
        let lane = match lane_end.iter().position(|&end| end <= start) {
            Some(l) => l,
            None => {
                lane_end.push(0.0);
                lane_end.len() - 1
            }
        };
        lane_end[lane] = records[i].departed_at.raw();
        lane_of[i] = lane;
    }

    let mut out = String::from("{\"traceEvents\":[");
    let mut first = true;
    for (i, r) in records.iter().enumerate() {
        let wait = (r.started_at.raw() - r.arrived_at.raw()) * 1e6;
        let service = (r.departed_at.raw() - r.started_at.raw()) * 1e6;
        for (name, tid, ts, dur) in [
            ("wait", 0usize, r.arrived_at.raw() * 1e6, wait),
            ("service", lane_of[i] + 1, r.started_at.raw() * 1e6, service),
        ] {
            if !first {
                out.push(',');
            }
            first = false;
            let _ = write!(
                out,
                "{{\"name\":\"{name}\",\"cat\":\"job\",\"ph\":\"X\",\"pid\":0,\"tid\":{tid},\"ts\":{ts},\"dur\":{dur},\"args\":{{\"job\":{}}}}}",
                r.job
            );
        }
    }
    out.push_str("]}");
    out
}

/// A 64-bit fingerprint of a run: FNV-1a over the seed and each record's job
/// number, priority, and the bit patterns of its three times.
///
/// Two runs with the same seed and configuration give the same fingerprint.
/// A change in any job's timing changes it. It is a replay check, not a
/// cryptographic hash.
pub fn trace_fingerprint(seed: u64, records: &[JobRecord]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(PRIME);
        }
    };
    feed(&seed.to_le_bytes());
    for r in records {
        feed(&r.job.to_le_bytes());
        feed(&r.priority.to_le_bytes());
        feed(&r.arrived_at.raw().to_bits().to_le_bytes());
        feed(&r.started_at.raw().to_bits().to_le_bytes());
        feed(&r.departed_at.raw().to_bits().to_le_bytes());
    }
    hash
}
