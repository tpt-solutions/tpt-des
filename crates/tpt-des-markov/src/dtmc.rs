//! Discrete-time Markov chains: validation, evolution, steady state.
//!
//! The chain itself is stored as a `tpt-math-prob-markov::MarkovChain`
//! (plain `Vec<Vec<f64>>` rows, simulation-ready), while the numerical
//! analysis (power iteration for the stationary distribution) runs on the
//! in-house [`DMatrix`].

use crate::error::MarkovError;
use alloc::vec::Vec;
use tpt_math_linalg_dense::{DMatrix, DVector};
use tpt_math_prob_markov::MarkovChain;

/// How far a row may drift from 1.0 before the matrix is rejected.
pub const ROW_TOLERANCE: f64 = 1e-9;

/// Validate that `rows` is a non-empty, square, non-negative, row-stochastic
/// matrix.
pub fn validate_row_stochastic(rows: &[Vec<f64>]) -> Result<(), MarkovError> {
    let n = rows.len();
    if n == 0 {
        return Err(MarkovError::Empty);
    }
    for row in rows {
        if row.len() != n {
            return Err(MarkovError::NotSquare);
        }
        let mut sum = 0.0;
        for &p in row {
            if p < 0.0 || !p.is_finite() {
                return Err(MarkovError::NegativeEntry);
            }
            sum += p;
        }
        if (sum - 1.0).abs() > ROW_TOLERANCE {
            return Err(MarkovError::NotRowStochastic);
        }
    }
    Ok(())
}

/// A discrete-time Markov chain with a validated row-stochastic matrix.
#[derive(Clone, Debug, PartialEq)]
pub struct Dtmc {
    chain: MarkovChain,
}

impl Dtmc {
    /// Construct from a row-stochastic matrix; every row is validated.
    pub fn from_rows(rows: Vec<Vec<f64>>) -> Result<Self, MarkovError> {
        validate_row_stochastic(&rows)?;
        let chain = MarkovChain::from_rows(rows).map_err(|_| MarkovError::NotRowStochastic)?;
        Ok(Self { chain })
    }

    /// Construct from arbitrary non-negative weights: each row is normalized
    /// to sum to exactly 1 (zero rows are rejected).
    pub fn from_weights(rows: Vec<Vec<f64>>) -> Result<Self, MarkovError> {
        let n = rows.len();
        if n == 0 {
            return Err(MarkovError::Empty);
        }
        let mut normalized = Vec::with_capacity(n);
        for mut row in rows {
            if row.len() != n {
                return Err(MarkovError::NotSquare);
            }
            if row.iter().any(|&w| w < 0.0 || !w.is_finite()) {
                return Err(MarkovError::NegativeEntry);
            }
            let sum: f64 = row.iter().sum();
            if sum <= 0.0 {
                return Err(MarkovError::NotRowStochastic);
            }
            for w in row.iter_mut() {
                *w /= sum;
            }
            // Absorb rounding residue in the last entry.
            let head: f64 = row[..n - 1].iter().sum();
            row[n - 1] = 1.0 - head;
            normalized.push(row);
        }
        Self::from_rows(normalized)
    }

    /// The number of states.
    pub fn len(&self) -> usize {
        self.chain.row(0).len()
    }

    /// True if the chain has no states (cannot be constructed, but kept for
    /// API symmetry).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// One-step transition probability `P[i][j]`.
    pub fn p(&self, from: usize, to: usize) -> f64 {
        self.chain.transition_prob(from, to)
    }

    /// The full transition matrix as rows.
    pub fn rows(&self) -> Vec<Vec<f64>> {
        (0..self.len())
            .map(|i| self.chain.row(i).to_vec())
            .collect()
    }

    /// The underlying `tpt-math-prob-markov` chain (for simulation).
    pub fn chain(&self) -> &MarkovChain {
        &self.chain
    }

    /// Evolve a distribution `n` steps: `v·Pⁿ`.
    pub fn distribution_after(&self, initial: &[f64], steps: usize) -> Vec<f64> {
        self.chain.distribution_after(initial, steps)
    }

    /// The transition matrix as an `n×n` [`DMatrix`].
    pub fn as_dmatrix(&self) -> DMatrix {
        let n = self.len();
        let mut data = Vec::with_capacity(n * n);
        for row in self.rows() {
            data.extend_from_slice(&row);
        }
        DMatrix::from_row_slice(n, n, &data)
    }

    /// Steady-state probabilities `π` with `πP = π` and `Σπ = 1`.
    ///
    /// Solved directly: the last balance equation is replaced by the
    /// normalization constraint and the system is solved. Unlike power
    /// iteration this converges for periodic chains, where iterates
    /// oscillate forever.
    ///
    /// * `tol` bounds the residual `‖πP − π‖₁`; if the solve misses it,
    ///   [`MarkovError::NotConvergent`] is returned.
    /// * `max_iter` is accepted for signature compatibility; the direct solve
    ///   does not iterate.
    ///
    /// A reducible chain has no unique stationary distribution and yields
    /// [`MarkovError::Singular`].
    ///
    /// # Examples
    ///
    /// The classic two-state chain:
    ///
    /// ```
    /// use tpt_des_markov::Dtmc;
    ///
    /// let chain = Dtmc::from_rows(vec![vec![0.9, 0.1], vec![0.5, 0.5]]).unwrap();
    /// let pi = chain.stationary(1e-12, 10_000).unwrap();
    /// assert!((pi[0] - 5.0 / 6.0).abs() < 1e-9);
    /// assert!((pi[0] + pi[1] - 1.0).abs() < 1e-12);
    /// ```
    pub fn stationary(&self, tol: f64, max_iter: usize) -> Result<Vec<f64>, MarkovError> {
        let _ = max_iter;
        if tol.is_nan() || tol < 0.0 {
            return Err(MarkovError::InvalidTolerance);
        }
        let n = self.len();
        let p = self.as_dmatrix();
        // (Pᵀ − I) π = 0, with the last equation replaced by Σπ = 1.
        let a = DMatrix::from_fn(n, n, |i, j| {
            if i == n - 1 {
                1.0
            } else {
                p[(j, i)] - if i == j { 1.0 } else { 0.0 }
            }
        });
        let b = DVector::from_fn(n, |i| if i == n - 1 { 1.0 } else { 0.0 });
        let solution = a.solve(&b).map_err(|_| MarkovError::Singular)?;
        let pi: Vec<f64> = (0..n).map(|i| solution[i]).collect();

        // The residual ‖πP − π‖₁ must be within tolerance; a non-finite or
        // clearly negative entry means no valid distribution was produced.
        let residual: f64 = (0..n)
            .map(|j| {
                let flow: f64 = (0..n).map(|i| pi[i] * p[(i, j)]).sum();
                (flow - pi[j]).abs()
            })
            .sum();
        if !residual.is_finite()
            || residual > tol
            || pi.iter().any(|&x| !x.is_finite() || x < -1e-9)
        {
            return Err(MarkovError::NotConvergent);
        }
        Ok(pi.iter().map(|&x| x.max(0.0)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn two_state_stationary_exact() {
        let chain = Dtmc::from_rows(vec![vec![0.9, 0.1], vec![0.5, 0.5]]).unwrap();
        let pi = chain.stationary(1e-14, 10_000).unwrap();
        assert!(close(pi[0], 5.0 / 6.0, 1e-10));
        assert!(close(pi[1], 1.0 / 6.0, 1e-10));
    }

    #[test]
    fn identity_chain_is_reducible_and_has_no_unique_stationary() {
        // Every state is absorbing, so any distribution is stationary and
        // none is unique: the direct solve reports Singular.
        let n = 5;
        let rows = (0..n)
            .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
            .collect();
        let chain = Dtmc::from_rows(rows).unwrap();
        assert_eq!(chain.stationary(1e-14, 10_000), Err(MarkovError::Singular));
    }

    #[test]
    fn from_weights_normalizes_rows() {
        let chain = Dtmc::from_weights(vec![vec![1.0, 3.0], vec![2.0, 2.0]]).unwrap();
        assert!(close(chain.p(0, 0), 0.25, 1e-12));
        assert!(close(chain.p(1, 0), 0.5, 1e-12));
    }

    #[test]
    fn rejects_invalid_matrices() {
        assert_eq!(
            Dtmc::from_rows(vec![vec![0.5, 0.4], vec![0.5, 0.5]]),
            Err(MarkovError::NotRowStochastic)
        );
        assert_eq!(
            Dtmc::from_rows(vec![vec![-0.1, 1.1], vec![0.5, 0.5]]),
            Err(MarkovError::NegativeEntry)
        );
        assert_eq!(
            Dtmc::from_rows(vec![vec![1.0, 0.0, 0.0], vec![0.5, 0.5, 0.0]]),
            Err(MarkovError::NotSquare)
        );
    }

    #[test]
    fn distribution_evolution_matches_matrix_power() {
        let chain = Dtmc::from_rows(vec![vec![0.5, 0.5], vec![0.2, 0.8]]).unwrap();
        let evolved = chain.distribution_after(&[1.0, 0.0], 3);
        // Manual: v1 = (0.5, 0.5); v2 = (0.35, 0.65); v3 = (0.305, 0.695).
        assert!(close(evolved[0], 0.305, 1e-12));
        assert!(close(evolved[1], 0.695, 1e-12));
    }
}
