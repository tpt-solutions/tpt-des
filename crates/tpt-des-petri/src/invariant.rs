//! Invariant analysis: P-invariants, T-invariants, conservation checks.
//!
//! A *P-invariant* is a non-negative integer weight vector `x` with
//! `xᵀC = 0` (C the incidence matrix): the weighted token sum `xᵀm` is the
//! same in every reachable marking — the net conserves `x`. A *T-invariant*
//! `y` satisfies `Cy = 0`: firing every transition in `y`'s support the
//! given number of times returns to the same marking.
//!
//! The bases are computed by Gauss–Jordan elimination over the rationals
//! (f64 with near-integer rounding), from scratch per the zero-dependency
//! principle.

use crate::net::PetriNet;
use alloc::vec::Vec;

/// Numerical tolerance for elimination and rounding.
const EPS: f64 = 1e-9;

/// Compute a basis of the null space of `matrix` (rows × cols) via
/// Gauss–Jordan elimination over f64. Each basis vector is rounded to
/// integers when its entries are within [`EPS`] of them.
fn null_space(matrix: &[Vec<f64>], cols: usize) -> Vec<Vec<f64>> {
    let rows = matrix.len();
    // Augmented work matrix on a copy.
    let mut work = matrix.to_vec();
    let mut pivot_col_of_row: Vec<Option<usize>> = vec![None; rows];
    let mut pivot_cols: Vec<usize> = Vec::new();
    let mut r = 0;
    for c in 0..cols {
        // Find pivot.
        let pivot = work
            .iter()
            .enumerate()
            .skip(r)
            .find(|(_, row)| row[c].abs() > 1e3 * EPS)
            .map(|(i, _)| i);
        let Some(p) = pivot else { continue };
        work.swap(r, p);
        let inv = 1.0 / work[r][c];
        for v in work[r].iter_mut() {
            *v *= inv;
        }
        for i in 0..rows {
            if i != r && work[i][c].abs() > EPS {
                let factor = work[i][c];
                let pivot_row = work[r].clone();
                for (wj, wr) in work[i].iter_mut().zip(&pivot_row) {
                    *wj -= factor * wr;
                }
            }
        }
        pivot_col_of_row[r] = Some(c);
        pivot_cols.push(c);
        r += 1;
        if r == rows {
            break;
        }
    }
    let free: Vec<usize> = (0..cols).filter(|c| !pivot_cols.contains(c)).collect();

    // Special vector for each free column.
    let basis: Vec<Vec<f64>> = free
        .iter()
        .map(|&free_col| {
            let mut x = vec![0.0; cols];
            x[free_col] = 1.0;
            for (row_idx, &pcol) in pivot_cols.iter().enumerate() {
                // x[pcol] = −work[row_idx][free_col]
                x[pcol] = -work[row_idx][free_col];
            }
            // Round near-integers.
            x.into_iter()
                .map(|v| {
                    let rounded = v.round();
                    if (v - rounded).abs() < 1e3 * EPS {
                        rounded
                    } else {
                        v
                    }
                })
                .collect()
        })
        .collect();
    basis
}

/// Basis of the P-invariants: solutions of `xᵀC = 0`, i.e. the null space
/// of `Cᵀ` (transitions × places). Empty if the net conserves nothing
/// nontrivially.
pub fn place_invariants(net: &PetriNet) -> Vec<Vec<f64>> {
    let c = net.incidence();
    let places = net.places();
    let transitions = net.transitions();
    // Cᵀ: transitions rows, places columns.
    let ct: Vec<Vec<f64>> = (0..transitions)
        .map(|t| (0..places).map(|p| c[p][t] as f64).collect())
        .collect();
    null_space(&ct, places)
}

/// Basis of the T-invariants: solutions of `Cy = 0` — the null space of the
/// incidence matrix itself.
pub fn transition_invariants(net: &PetriNet) -> Vec<Vec<f64>> {
    let c = net.incidence();
    let rows: Vec<Vec<f64>> = c
        .iter()
        .map(|row| row.iter().map(|&v| v as f64).collect())
        .collect();
    null_space(&rows, net.transitions())
}

/// True if `weights` is a (nontrivial) conservation law: non-negative, not
/// all zero, and `weightsᵀC = 0` within tolerance.
pub fn is_conservation_law(net: &PetriNet, weights: &[f64]) -> bool {
    if weights.len() != net.places() || weights.iter().any(|&w| w < 0.0) {
        return false;
    }
    if weights.iter().all(|&w| w.abs() < EPS) {
        return false;
    }
    let c = net.incidence();
    for (t, _col) in c.iter().take(net.transitions()).enumerate() {
        let flow: f64 = (0..net.places()).map(|p| weights[p] * c[p][t] as f64).sum();
        if flow.abs() > 1e3 * EPS {
            return false;
        }
    }
    true
}

/// Weighted token sum `weightsᵀ·marking` (the conserved quantity).
pub fn weighted_total(net: &PetriNet, weights: &[f64], marking: &crate::net::Marking) -> f64 {
    let _ = net;
    (0..weights.len())
        .map(|p| weights[p] * marking.at(p) as f64)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Marking;
    use alloc::vec;

    /// Two-place loop: A →t1→ B →t2→ A. Invariant: [1, 1].
    fn two_place_loop() -> PetriNet {
        let mut net = PetriNet::new();
        let a = net.add_place("A");
        let b = net.add_place("B");
        let t1 = net.add_transition("t1");
        let t2 = net.add_transition("t2");
        net.add_arc_pt(a, t1, 1);
        net.add_arc_tp(t1, b, 1);
        net.add_arc_pt(b, t2, 1);
        net.add_arc_tp(t2, a, 1);
        net
    }

    #[test]
    fn loop_net_conserves_total_tokens() {
        let net = two_place_loop();
        let invariants = place_invariants(&net);
        assert!(!invariants.is_empty());
        assert!(invariants.iter().any(|x| is_conservation_law(&net, x)
            && (x[0] - 1.0).abs() < 1e-9
            && (x[1] - 1.0).abs() < 1e-9));
    }

    #[test]
    fn conservation_law_holds_over_reachable_markings() {
        let net = two_place_loop();
        let weights = [1.0, 1.0];
        for marking in [
            Marking(vec![2, 0]),
            Marking(vec![1, 1]),
            Marking(vec![0, 2]),
        ] {
            let total = weighted_total(&net, &weights, &marking);
            assert!((total - 2.0).abs() < 1e-12);
        }
    }

    #[test]
    fn t_invariants_of_loop_are_ones() {
        let net = two_place_loop();
        let t_invariants = transition_invariants(&net);
        // Firing t1 and t2 once each returns to the same marking: (1, 1).
        assert!(t_invariants
            .iter()
            .any(|y| (y[0] - 1.0).abs() < 1e-9 && (y[1] - 1.0).abs() < 1e-9));
    }

    #[test]
    fn open_net_conserves_nothing() {
        // A sink transition only: tokens leak, nothing is conserved.
        let mut net = PetriNet::new();
        let a = net.add_place("A");
        let drain = net.add_transition("drain");
        net.add_arc_pt(a, drain, 1);
        assert!(place_invariants(&net).is_empty());
        assert!(!is_conservation_law(&net, &[1.0]));
    }
}
