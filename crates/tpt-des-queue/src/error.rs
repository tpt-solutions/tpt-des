//! Error types for the queuing crate.

use core::fmt;

/// Errors returned by the analytical queuing formulas and the network
/// solvers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueError {
    /// A rate, load, or moment argument was negative, zero, or non-finite.
    InvalidParameter,
    /// Arrival rate at or above service capacity: the queue is unstable and
    /// no steady state exists.
    Unstable,
    /// A routing matrix was malformed (wrong shape, negative entry, or a row
    /// summing above one).
    InvalidRoutingMatrix,
    /// The linear solve for node arrival rates failed to converge to a
    /// finite solution (e.g. the routing matrix has spectral radius ≥ 1).
    NoTrafficSolution,
    /// The population lattice of a multi-class MVA exceeds the state bound
    /// (the product of `N_c + 1` over classes).
    StateSpaceTooLarge,
}

impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueueError::InvalidParameter => {
                write!(f, "invalid parameter (negative, zero, or non-finite)")
            }
            QueueError::Unstable => write!(
                f,
                "arrival rate at or above service capacity; no steady state"
            ),
            QueueError::InvalidRoutingMatrix => {
                write!(f, "routing matrix is malformed or sub-row-stochastic")
            }
            QueueError::NoTrafficSolution => write!(f, "traffic equations have no finite solution"),
            QueueError::StateSpaceTooLarge => {
                write!(f, "multi-class population lattice exceeds the state bound")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for QueueError {}
