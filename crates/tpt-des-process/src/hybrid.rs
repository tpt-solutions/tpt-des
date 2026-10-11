//! Hybrid discrete–continuous simulation hook.
//!
//! The spec's integration plan has `tpt-des-process` host the seam where a
//! continuous solver plugs in: discrete events interleave with continuous
//! integration steps, and a *guard crossing* in the continuous state raises
//! a process interrupt. The Phase 2 checkpoint in `todo.md` asked to
//! confirm `tpt-control`'s API surface first — that repo does not exist in
//! the workspace yet, so this module defines the local
//! [`ContinuousModel`] trait; when `tpt-control` lands, it implements this
//! trait and the bridge below needs no changes.
//!
//! # Examples
//!
//! A tank fills at a constant rate; when it crosses 80%, the filling
//! process is interrupted:
//!
//! ```
//! use tpt_des_core::{Seconds, Time};
//! use tpt_des_process::hybrid::{ContinuousModel, HybridBridge};
//! use tpt_des_process::{Interrupt, ProcessSystem};
//!
//! #[derive(Debug)]
//! struct Tank { level: f64, rate: f64 }
//! impl ContinuousModel for Tank {
//!     fn integrate(&mut self, dt: Time<Seconds>) { self.level += self.rate * dt.raw(); }
//!     fn guard(&self) -> f64 { self.level }
//! }
//!
//! let mut sys = ProcessSystem::new(1);
//! sys.add_resource(0, 1);
//! let filler = sys.spawn(|sim| async move {
//!     sim.acquire(0, 1).await?;               // hold the valve
//!     // Filling runs continuously; the guard interrupts us at 80%.
//!     sim.delay(Time::from_raw(100.0)).await.unwrap_err();     // never finishes the delay
//!     sim.release(0, 1);
//!     Ok(())
//! });
//!
//! let mut bridge = HybridBridge::new(filler, 0.80, Time::from_raw(1.0));
//! bridge.attach(Tank { level: 0.0, rate: 0.1 });
//! while !sys.is_finished(filler) && bridge.now().raw() < 100.0 {
//!     bridge.step(&sys, Time::from_raw(0.1));                  // integrate 0.1s per step
//!     sys.run_until(bridge.now());
//! }
//! // The guard fired near level 0.8, i.e. around t ≈ 8.
//! assert!((bridge.now().raw() - 8.0).abs() < 0.15);
//! ```

use crate::system::{ProcessId, ProcessSystem};
use tpt_des_core::{Seconds, Time};

/// A continuous model integrated between discrete events.
///
/// `tpt-control`-style solvers implement this trait; [`HybridBridge`] drives
/// it in fixed steps and watches the guard.
pub trait ContinuousModel: 'static {
    /// Advance the continuous state by `dt` time units.
    fn integrate(&mut self, dt: Time<Seconds>);

    /// The scalar guard value watched for threshold crossings.
    fn guard(&self) -> f64;
}

/// Drives a [`ContinuousModel`] in fixed steps and interrupts a process
/// when the guard crosses `threshold` (from either direction).
pub struct HybridBridge<M: ContinuousModel> {
    model: Option<M>,
    threshold: f64,
    step: Time<Seconds>,
    now: Time<Seconds>,
    last_guard: Option<f64>,
    target: ProcessId,
    fired: bool,
}

impl<M: ContinuousModel> core::fmt::Debug for HybridBridge<M> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HybridBridge")
            .field("threshold", &self.threshold)
            .field("step", &self.step)
            .field("now", &self.now)
            .field("fired", &self.fired)
            .finish()
    }
}

impl<M: ContinuousModel> HybridBridge<M> {
    /// A bridge interrupting `target` when the guard crosses `threshold`;
    /// continuous time advances in `step` increments.
    ///
    /// # Panics
    ///
    /// If `threshold` is NaN, or `step` is not finite and positive.
    pub fn new(target: ProcessId, threshold: f64, step: Time<Seconds>) -> Self {
        assert!(!threshold.is_nan(), "guard threshold must not be NaN");
        assert!(
            step.raw() > 0.0,
            "integration step must be finite and positive, got {}",
            step.raw()
        );
        Self {
            model: None,
            threshold,
            step,
            now: Time::ZERO,
            last_guard: None,
            target,
            fired: false,
        }
    }

    /// Attach the continuous model.
    pub fn attach(&mut self, model: M) {
        self.last_guard = Some(model.guard());
        self.model = Some(model);
    }

    /// Current continuous time.
    pub fn now(&self) -> Time<Seconds> {
        self.now
    }

    /// The current guard value (if a model is attached).
    pub fn guard(&self) -> Option<f64> {
        self.model.as_ref().map(|m| m.guard())
    }

    /// True once the crossing fired.
    pub fn fired(&self) -> bool {
        self.fired
    }

    /// Advance the model one step; interrupts the target process when the
    /// guard crosses the threshold. Returns the new continuous time.
    ///
    /// # Panics
    ///
    /// If `dt` is NaN, infinite, or negative: integrating over such a span
    /// would silently skip the step or never terminate.
    pub fn step(&mut self, system: &ProcessSystem, dt: Time<Seconds>) -> Time<Seconds> {
        assert!(
            dt.raw() >= 0.0,
            "step duration must be non-negative, got {}",
            dt.raw()
        );
        let Some(model) = self.model.as_mut() else {
            return self.now;
        };
        let mut remaining = dt.raw();
        while remaining > 0.0 {
            let h = remaining.min(self.step.raw());
            model.integrate(Time::from_raw(h));
            self.now += Time::from_raw(h);
            remaining -= h;
            let guard = model.guard();
            if let Some(last) = self.last_guard {
                let crossed = (last < self.threshold && guard >= self.threshold)
                    || (last > self.threshold && guard <= self.threshold);
                if crossed && !self.fired {
                    self.fired = true;
                    system.interrupt(self.target);
                    self.last_guard = Some(guard);
                    return self.now;
                }
            }
            self.last_guard = Some(guard);
        }
        self.now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::{Interrupt, ProcessSystem};

    #[derive(Debug)]
    struct Tank {
        level: f64,
        rate: f64,
    }

    impl ContinuousModel for Tank {
        fn integrate(&mut self, dt: Time<Seconds>) {
            self.level += self.rate * dt.raw();
        }
        fn guard(&self) -> f64 {
            self.level
        }
    }

    #[test]
    fn guard_crossing_interrupts_process() {
        let mut sys = ProcessSystem::new(1);
        sys.add_resource(0, 1);
        let filler = sys.spawn(|sim| async move {
            sim.acquire(0, 1).await?;
            // Will be interrupted long before t = 100.
            assert!(sim.delay(Time::from_raw(100.0)).await.is_err());
            sim.release(0, 1);
            Ok(())
        });

        let mut bridge = HybridBridge::new(filler, 0.80, Time::from_raw(1.0));
        bridge.attach(Tank {
            level: 0.0,
            rate: 0.1,
        });
        let mut steps = 0;
        while !sys.is_finished(filler) && bridge.now().raw() < 100.0 && steps < 10_000 {
            bridge.step(&sys, Time::from_raw(0.1));
            sys.set_time(bridge.now());
            sys.run_until(bridge.now());
            steps += 1;
        }
        assert!(bridge.fired());
        assert!((bridge.now().raw() - 8.0).abs() < 0.15);
        assert!(sys.is_finished(filler));
        assert_eq!(sys.now(), bridge.now());
    }

    #[test]
    fn interruption_delivers_through_pending_delay() {
        let mut sys = ProcessSystem::new(3);
        let pid = sys.spawn(|sim| async move {
            match sim.delay(Time::from_raw(10.0)).await {
                Err(Interrupt) => Ok(()), // recovered
                Ok(()) => Err(Interrupt), // would mean no interruption happened
            }
        });
        // The delay is pending after the first poll; interrupt it.
        sys.run_until(Time::from_raw(1.0));
        sys.interrupt(pid);
        sys.run();
        assert!(sys.is_finished(pid));
        assert!(sys.now().raw() < 10.0);
    }
}
