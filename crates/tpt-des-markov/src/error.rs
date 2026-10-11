//! Error types for the Markov crate.

use core::fmt;

/// Errors returned by Markov chain construction and analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkovError {
    /// The matrix is not square.
    NotSquare,
    /// A matrix has a negative entry where probabilities are required.
    NegativeEntry,
    /// A transition-matrix row does not sum to 1 within the tolerance.
    NotRowStochastic,
    /// A generator-matrix row does not sum to 0 within the tolerance, or the
    /// diagonal is not non-positive.
    InvalidGenerator,
    /// Zero states is not a meaningful chain.
    Empty,
    /// The chain has no absorbing states, or the requested start state can
    /// never reach an absorbing state.
    NoAbsorption,
    /// A linear solve or power iteration failed to converge.
    NotConvergent,
    /// The linear system has no unique solution: the chain is reducible
    /// (for stationary distributions) or the target set is not reached with
    /// probability one (for absorption and first-passage times).
    Singular,
    /// The tolerance is NaN or negative.
    InvalidTolerance,
}

impl fmt::Display for MarkovError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarkovError::NotSquare => write!(f, "matrix is not square"),
            MarkovError::NegativeEntry => write!(f, "negative probability entry"),
            MarkovError::NotRowStochastic => write!(f, "matrix rows do not sum to 1"),
            MarkovError::InvalidGenerator => write!(f, "generator rows must sum to 0"),
            MarkovError::Empty => write!(f, "chain must have at least one state"),
            MarkovError::NoAbsorption => write!(f, "no reachable absorbing state"),
            MarkovError::NotConvergent => write!(f, "solver did not converge"),
            MarkovError::Singular => write!(f, "linear system is singular (no unique solution)"),
            MarkovError::InvalidTolerance => write!(f, "tolerance must be a non-negative number"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MarkovError {}
