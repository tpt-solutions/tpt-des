//! Worked example: resource contention with preemption-free seize-delay-
//! release — the classic two-entity cross-seize deadlock, its detection via
//! wait-for-graph cycle detection, and resolution by a timeout cancellation.

use tpt_des::resource::{ResourceController, SeizeOutcome, SeizeRequest};

fn main() {
    let mut rc = ResourceController::new();
    rc.add_pool(0, 1).unwrap(); // resource 0: one unit (e.g. printer A)
    rc.add_pool(1, 1).unwrap(); // resource 1: one unit (e.g. printer B)

    // Each entity grabs its own resource first…
    println!(
        "E0 seizes R0: {:?}",
        rc.seize(0, SeizeRequest::new(0, 1)).is_ok()
    );
    println!(
        "E1 seizes R1: {:?}",
        rc.seize(1, SeizeRequest::new(1, 1)).is_ok()
    );
    println!("deadlock now? {:?}", rc.detect_deadlock());

    // …then each wants the other's resource: both queue, deadlock forms.
    let _e0_wait = rc.seize(0, SeizeRequest::new(1, 1)).unwrap();
    let e1_wait = rc.seize(1, SeizeRequest::new(0, 1)).unwrap();
    let e1_request_id = match e1_wait {
        SeizeOutcome::Queued(id) => id,
        SeizeOutcome::Granted(_) => panic!("resource was free?"),
    };

    match rc.detect_deadlock() {
        Some(cycle) => println!("\nDEADLOCK detected, cycle: {cycle:?}"),
        None => println!("\nno deadlock (unexpected)"),
    }

    // Timeout resolution: E1's timer fires and cancels its queued request;
    // the cycle dissolves, E0's request can then be granted on release.
    let cancelled = rc.cancel(1, e1_request_id);
    println!("\nE1 timeout fired: cancelled = {cancelled}");
    println!("deadlock now? {:?}", rc.detect_deadlock());
    assert!(cancelled);
    assert!(rc.detect_deadlock().is_none());

    // E0 gets R1, finishes its work, and releases; the controller hands R1's
    // slot back and the system drains in FIFO order.
    // E0 was still queued on R1 when E1 cancelled; E0 finishes with its own
    // printer first (nothing waits behind it), then E1 finishes with R1,
    // which finally grants E0's long-pending request.
    println!(
        "E0 finishes, releases R0 -> granted to: {:?}",
        rc.release(0, 0)
    );
    println!(
        "E1 finishes, releases R1 -> granted to: {:?}",
        rc.release(1, 1)
    );
    println!(
        "E0 completes, releases R1 -> granted to: {:?}",
        rc.release(0, 1)
    );
    println!("\nsystem drained cleanly.");
}
