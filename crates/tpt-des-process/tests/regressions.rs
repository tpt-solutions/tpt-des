//! Regression tests for the Phase 6 executor fixes.

use std::cell::Cell;
use std::rc::Rc;
use tpt_des_core::{Seconds, Time};
use tpt_des_process::{Interrupt, ProcessSystem, Sim};

/// Contended acquire: the waiter must be woken by `release` and complete.
/// Before the fix, the woken waiter was no longer at the queue front after
/// `release` had popped it, so it stayed pending forever.
#[test]
fn contended_acquire_completes_after_release() {
    let mut sys = ProcessSystem::new(1);
    sys.add_resource(0, 1);
    let done = Rc::new(Cell::new(0u32));

    let d1 = done.clone();
    sys.spawn(|sim: Sim| async move {
        sim.acquire(0, 1).await?;
        sim.delay(Time::from_raw(5.0)).await?;
        sim.release(0, 1);
        d1.set(d1.get() + 1);
        Ok(())
    });
    let d2 = done.clone();
    let second = sys.spawn(|sim: Sim| async move {
        sim.acquire(0, 1).await?; // queued behind the first holder
        sim.release(0, 1);
        d2.set(d2.get() + 1);
        Ok(())
    });

    sys.run();
    assert!(sys.is_finished(second), "contended waiter never completed");
    assert_eq!(done.get(), 2);
    assert_eq!(sys.now().raw(), 5.0);
}

/// FIFO: with capacity 2 and three single-unit waiters, the head goes first
/// and the next waiter is woken as soon as units free up.
#[test]
fn contended_acquire_fifo_with_multiple_units() {
    let mut sys = ProcessSystem::new(2);
    sys.add_resource(0, 2);
    let order = Rc::new(std::cell::RefCell::new(Vec::new()));
    for id in 0..4u32 {
        let order = order.clone();
        sys.spawn(move |sim: Sim| async move {
            sim.acquire(0, 1).await?;
            order.borrow_mut().push(id);
            sim.delay(Time::from_raw(1.0 + id as f64)).await?;
            sim.release(0, 1);
            Ok(())
        });
    }
    sys.run();
    assert_eq!(*order.borrow(), vec![0, 1, 2, 3]);
}

/// `notify_all` must not panic when a predicate reads simulation state.
/// Before the fix the predicate ran under a `RefCell` borrow and `Sim::available`
/// re-borrowed it.
#[test]
fn notify_all_predicate_can_read_sim_state() {
    let mut sys = ProcessSystem::new(3);
    sys.add_resource(0, 1);
    let waiter = sys.spawn(|sim: Sim| async move {
        sim.wait_until(|s| s.available(0) >= 1).await?;
        Ok(())
    });
    let handle = sys.handle();
    sys.spawn(move |sim: Sim| async move {
        sim.delay(Time::from_raw(2.0)).await?;
        // Make the condition true, then notify.
        sim.release(0, 1);
        sim.notify_all();
        Ok(())
    });
    sys.run();
    assert!(handle.is_finished(waiter));
    assert_eq!(sys.now().raw(), 2.0);
}

/// `wait_until` evaluates its predicate at registration: an already-true
/// condition completes without waiting for any notification.
#[test]
fn wait_until_true_at_registration_does_not_block() {
    let mut sys = ProcessSystem::new(4);
    let pid = sys.spawn(|sim: Sim| async move {
        sim.wait_until(|_| true).await?;
        Ok(())
    });
    sys.run();
    assert!(sys.is_finished(pid));
    assert_eq!(sys.now().raw(), 0.0);
}

/// Interrupting a process blocked in `acquire` must not leave it queued:
/// the next waiter should get the resource.
#[test]
fn interrupt_pending_acquire_unblocks_next_waiter() {
    let mut sys = ProcessSystem::new(5);
    sys.add_resource(0, 1);
    sys.spawn(|sim: Sim| async move {
        sim.acquire(0, 1).await?;
        sim.delay(Time::from_raw(100.0)).await?;
        sim.release(0, 1);
        Ok(())
    });
    let victim = sys.spawn(|sim: Sim| async move {
        // Recover from the interrupt.
        match sim.acquire(0, 1).await {
            Err(Interrupt) => Ok(()),
            Ok(()) => Err(Interrupt),
        }
    });
    let got = Rc::new(Cell::new(false));
    let g = got.clone();
    let next = sys.spawn(move |sim: Sim| async move {
        sim.acquire(0, 1).await?;
        g.set(true);
        sim.release(0, 1);
        Ok(())
    });
    sys.run_until(Time::from_raw(1.0));
    sys.interrupt(victim);
    sys.run_until(Time::from_raw(2.0));
    assert!(sys.is_finished(victim));
    // The first holder still holds the unit, so `next` waits for it. Once
    // the holder releases at t = 100 the waiter proceeds.
    assert!(!got.get());
    sys.run();
    assert!(got.get());
    assert!(sys.is_finished(next));
}

/// Interrupting one process blocked in `wait_until` must not drop the
/// condition for other processes waiting on the same predicate.
#[test]
fn interrupt_wait_until_keeps_condition_for_others() {
    let mut sys = ProcessSystem::new(6);
    let flag = Rc::new(Cell::new(false));
    let f = flag.clone();
    let victim = sys.spawn({
        let f = f.clone();
        move |sim: Sim| async move {
            match sim.wait_until(move |_| f.get()).await {
                Err(Interrupt) => Ok(()),
                Ok(()) => Err(Interrupt),
            }
        }
    });
    let other = sys.spawn(move |sim: Sim| async move {
        sim.wait_until(move |_| f.get()).await?;
        Ok(())
    });
    sys.run_until(Time::from_raw(0.5));
    sys.interrupt(victim);
    sys.run_until(Time::from_raw(0.5));
    assert!(sys.is_finished(victim));
    assert!(!sys.is_finished(other));

    flag.set(true);
    let handle = sys.handle();
    handle.notify_all();
    sys.run();
    assert!(sys.is_finished(other));
}

/// A process may interrupt itself; the pending delay then fails with
/// `Interrupt` and the body recovers.
#[test]
fn self_interrupt_is_delivered() {
    let mut sys = ProcessSystem::new(7);
    let pid_cell = Rc::new(Cell::new(None));
    let pc = pid_cell.clone();
    let pid = sys.spawn(move |sim: Sim| async move {
        let me = pc.get().expect("pid recorded before first resume");
        sim.interrupt(me);
        match sim.delay(Time::from_raw(10.0)).await {
            Err(Interrupt) => Ok(()),
            Ok(()) => Err(Interrupt),
        }
    });
    pid_cell.set(Some(pid));
    sys.run();
    assert!(sys.is_finished(pid));
    assert_eq!(sys.now().raw(), 0.0);
}

/// Releasing more than was taken must not create units beyond capacity.
#[test]
fn release_is_clamped_to_capacity() {
    let sys = ProcessSystem::new(8);
    sys.add_resource(0, 2);
    let handle = sys.handle();
    handle.release(0, 5);
    assert_eq!(handle.available(0), 2);
}

#[test]
#[should_panic(expected = "non-negative")]
fn negative_delay_is_rejected() {
    let mut sys = ProcessSystem::new(9);
    sys.spawn(|sim: Sim| async move {
        let _ = sim.delay(Time::from_raw(-1.0)).await;
        Ok(())
    });
    sys.run();
}

#[test]
#[should_panic(expected = "simulation time must be finite")]
fn nan_delay_is_rejected_at_construction() {
    let mut sys = ProcessSystem::new(9);
    sys.spawn(|sim: Sim| async move {
        let _ = sim.delay(Time::from_raw(f64::NAN)).await;
        Ok(())
    });
    sys.run();
}

#[test]
#[should_panic(expected = "exceeds capacity")]
fn acquire_beyond_capacity_is_rejected() {
    let mut sys = ProcessSystem::new(10);
    sys.add_resource(0, 1);
    sys.spawn(|sim: Sim| async move {
        let _ = sim.acquire(0, 2).await;
        Ok(())
    });
    sys.run();
}

#[test]
#[should_panic(expected = "outside a spawned process")]
fn blocking_op_outside_process_has_clear_message() {
    let sys = ProcessSystem::new(11);
    let handle = sys.handle();
    drop(handle.delay(Time::from_raw(1.0)));
}

#[test]
#[should_panic(expected = "simulation time must be finite")]
fn hybrid_step_rejects_nan_duration() {
    use tpt_des_process::hybrid::{ContinuousModel, HybridBridge};
    struct Flat;
    impl ContinuousModel for Flat {
        fn integrate(&mut self, _dt: Time<Seconds>) {}
        fn guard(&self) -> f64 {
            0.0
        }
    }
    let sys = ProcessSystem::new(12);
    let pid = sys.spawn(|_sim: Sim| async move { Ok(()) });
    let mut bridge = HybridBridge::new(pid, 1.0, Time::from_raw(0.1));
    bridge.attach(Flat);
    bridge.step(&sys, Time::from_raw(f64::NAN));
}

/// A process spawned at t = 0.5 must run at t = 0.5. The executor used to
/// fire the next timer (t = 1.0) before polling runnable processes, so the
/// spawned process started late, and any wait measured from its arrival was
/// understated (the bank-teller example reported zero waits).
#[test]
fn spawned_process_runs_before_clock_advances() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let starts = Rc::new(RefCell::new(Vec::new()));
    let mut sys = ProcessSystem::new(13);
    let s = starts.clone();
    sys.spawn(move |sim: Sim| async move {
        for _ in 0..3 {
            sim.delay(Time::from_raw(0.5)).await?;
            let s = s.clone();
            sim.spawn(move |sim: Sim| async move {
                s.borrow_mut().push(sim.now().raw());
                Ok(())
            });
        }
        Ok(())
    });
    sys.run();
    assert_eq!(*starts.borrow(), vec![0.5, 1.0, 1.5]);
}
