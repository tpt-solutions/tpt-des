//! Hybrid discrete–continuous simulation: a filling tank.
//!
//! A tank fills continuously at a constant rate. A discrete process holds the
//! inlet valve and sleeps for a long, fixed fill time. The `HybridBridge`
//! integrates the tank level in small steps and interrupts the process the
//! moment the level crosses 80%, so the valve is released by the continuous
//! model rather than by the clock.
//!
//! Run with `cargo run -p tpt-des --example hybrid_bridge --features "std process"`.

use tpt_des::kernel::{Seconds, Time};
use tpt_des::process::hybrid::{ContinuousModel, HybridBridge};
use tpt_des::process::{ProcessSystem, Sim};

const RATE: f64 = 0.1; // level units per second
const THRESHOLD: f64 = 0.8;
const STEP: f64 = 0.05; // integration step in seconds

#[derive(Debug)]
struct Tank {
    level: f64,
}

impl ContinuousModel for Tank {
    fn integrate(&mut self, dt: Time<Seconds>) {
        self.level += RATE * dt.raw();
    }

    fn guard(&self) -> f64 {
        self.level
    }
}

fn main() {
    let mut system = ProcessSystem::new(1);
    system.add_resource(0, 1); // the inlet valve

    let filler = system.spawn(|sim: Sim| async move {
        sim.acquire(0, 1).await?;
        // The delay is far longer than the fill time; the guard interrupts it.
        let outcome = sim.delay(Time::from_raw(1_000.0)).await;
        sim.release(0, 1);
        println!(
            "valve released at t = {:.2} s (interrupted: {})",
            sim.now(),
            outcome.is_err()
        );
        Ok(())
    });

    let mut bridge = HybridBridge::new(filler, THRESHOLD, Time::from_raw(STEP));
    bridge.attach(Tank { level: 0.0 });

    while !system.is_finished(filler) && bridge.now().raw() < 1_000.0 {
        bridge.step(&system, Time::from_raw(STEP));
        system.set_time(bridge.now());
        system.run_until(bridge.now());
    }

    let expected = THRESHOLD / RATE;
    println!("expected crossing at t = {expected:.2} s");
    assert!(bridge.fired(), "the guard should have fired");
    assert!(
        (bridge.now().raw() - expected).abs() < 2.0 * STEP,
        "crossing time off by more than two steps"
    );
}
