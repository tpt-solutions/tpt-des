//! Continuous-time Markov chains: generator matrices, stationary
//! distribution, transient probabilities.
//!
//! The generator `Q` is validated (square, finite, non-negative
//! off-diagonal, rows summing to 0). Transient probabilities
//! `P(t) = exp(Qt)` use a from-scratch scaling-and-squaring Taylor
//! exponential — no external solver, per the zero-external-dependency
//! principle.

use crate::error::MarkovError;
use alloc::vec::Vec;
use tpt_math_linalg_dense::{DMatrix, DVector};

/// A continuous-time Markov chain with generator matrix `Q`.
#[derive(Clone, Debug, PartialEq)]
pub struct Ctmc {
    q: DMatrix,
}

impl Ctmc {
    /// Construct and validate a generator matrix: square, finite,
    /// non-negative off-diagonal entries, rows summing to 0 within
    /// tolerance. The diagonal is recomputed exactly as `−rowsum`, so tiny
    /// row-sum residues cannot accumulate.
    pub fn new(rows: &[Vec<f64>], tolerance: f64) -> Result<Self, MarkovError> {
        let n = rows.len();
        if n == 0 {
            return Err(MarkovError::Empty);
        }
        for row in rows {
            if row.len() != n {
                return Err(MarkovError::NotSquare);
            }
            for &q_ij in row {
                if !q_ij.is_finite() {
                    return Err(MarkovError::InvalidGenerator);
                }
            }
            if row_sum(row).abs() > tolerance {
                return Err(MarkovError::InvalidGenerator);
            }
        }
        for (i, row) in rows.iter().enumerate() {
            for (j, &q_ij) in row.iter().enumerate() {
                if i != j && q_ij < 0.0 {
                    return Err(MarkovError::NegativeEntry);
                }
            }
        }
        let matrix = DMatrix::from_fn(n, n, |i, j| {
            if i == j {
                // Exact diagonal: rows sum to 0 up to rounding only.
                -rows[i]
                    .iter()
                    .enumerate()
                    .filter_map(|(k, &v)| (k != i).then_some(v))
                    .sum::<f64>()
            } else {
                rows[i][j]
            }
        });
        Ok(Self { q: matrix })
    }

    /// The number of states.
    pub fn len(&self) -> usize {
        self.q.nrows()
    }

    /// True if the chain has no states.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The generator matrix.
    pub fn q(&self) -> &DMatrix {
        &self.q
    }

    /// Generator rows as plain vectors (for inspection and tests).
    pub fn rows(&self) -> Vec<Vec<f64>> {
        let n = self.len();
        (0..n)
            .map(|i| (0..n).map(|j| self.q[(i, j)]).collect())
            .collect()
    }

    /// Stationary distribution by solving the augmented system
    /// `(Qᵀ | 1)πᵀ = (0 | 1)`: the last equation is replaced with Σπ = 1.
    pub fn stationary(&self) -> Result<Vec<f64>, MarkovError> {
        let n = self.len();
        let qt = self.q.transpose();
        let a = DMatrix::from_fn(n, n, |i, j| if i == n - 1 { 1.0 } else { qt[(i, j)] });
        let b = DVector::from_fn(n, |i| if i == n - 1 { 1.0 } else { 0.0 });
        let solution = a.solve(&b).map_err(|_| MarkovError::NotConvergent)?;
        let pi: Vec<f64> = (0..n).map(|i| solution[i]).collect();
        let total: f64 = pi.iter().sum();
        if !total.is_finite() || (total - 1.0).abs() > 1e-6 || pi.iter().any(|&p| p < -1e-9) {
            return Err(MarkovError::NotConvergent);
        }
        Ok(pi)
    }

    /// Transient state probabilities `π(t) = π(0)·exp(Qt)` via the
    /// scaling-and-squaring matrix exponential.
    pub fn transient(&self, initial: &[f64], t: f64) -> Result<Vec<f64>, MarkovError> {
        let n = self.len();
        if initial.len() != n {
            return Err(MarkovError::NotSquare);
        }
        // Row-vector convention: pi(t) = pi(0)^T . exp(Qt), i.e. the result
        // is exp(Qt)^T applied to the initial column.
        let p = matrix_exponential(&(self.q.clone() * t));
        let v = DVector::from_vec(initial.to_vec());
        let result = p.transpose() * v;
        Ok((0..n).map(|i| result[i]).collect())
    }
}

fn row_sum(row: &[f64]) -> f64 {
    row.iter().sum()
}

/// Matrix exponential `exp(A)` by scaling and squaring with a 24-term
/// Taylor series (near machine precision for generator-matrix norms).
pub fn matrix_exponential(a: &DMatrix) -> DMatrix {
    let n = a.nrows();
    let norm = a.iter().fold(0.0f64, |m, &x| m.max(x.abs()));
    let mut squarings = 0u32;
    let mut scale = 1.0f64;
    while norm / scale > 0.25 {
        scale *= 2.0;
        squarings += 1;
    }
    let small = a.clone() * (1.0 / scale);

    // Taylor series: Σ Aᵏ/k!
    let one = |v: f64| v;
    let _ = one;
    let identity = DMatrix::from_diagonal(&DVector::from_fn(n, |_| 1.0));
    let mut sum = identity.clone();
    let mut term = identity;
    for k in 1..=24 {
        term = term * small.clone() * (1.0 / k as f64);
        sum = sum + term.clone();
    }
    for _ in 0..squarings {
        sum = sum.clone() * sum;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn two_state_symmetric_stationary() {
        let ctmc = Ctmc::new(&[vec![-1.0, 1.0], vec![1.0, -1.0]], 1e-9).unwrap();
        let pi = ctmc.stationary().unwrap();
        assert!(close(pi[0], 0.5, 1e-9));
        assert!(close(pi[1], 0.5, 1e-9));
    }

    #[test]
    fn birth_death_stationary_is_geometric() {
        // M/M/1-style chain on {0,1,2,3} with λ=1, μ=2 → ρ=0.5.
        let n = 4;
        let (lambda, mu) = (1.0, 2.0);
        let mut rows = Vec::new();
        for i in 0..n {
            let mut row = vec![0.0; n];
            if i + 1 < n {
                row[i + 1] = lambda;
            }
            if i > 0 {
                row[i - 1] = mu;
            }
            let rate_out = if i + 1 < n { lambda } else { 0.0 } + if i > 0 { mu } else { 0.0 };
            row[i] = -rate_out;
            rows.push(row);
        }
        let ctmc = Ctmc::new(&rows, 1e-9).unwrap();
        let pi = ctmc.stationary().unwrap();
        // Truncated geometric on 0..4: π_i ∝ ρⁱ.
        let rho = lambda / mu;
        let z: f64 = (0..n).map(|i| rho.powi(i as i32)).sum();
        for (i, &p_i) in pi.iter().enumerate() {
            assert!(close(p_i, rho.powi(i as i32) / z, 1e-9), "pi[{i}]={p_i}");
        }
    }

    #[test]
    fn expm_at_zero_is_identity() {
        let q = DMatrix::from_row_slice(2, 2, &[-1.0, 1.0, 2.0, -2.0]);
        let e = matrix_exponential(&(q * 0.0));
        assert!(close(e[(0, 0)], 1.0, 1e-14));
        assert!(close(e[(1, 1)], 1.0, 1e-14));
        assert!(close(e[(0, 1)], 0.0, 1e-14));
    }

    #[test]
    fn transient_probabilities_sum_to_one_and_converge() {
        let ctmc = Ctmc::new(&[vec![-2.0, 2.0], vec![3.0, -3.0]], 1e-9).unwrap();
        let p_small = ctmc.transient(&[1.0, 0.0], 0.01).unwrap();
        let p_big = ctmc.transient(&[1.0, 0.0], 50.0).unwrap();
        assert!(close(p_small.iter().sum(), 1.0, 1e-10));
        // At t→∞ the distribution tends to the stationary one.
        let pi = ctmc.stationary().unwrap();
        assert!(close(p_big[0], pi[0], 1e-6));
    }

    #[test]
    fn expm_two_state_closed_form() {
        // exp(Qt) for Q=[[-a,a],[a,-a]] is [[.5+.5e^{-2at}, .5-.5e^{-2at}], …].
        let (a, t) = (2.0, 0.3);
        let ctmc = Ctmc::new(&[vec![-a, a], vec![a, -a]], 1e-9).unwrap();
        let p = ctmc.transient(&[1.0, 0.0], t).unwrap();
        let e = (-2.0 * a * t).exp();
        assert!(close(p[0], 0.5 + 0.5 * e, 1e-12));
        assert!(close(p[1], 0.5 - 0.5 * e, 1e-12));
    }

    #[test]
    fn rejects_bad_generators() {
        // Row sums not zero.
        assert_eq!(
            Ctmc::new(&[vec![-1.0, 2.0], vec![1.0, -1.0]], 1e-9),
            Err(MarkovError::InvalidGenerator)
        );
        // Negative off-diagonal.
        assert_eq!(
            Ctmc::new(&[vec![-1.0, 1.0], vec![-1.0, 1.0]], 1e-9),
            Err(MarkovError::NegativeEntry)
        );
        // Not square.
        assert_eq!(
            Ctmc::new(&[vec![-1.0, 1.0]], 1e-9),
            Err(MarkovError::NotSquare)
        );
    }
}
