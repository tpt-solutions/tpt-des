//! Regression tests for the Phase 6 resource fixes.

use tpt_des_resource::deadlock::WaitForGraph;
use tpt_des_resource::pool::ResourcePool;
use tpt_des_resource::store::{Buffer, Store};
use tpt_des_resource::{
    PreemptPolicy, ResourceController, ResourceError, SeizeOutcome, SeizeRequest,
};

/// `held_fraction` used to return the inverse (free / capacity).
#[test]
fn held_fraction_is_the_held_share() {
    let mut pool = ResourcePool::new(4).unwrap();
    assert_eq!(pool.held_fraction(), 0.0);
    assert!(pool.acquire(1));
    assert!((pool.held_fraction() - 0.25).abs() < 1e-12);
}

/// A request larger than capacity can never be granted; it is now rejected
/// up front instead of blocking the queue forever.
#[test]
fn seize_beyond_capacity_is_rejected() {
    let mut rc = ResourceController::new();
    rc.add_pool(0, 2).unwrap();
    assert_eq!(
        rc.seize(0, SeizeRequest::new(0, 3)),
        Err(ResourceError::InvalidParameter)
    );
    assert_eq!(rc.waiting_count(0), 0);
}

/// FIFO fairness: a free unit must not be handed to a later small request
/// while an earlier larger request is still queued for the same resource.
/// Before the fix, the small request was granted immediately (starvation).
#[test]
fn new_seize_cannot_bypass_queued_waiter() {
    let mut rc = ResourceController::new();
    rc.add_pool(0, 2).unwrap();
    assert!(matches!(
        rc.seize(1, SeizeRequest::new(0, 1)),
        Ok(SeizeOutcome::Granted(_))
    ));
    // Needs both units: queued (one unit is free).
    assert!(matches!(
        rc.seize(2, SeizeRequest::new(0, 2)),
        Ok(SeizeOutcome::Queued(_))
    ));
    // Would fit in the free unit, but must wait behind entity 2.
    assert!(matches!(
        rc.seize(3, SeizeRequest::new(0, 1)),
        Ok(SeizeOutcome::Queued(_))
    ));
    assert_eq!(rc.held(3, 0), 0);
    // Once entity 1 releases, the head (entity 2) goes first.
    assert_eq!(rc.release(1, 0).unwrap(), vec![2]);
}

/// Priority policies only let a request jump ahead of waiters that are no
/// more urgent than it.
#[test]
fn priority_seize_jumps_only_less_urgent_waiters() {
    let mut rc = ResourceController::with_policy(PreemptPolicy::Priority);
    rc.add_pool(0, 1).unwrap();
    rc.seize(1, SeizeRequest::new(0, 1)).unwrap();
    rc.seize(2, SeizeRequest::new(0, 1).with_priority(5))
        .unwrap();
    // More urgent than the waiter: granted from the queue order on release.
    rc.seize(3, SeizeRequest::new(0, 1).with_priority(-1))
        .unwrap();
    assert_eq!(rc.release(1, 0).unwrap(), vec![3]);
}

/// An entity with two queued requests must keep both wait-for edges. The old
/// graph stored one wait per entity, so the second request erased the first
/// and this cycle (E0 waits on R1 held by E1; E1 waits on R0 held by E0) went
/// undetected.
#[test]
fn deadlock_detected_when_entity_has_several_queued_requests() {
    let mut rc = ResourceController::new();
    for r in 0..3 {
        rc.add_pool(r, 1).unwrap();
    }
    rc.seize(0, SeizeRequest::new(0, 1)).unwrap(); // E0 holds R0
    rc.seize(1, SeizeRequest::new(1, 1)).unwrap(); // E1 holds R1
    rc.seize(2, SeizeRequest::new(2, 1)).unwrap(); // E2 holds R2
    rc.seize(0, SeizeRequest::new(1, 1)).unwrap(); // E0 waits on R1
    rc.seize(0, SeizeRequest::new(2, 1)).unwrap(); // E0 also waits on R2
    rc.seize(1, SeizeRequest::new(0, 1)).unwrap(); // E1 waits on R0
    let cycle = rc.detect_deadlock().expect("deadlock must be found");
    assert!(cycle.contains(&0) && cycle.contains(&1), "{cycle:?}");
}

#[test]
fn granting_one_request_keeps_the_other_wait_edge() {
    let mut rc = ResourceController::new();
    rc.add_pool(1, 1).unwrap();
    rc.add_pool(2, 1).unwrap();
    rc.seize(9, SeizeRequest::new(1, 1)).unwrap(); // E9 holds R1
    rc.seize(9, SeizeRequest::new(2, 1)).unwrap(); // E9 holds R2
    rc.seize(0, SeizeRequest::new(1, 1)).unwrap(); // E0 waits on R1
    rc.seize(0, SeizeRequest::new(2, 1)).unwrap(); // E0 waits on R2
    rc.release(9, 1).unwrap(); // R1 granted to E0
    assert_eq!(rc.held(0, 1), 1);
    // The R2 wait is still outstanding.
    assert_eq!(rc.wait_graph().waiting_for(0), Some(2));
}

#[test]
fn wait_for_graph_counts_repeated_requests() {
    let mut g = WaitForGraph::default();
    g.add_request(0, 5);
    g.add_request(0, 5);
    assert!(g.remove_request(0, 5));
    assert_eq!(
        g.waiting_for(0),
        Some(5),
        "one request is still outstanding"
    );
    assert!(g.remove_request(0, 5));
    assert_eq!(g.waiting_for(0), None);
}

/// `put` used to add the accepted amount to a level near `i64::MAX`, which
/// overflowed (panic in debug, wrap in release).
#[test]
fn store_does_not_overflow_at_i64_max() {
    let mut s = Store::unbounded();
    assert_eq!(s.put(i64::MAX), i64::MAX);
    assert_eq!(s.put(5), 0);
    assert_eq!(s.level(), i64::MAX);
}

/// NaN used to flow through `clamp` and poison the buffer level.
#[test]
fn buffer_refuses_nan_amounts() {
    let mut b = Buffer::new(10.0).unwrap();
    b.pour(4.0);
    assert_eq!(b.pour(f64::NAN), 0.0);
    assert_eq!(b.drain(f64::NAN), 0.0);
    assert_eq!(b.level(), 4.0);
    assert_eq!(b.pour_duration(f64::NAN, 1.0), None);
}
