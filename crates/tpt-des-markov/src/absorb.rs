//! Absorbing Markov chains: fundamental matrix, time to absorption,
//! absorption probabilities.
//!
//! The analysis puts the chain in canonical form (transient states first),
//! computes the fundamental matrix `N = (I−Q)⁻¹` with the in-house dense
//! inverse, and derives `t = N·1` (expected steps to absorption) and
//! `B = N·R` (probability of ending in each absorbing state).

use crate::dtmc::Dtmc;
use crate::error::MarkovError;
use alloc::vec::Vec;
use tpt_math_linalg_dense::{DMatrix, DVector};

/// Results of the absorbing-chain analysis, indexed in the order reported
/// by [`AbsorbingAnalysis::transient_states`] and
/// [`AbsorbingAnalysis::absorbing_states`].
#[derive(Clone, Debug, PartialEq)]
pub struct AbsorbingAnalysis {
    /// Expected number of steps until absorption, per transient state.
    pub expected_time_to_absorption: Vec<f64>,
    /// Probability of ending in each absorbing state, per transient state.
    pub absorption_probabilities: Vec<Vec<f64>>,
    /// The transient (non-absorbing) state indices in original numbering.
    pub transient_states: Vec<usize>,
    /// The absorbing state indices in original numbering.
    pub absorbing_states: Vec<usize>,
}

/// True if state `i` is absorbing (its row is the unit vector on `i`).
pub fn is_absorbing(dtmc: &Dtmc, i: usize) -> bool {
    dtmc.p(i, i) == 1.0
}

/// Analyze an absorbing chain.
///
/// Errors with [`MarkovError::NoAbsorption`] if the chain has no absorbing
/// states.
pub fn analyze(dtmc: &Dtmc) -> Result<AbsorbingAnalysis, MarkovError> {
    let n = dtmc.len();
    let absorbing: Vec<usize> = (0..n).filter(|&i| is_absorbing(dtmc, i)).collect();
    if absorbing.is_empty() {
        return Err(MarkovError::NoAbsorption);
    }
    let transient: Vec<usize> = (0..n).filter(|i| !is_absorbing(dtmc, *i)).collect();
    let t_count = transient.len();
    let a_count = absorbing.len();

    if t_count == 0 {
        // Every state is absorbing: nothing to analyze.
        return Ok(AbsorbingAnalysis {
            expected_time_to_absorption: Vec::new(),
            absorption_probabilities: Vec::new(),
            transient_states: Vec::new(),
            absorbing_states: absorbing,
        });
    }

    // Canonical blocks: Q (transient→transient), R (transient→absorbing).
    let mut q_data = Vec::with_capacity(t_count * t_count);
    for &i in &transient {
        for &j in &transient {
            q_data.push(dtmc.p(i, j));
        }
    }
    let q = DMatrix::from_row_slice(t_count, t_count, &q_data);
    let identity = DMatrix::from_diagonal(&DVector::from_fn(t_count, |_| 1.0));
    let fundamental = (identity - q)
        .inverse()
        .map_err(|_| MarkovError::NotConvergent)?;

    // t = N·1
    let ones = DVector::from_fn(t_count, |_| 1.0);
    let times = fundamental.clone() * ones;

    // B = N·R
    let mut r_data = Vec::with_capacity(t_count * a_count);
    for &i in &transient {
        for &a in &absorbing {
            r_data.push(dtmc.p(i, a));
        }
    }
    let r = DMatrix::from_row_slice(t_count, a_count, &r_data);
    let b = fundamental * r;

    let expected_time_to_absorption: Vec<f64> = (0..t_count).map(|i| times[i]).collect();
    if expected_time_to_absorption
        .iter()
        .any(|t| !t.is_finite() || *t < -1e-9)
    {
        return Err(MarkovError::NoAbsorption);
    }
    let absorption_probabilities: Vec<Vec<f64>> = (0..t_count)
        .map(|i| (0..a_count).map(|j| b[(i, j)]).collect())
        .collect();

    Ok(AbsorbingAnalysis {
        expected_time_to_absorption,
        absorption_probabilities,
        transient_states: transient,
        absorbing_states: absorbing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// Gambler's ruin on {0..4}: absorbing at 0 and 4, fair coin otherwise.
    fn gamblers_ruin() -> Dtmc {
        let n = 5;
        let mut rows = vec![vec![0.0; n]; n];
        rows[0][0] = 1.0;
        rows[4][4] = 1.0;
        for i in 1..4 {
            rows[i][i - 1] = 0.5;
            rows[i][i + 1] = 0.5;
        }
        Dtmc::from_rows(rows).unwrap()
    }

    #[test]
    fn expected_absorption_time_is_i_times_n_minus_i() {
        let analysis = analyze(&gamblers_ruin()).unwrap();
        assert_eq!(analysis.transient_states, vec![1, 2, 3]);
        assert_eq!(analysis.absorbing_states, vec![0, 4]);
        // Classic result: t_i = i(N−i) with N = 4.
        assert!(close(analysis.expected_time_to_absorption[0], 3.0, 1e-9));
        assert!(close(analysis.expected_time_to_absorption[1], 4.0, 1e-9));
        assert!(close(analysis.expected_time_to_absorption[2], 3.0, 1e-9));
    }

    #[test]
    fn absorption_probabilities_are_i_over_n() {
        let analysis = analyze(&gamblers_ruin()).unwrap();
        // P(reach 4) = i/4 from transient state i.
        assert!(close(analysis.absorption_probabilities[0][1], 0.25, 1e-9));
        assert!(close(analysis.absorption_probabilities[1][1], 0.5, 1e-9));
        assert!(close(analysis.absorption_probabilities[2][1], 0.75, 1e-9));
        // Rows sum to 1.
        for row in &analysis.absorption_probabilities {
            assert!(close(row.iter().sum(), 1.0, 1e-9));
        }
    }

    #[test]
    fn all_absorbing_chain_yields_empty_analysis() {
        let chain = Dtmc::from_rows(vec![vec![1.0]]).unwrap();
        let analysis = analyze(&chain).unwrap();
        assert!(analysis.expected_time_to_absorption.is_empty());
    }

    #[test]
    fn rejects_chain_without_absorption() {
        let chain = Dtmc::from_rows(vec![vec![0.5, 0.5], vec![0.5, 0.5]]).unwrap();
        assert_eq!(analyze(&chain), Err(MarkovError::NoAbsorption));
    }
}
