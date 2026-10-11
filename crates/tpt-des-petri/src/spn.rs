//! Stochastic Petri nets: Gillespie simulation and CTMC conversion.
//!
//! Each transition fires after an exponentially distributed delay at its
//! own rate; at any marking the next transition to fire is chosen by
//! competing exponentials — total rate `λ = Σ enabled rates`, waiting time
//! `Exp(λ)` (via `tpt-math-prob-dist`), transition picked with probability
//! proportional to its rate (via `tpt-math-prob-sampler`). This is exactly
//! Gillespie's direct method, and it makes the SPN a continuous-time Markov
//! chain over reachable markings — which [`StochasticPetriNet::to_ctmc`]
//! exploits for exact steady-state analysis.

use crate::net::{Marking, PetriNet};
use crate::reach::{reachability, ReachabilityError};
use alloc::vec::Vec;
use std::collections::BTreeMap;
use tpt_des_markov::Ctmc;
use tpt_des_markov::MarkovError as CtmcError;
use tpt_math_prob_core::{Distribution as _, SplitMix64};
use tpt_math_prob_dist::{exp, Dist};
use tpt_math_prob_sampler::sample_categorical;

/// A place/transition net with exponentially distributed firing times.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StochasticPetriNet {
    net: PetriNet,
    rates: Vec<f64>,
}

impl StochasticPetriNet {
    /// Wrap a net; every transition starts at rate 0 (disabled) until a
    /// positive rate is set.
    pub fn new(net: PetriNet) -> Self {
        let rates = vec![0.0; net.transitions()];
        Self { net, rates }
    }

    /// Set the firing rate of transition `t` (must be finite and ≥ 0).
    pub fn set_rate(&mut self, t: usize, rate: f64) -> Result<(), SpnError> {
        if rate < 0.0 || !rate.is_finite() || t >= self.rates.len() {
            return Err(SpnError::InvalidRate);
        }
        self.rates[t] = rate;
        Ok(())
    }

    /// The underlying net.
    pub fn net(&self) -> &PetriNet {
        &self.net
    }

    /// Firing rate of transition `t`.
    pub fn rate(&self, t: usize) -> f64 {
        self.rates[t]
    }

    /// Simulate from `initial` until `horizon` (Gillespie direct method).
    ///
    /// Returns the firing counts, the final marking, and the *occupancy* —
    /// total time spent in each visited marking, the empirical estimate of
    /// the stationary distribution.
    ///
    /// Deterministic under `seed`.
    pub fn simulate(&self, initial: &Marking, horizon: f64, seed: u64) -> Result<SpnRun, SpnError> {
        if initial.0.len() != self.net.places() {
            return Err(SpnError::InvalidInitialMarking);
        }
        if !horizon.is_finite() || horizon < 0.0 {
            return Err(SpnError::InvalidHorizon);
        }
        let mut rng = SplitMix64::seed_from_u64(seed);
        let mut marking = initial.clone();
        let mut time = 0.0;
        let mut occupancy: BTreeMap<Marking, f64> = BTreeMap::new();
        let mut firing_counts = vec![0u64; self.net.transitions()];
        let mut total_firings = 0u64;

        // Accumulate occupancy time per distinct marking.
        // O(log k) per firing instead of a linear scan over visited markings.
        let occupy = |occupancy: &mut BTreeMap<Marking, f64>, m: &Marking, dt: f64| {
            if let Some(total) = occupancy.get_mut(m) {
                *total += dt;
            } else {
                occupancy.insert(m.clone(), dt);
            }
        };

        while time < horizon {
            let enabled = self.net.enabled(&marking);
            let rates: Vec<f64> = enabled.iter().map(|&t| self.rates[t]).collect();
            let total: f64 = rates.iter().sum();
            if total <= 0.0 {
                // Dead marking: absorb the remaining time and stop.
                occupy(&mut occupancy, &marking, horizon - time);
                break;
            }
            let remaining = horizon - time;
            let dt = Dist::new(exp(total).expect("total > 0"))
                .sample(&mut rng)
                .min(remaining);
            occupy(&mut occupancy, &marking, dt);
            time += dt;
            if time >= horizon {
                break;
            }
            let choice =
                sample_categorical(&rates, &mut rng).ok_or(SpnError::NoEnabledTransition)?;
            let fired = enabled[choice];
            marking = self.net.fire(fired, &marking).expect("enabled transition");
            firing_counts[fired] += 1;
            total_firings += 1;
        }

        Ok(SpnRun {
            final_marking: marking,
            firing_counts,
            total_firings,
            occupancy: occupancy.into_iter().collect(),
            horizon,
        })
    }

    /// Convert the bounded SPN reachable from `initial` into a CTMC whose
    /// state `i` is `graph.states[i]`; edge rates are the firing rates.
    ///
    /// Returns the CTMC plus the marking each state corresponds to, so
    /// stationary probabilities can be read per marking.
    pub fn to_ctmc(
        &self,
        initial: &Marking,
        max_states: usize,
    ) -> Result<(Ctmc, Vec<Marking>), SpnError> {
        let graph =
            reachability(&self.net, initial, max_states).map_err(SpnError::from_reachability)?;
        let n = graph.len();
        // The generator is stored densely (n² f64), so bound n before
        // allocating: 4096 states is 128 MiB.
        if n > MAX_CTMC_STATES {
            return Err(SpnError::TooManyStates);
        }
        let mut rows = vec![vec![0.0f64; n]; n];
        for (i, edges) in graph.edges.iter().enumerate() {
            for &(t, j) in edges {
                let rate = self.rates[t];
                if rate > 0.0 {
                    rows[i][j] += rate;
                }
            }
        }
        // Diagonals are recomputed by Ctmc::new; validate zero-rate rows:
        // a state with no outgoing rate is absorbing (row all zeros) — fine.
        for (i, row) in rows.iter_mut().enumerate() {
            let off: f64 = row
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, v)| v)
                .sum();
            row[i] = -off;
        }
        let ctmc = Ctmc::new(&rows, 1e-9).map_err(SpnError::from_ctmc)?;
        Ok((ctmc, graph.states))
    }
}

/// Failures of SPN simulation and conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpnError {
    /// A firing rate was negative or non-finite.
    InvalidRate,
    /// The initial marking does not match the net.
    InvalidInitialMarking,
    /// Reachability exploration exceeded the state bound.
    TooManyStates,
    /// CTMC construction from the reachability graph failed.
    CtmcConversion,
    /// Internal: a categorical draw found no positive rate.
    NoEnabledTransition,
    /// The horizon is NaN, infinite, or negative. An infinite horizon on a
    /// live net would never end.
    InvalidHorizon,
}

impl SpnError {
    fn from_reachability(e: ReachabilityError) -> Self {
        match e {
            ReachabilityError::TooManyStates => SpnError::TooManyStates,
            ReachabilityError::InvalidInitialMarking => SpnError::InvalidInitialMarking,
        }
    }

    fn from_ctmc(_e: CtmcError) -> Self {
        SpnError::CtmcConversion
    }
}

/// Largest state space [`StochasticPetriNet::to_ctmc`] will densify.
pub const MAX_CTMC_STATES: usize = 4096;

impl core::fmt::Display for SpnError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SpnError::InvalidRate => write!(f, "firing rate must be finite and non-negative"),
            SpnError::InvalidInitialMarking => {
                write!(f, "initial marking does not match the net's place count")
            }
            SpnError::TooManyStates => write!(f, "state space exceeds the configured bound"),
            SpnError::CtmcConversion => write!(f, "could not build the CTMC generator"),
            SpnError::NoEnabledTransition => write!(f, "no enabled transition has a positive rate"),
            SpnError::InvalidHorizon => write!(f, "horizon must be finite and non-negative"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SpnError {}

/// One simulation run's measurements.
#[derive(Clone, Debug, PartialEq)]
pub struct SpnRun {
    /// Marking when the horizon was reached.
    pub final_marking: Marking,
    /// Firings per transition.
    pub firing_counts: Vec<u64>,
    /// Total firings.
    pub total_firings: u64,
    /// Time spent per distinct marking — the empirical stationary
    /// distribution when the horizon is long.
    pub occupancy: Vec<(Marking, f64)>,
    /// The simulated horizon.
    pub horizon: f64,
}

impl SpnRun {
    /// Occupancy fraction of `marking` (time in marking / horizon).
    pub fn occupancy_fraction(&self, marking: &Marking) -> f64 {
        self.occupancy
            .iter()
            .filter(|(m, _)| m == marking)
            .map(|(_, t)| t)
            .sum::<f64>()
            / self.horizon
    }

    /// Mean firing rate of transition `t` (firings / horizon).
    pub fn firing_rate(&self, t: usize) -> f64 {
        self.firing_counts[t] as f64 / self.horizon
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::PetriNet;
    use alloc::vec;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// Pure Poisson process: one place, one unconditioned transition.
    fn poisson_net(rate: f64) -> StochasticPetriNet {
        let mut net = PetriNet::new();
        let p = net.add_place("count");
        let fire = net.add_transition("arrive");
        net.add_arc_tp(fire, p, 1);
        let mut spn = StochasticPetriNet::new(net);
        spn.set_rate(fire, rate).unwrap();
        spn
    }

    #[test]
    fn poisson_firing_rate_matches_lambda() {
        let spn = poisson_net(5.0);
        let run = spn.simulate(&Marking(vec![0]), 10_000.0, 42).unwrap();
        let measured = run.firing_rate(0);
        assert!(close(measured, 5.0, 0.25), "rate {measured} vs 5.0");
    }

    #[test]
    fn deterministic_seed_replays() {
        let spn = poisson_net(3.0);
        let a = spn.simulate(&Marking(vec![0]), 1_000.0, 7).unwrap();
        let b = spn.simulate(&Marking(vec![0]), 1_000.0, 7).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn dead_marking_absorbs() {
        // p0 →t→ p1, no way back: when p0 empties, time runs out in the sink.
        let mut net = PetriNet::new();
        let p0 = net.add_place("p0");
        let p1 = net.add_place("p1");
        let t = net.add_transition("t");
        net.add_arc_pt(p0, t, 1);
        net.add_arc_tp(t, p1, 1);
        let mut spn = StochasticPetriNet::new(net);
        spn.set_rate(t, 1.0).unwrap();
        let run = spn.simulate(&Marking(vec![1, 0]), 100.0, 1).unwrap();
        assert_eq!(run.final_marking, Marking(vec![0, 1]));
        assert_eq!(run.total_firings, 1);
        // The rest of the horizon is absorbed in the sink marking.
        assert!(run.occupancy_fraction(&Marking(vec![0, 1])) > 0.9);
    }

    /// M/M/1/K: bounded queue (capacity place "slots" with K tokens).
    fn mm1k(k: u32, lambda: f64, mu: f64) -> StochasticPetriNet {
        let mut net = PetriNet::new();
        let q = net.add_place("queue");
        let slots = net.add_place("slots");
        let arrive = net.add_transition("arrive");
        let serve = net.add_transition("serve");
        net.add_arc_pt(slots, arrive, 1);
        net.add_arc_tp(arrive, q, 1);
        net.add_arc_pt(q, serve, 1);
        net.add_arc_tp(serve, slots, 1);
        let mut spn = StochasticPetriNet::new(net);
        spn.set_rate(arrive, lambda).unwrap();
        spn.set_rate(serve, mu).unwrap();
        spn.net()
            .fire(0, &Marking(vec![0, k]))
            .unwrap_or_else(|| Marking(vec![0, k]));
        spn
    }

    #[test]
    fn ctmc_conversion_birth_death_stationary() {
        // Bounded M/M/1/3: λ=1, μ=2 → ρ=0.5; stationary is truncated
        // geometric over queue length 0..=3.
        let spn = mm1k(3, 1.0, 2.0);
        let (ctmc, states) = spn.to_ctmc(&Marking(vec![0, 3]), 64).unwrap();
        assert_eq!(states.len(), 4); // queue = 0, 1, 2, 3
        let pi = ctmc.stationary().unwrap();
        let n = states.len();
        let rho: f64 = 0.5;
        let z: f64 = (0..n).map(|k| rho.powi(k as i32)).sum();
        for (k, m) in states.iter().enumerate() {
            let queue = m.0[0] as usize;
            assert_eq!(queue, k, "states enumerated by queue length");
            assert!(
                close(pi[k], rho.powi(k as i32) / z, 1e-9),
                "pi[{k}] = {} vs {}",
                pi[k],
                rho.powi(k as i32) / z
            );
        }
    }

    #[test]
    fn occupancy_tracks_ctmc_stationary() {
        // Long-run occupancy fraction of queue=0 should approach π_0 = 0.5.
        let mut net = PetriNet::new();
        let q = net.add_place("queue");
        let arrive = net.add_transition("arrive");
        let serve = net.add_transition("serve");
        net.add_arc_tp(arrive, q, 1);
        net.add_arc_pt(q, serve, 1);
        let mut spn = StochasticPetriNet::new(net);
        spn.set_rate(arrive, 1.0).unwrap();
        spn.set_rate(serve, 2.0).unwrap();
        let run = spn.simulate(&Marking(vec![0]), 50_000.0, 9).unwrap();
        let frac0 = run.occupancy_fraction(&Marking(vec![0]));
        assert!(close(frac0, 0.5, 0.05), "occupancy of empty queue: {frac0}");
    }
}
