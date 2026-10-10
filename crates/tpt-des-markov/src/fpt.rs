//! First-passage and recurrence times.
//!
//! * [`mean_first_passage`] — expected number of steps to reach `target`
//!   from every state, via the absorbing-chain linear solve per target.
//! * [`mean_recurrence_times`] — `1/π_i` from the stationary distribution
//!   (Kac's formula).

use crate::dtmc::Dtmc;
use crate::error::MarkovError;
use alloc::vec::Vec;
use tpt_math_linalg_dense::{DMatrix, DVector};

/// Expected number of steps to reach `target` from every state
/// (`0.0` for the target itself, [`f64::INFINITY`] when the target is not
/// reachable from a state).
///
/// Internally: make `target` absorbing, drop its row/column, and solve
/// `(I − Q̃)·m = 1` for the remaining states.
pub fn mean_first_passage(dtmc: &Dtmc, target: usize) -> Result<Vec<f64>, MarkovError> {
    let n = dtmc.len();
    if target >= n {
        return Err(MarkovError::Empty);
    }
    let others: Vec<usize> = (0..n).filter(|&i| i != target).collect();
    if others.is_empty() {
        return Ok(vec![0.0]);
    }
    // Only states with a directed path to the target participate; a closed
    // class elsewhere would make the system singular.
    let solvable: Vec<usize> = others
        .iter()
        .copied()
        .filter(|&s| can_reach(dtmc, s, target))
        .collect();
    let k = solvable.len();

    let mut result = vec![f64::INFINITY; n];
    result[target] = 0.0;
    if k == 0 {
        return Ok(result);
    }

    // Q̃ = P restricted to the solvable states.
    let mut q_data = Vec::with_capacity(k * k);
    for &i in &solvable {
        for &j in &solvable {
            q_data.push(dtmc.p(i, j));
        }
    }
    let q = DMatrix::from_row_slice(k, k, &q_data);
    let identity = DMatrix::from_diagonal(&DVector::from_fn(k, |_| 1.0));
    let system = identity - q;
    let ones = DVector::from_fn(k, |_| 1.0);
    let solution = system
        .solve(&ones)
        .map_err(|_| MarkovError::NotConvergent)?;

    for (idx, &state) in solvable.iter().enumerate() {
        result[state] = solution[idx];
    }
    Ok(result)
}

/// Mean recurrence time of each state: `1/π_i` (Kac's formula); `∞` for
/// states with zero stationary mass (not positive recurrent).
pub fn mean_recurrence_times(stationary: &[f64]) -> Vec<f64> {
    stationary
        .iter()
        .map(|&pi| if pi > 0.0 { 1.0 / pi } else { f64::INFINITY })
        .collect()
}

/// Directed reachability over the support of `P` (BFS).
fn can_reach(dtmc: &Dtmc, from: usize, to: usize) -> bool {
    if from == to {
        return true;
    }
    let n = dtmc.len();
    let mut visited = alloc::vec![false; n];
    let mut queue = alloc::collections::VecDeque::new();
    visited[from] = true;
    queue.push_back(from);
    while let Some(node) = queue.pop_front() {
        let successors: alloc::vec::Vec<usize> =
            (0..n).filter(|&j| dtmc.p(node, j) > 0.0).collect();
        for next in successors {
            if !visited[next] {
                if next == to {
                    return true;
                }
                visited[next] = true;
                queue.push_back(next);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtmc::Dtmc;
    use alloc::vec;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn symmetric_three_state_ring() {
        // 0 → 1 → 2 → 0 deterministically: MFPT to 0 is 1 from 2, 2 from 1.
        let chain = Dtmc::from_rows(vec![
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
            vec![1.0, 0.0, 0.0],
        ])
        .unwrap();
        let m0 = mean_first_passage(&chain, 0).unwrap();
        assert!(close(m0[0], 0.0, 1e-12));
        assert!(close(m0[1], 2.0, 1e-9));
        assert!(close(m0[2], 1.0, 1e-9));
    }

    #[test]
    fn two_state_symmetric_mfpt() {
        let chain = Dtmc::from_rows(vec![vec![0.5, 0.5], vec![0.5, 0.5]]).unwrap();
        let m0 = mean_first_passage(&chain, 0).unwrap();
        assert!(close(m0[1], 2.0, 1e-9)); // geometric(0.5) mean
        let m1 = mean_first_passage(&chain, 1).unwrap();
        assert!(close(m1[0], 2.0, 1e-9));
    }

    #[test]
    fn unreachable_target_is_infinite() {
        // Two disjoint communicating classes: {0, 1} and {2, 3}.
        let chain = Dtmc::from_rows(vec![
            vec![0.5, 0.5, 0.0, 0.0],
            vec![0.5, 0.5, 0.0, 0.0],
            vec![0.0, 0.0, 0.5, 0.5],
            vec![0.0, 0.0, 0.5, 0.5],
        ])
        .unwrap();
        let m1 = mean_first_passage(&chain, 1).unwrap();
        assert!(m1[2].is_infinite());
        assert!(m1[3].is_infinite());
        assert!(close(m1[0], 2.0, 1e-9));
        assert_eq!(m1[1], 0.0);
    }

    #[test]
    fn recurrence_times_from_stationary() {
        // Two-state chain with π = (5/6, 1/6): recurrence times (6/5, 6).
        let chain = Dtmc::from_rows(vec![vec![0.9, 0.1], vec![0.5, 0.5]]).unwrap();
        let pi = chain.stationary(1e-14, 10_000).unwrap();
        let r = mean_recurrence_times(&pi);
        assert!(close(r[0], 6.0 / 5.0, 1e-6));
        assert!(close(r[1], 6.0, 1e-6));
    }

    #[test]
    fn kac_identity_holds_on_ring() {
        // On the deterministic 3-ring, π = (1/3, 1/3, 1/3) and every
        // recurrence time is 3.
        let chain = Dtmc::from_rows(vec![
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
            vec![1.0, 0.0, 0.0],
        ])
        .unwrap();
        let pi = chain.stationary(1e-14, 10_000).unwrap();
        let r = mean_recurrence_times(&pi);
        for t in r {
            assert!(close(t, 3.0, 1e-6));
        }
    }
}
