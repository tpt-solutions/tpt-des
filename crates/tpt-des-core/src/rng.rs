//! Deterministic, seeded simulation RNG.
//!
//! Every simulation draws its randomness from a single [`SimRng`] — a
//! `SplitMix64` from `tpt-math-prob-core` — created from an explicit seed.
//! Re-running the same model with the same seed reproduces the same event
//! trace bit-for-bit (see the replay test at the bottom of this file), and
//! [`SimRng::restart`] rewinds to the initial seed mid-flight.
//!
//! The RNG implements [`Rng`](tpt_math_prob_core::Rng), so every
//! `tpt-math-prob-dist` / `tpt-math-prob-sampler` distribution can sample
//! from it directly.

use core::fmt;
use tpt_math_prob_core::{Rng, SplitMix64};

/// The seeded RNG driving one simulation.
pub struct SimRng {
    seed: u64,
    inner: SplitMix64,
    draws: u64,
}

impl fmt::Debug for SimRng {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimRng")
            .field("seed", &self.seed)
            .field("draws", &self.draws)
            .finish()
    }
}

impl SimRng {
    /// Create the RNG for a simulation with the given seed.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            inner: SplitMix64::seed_from_u64(seed),
            draws: 0,
        }
    }

    /// The seed this RNG was created from.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Number of draws consumed so far — a cheap determinism fingerprint.
    pub fn draws(&self) -> u64 {
        self.draws
    }

    /// Rewind to the initial seed, producing the exact same stream again.
    pub fn restart(&mut self) {
        self.inner = SplitMix64::seed_from_u64(self.seed);
        self.draws = 0;
    }

    /// Access the underlying `SplitMix64` (e.g. to hand to a
    /// `tpt-math-prob-dist` distribution's `sample`).
    pub fn core_mut(&mut self) -> &mut SplitMix64 {
        &mut self.inner
    }
}

impl Rng for SimRng {
    fn next_u64(&mut self) -> u64 {
        self.draws += 1;
        self.inner.next_u64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_streams_are_identical() {
        let mut a = SimRng::new(7);
        let mut b = SimRng::new(7);
        let xs: Vec<f64> = (0..64).map(|_| a.next_f64()).collect();
        let ys: Vec<f64> = (0..64).map(|_| b.next_f64()).collect();
        assert_eq!(xs, ys);
        assert_eq!(a.draws(), b.draws());

        // Different seeds diverge.
        let mut c = SimRng::new(8);
        let zs: Vec<f64> = (0..64).map(|_| c.next_f64()).collect();
        assert_ne!(xs, zs);
    }

    #[test]
    fn restart_rewinds() {
        let mut rng = SimRng::new(123);
        let first: Vec<u64> = (0..16).map(|_| rng.next_u64()).collect();
        rng.restart();
        assert_eq!(rng.draws(), 0);
        let second: Vec<u64> = (0..16).map(|_| rng.next_u64()).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn uniforms_in_range() {
        let mut rng = SimRng::new(1);
        for _ in 0..10_000 {
            let u = rng.next_f64();
            assert!((0.0..1.0).contains(&u));
        }
    }
}
