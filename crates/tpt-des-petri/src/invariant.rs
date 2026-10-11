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

/// Non-negative minimal P-invariants, by the Farkas (Martínez–Silva)
/// algorithm: solutions `x ≥ 0` of `xᵀC = 0` with minimal support.
///
/// Each row carries its incidence part (`C` row, eliminated transition by
/// transition) alongside an identity part (the place weights). Combining a
/// positive and a negative entry in one transition column cancels it; rows
/// that survive all eliminations are exactly the non-negative invariants,
/// and only minimal supports are kept. Empty if the net conserves nothing.
pub fn place_invariants(net: &PetriNet) -> Vec<Vec<f64>> {
    let c = net.incidence();
    let places = net.places();
    let transitions = net.transitions();

    // (incidence part over transitions, identity part over places)
    let mut rows: Vec<(Vec<i64>, Vec<i64>)> = (0..places)
        .map(|p| {
            let mut identity = vec![0i64; places];
            identity[p] = 1;
            (c[p].clone(), identity)
        })
        .collect();

    for t in 0..transitions {
        let (mut next, rest): (Vec<_>, Vec<_>) = rows.into_iter().partition(|r| r.0[t] == 0);
        let pos: Vec<_> = rest.iter().filter(|r| r.0[t] > 0).cloned().collect();
        let neg: Vec<_> = rest.iter().filter(|r| r.0[t] < 0).cloned().collect();
        for p_row in &pos {
            for n_row in &neg {
                let a = -n_row.0[t];
                let b = p_row.0[t];
                let mut inc: Vec<i64> = (0..transitions)
                    .map(|k| a * p_row.0[k] + b * n_row.0[k])
                    .collect();
                let mut ident: Vec<i64> = (0..places)
                    .map(|k| a * p_row.1[k] + b * n_row.1[k])
                    .collect();
                let g = inc
                    .iter()
                    .chain(ident.iter())
                    .fold(0, |g, &v| gcd(g, v.abs()));
                if g > 1 {
                    inc.iter_mut().for_each(|v| *v /= g);
                    ident.iter_mut().for_each(|v| *v /= g);
                }
                next.push((inc, ident));
            }
        }
        next.sort();
        next.dedup();
        rows = next;
    }

    // Every surviving row has a zero incidence part: its identity part is a
    // non-negative P-invariant.
    let mut candidates: Vec<Vec<i64>> = rows.into_iter().map(|(_, ident)| ident).collect();
    candidates.sort();
    candidates.dedup();
    let supports: Vec<Vec<bool>> = candidates
        .iter()
        .map(|v| v.iter().map(|&x| x != 0).collect())
        .collect();
    // Keep minimal supports only: drop any invariant whose support strictly
    // contains another's.
    let minimal: Vec<Vec<f64>> = (0..candidates.len())
        .filter(|&i| {
            !(0..candidates.len()).any(|j| {
                j != i
                    && supports[j]
                        .iter()
                        .zip(&supports[i])
                        .all(|(&sj, &si)| !sj || si)
                    && supports[j] != supports[i]
            })
        })
        .map(|i| candidates[i].iter().map(|&x| x as f64).collect())
        .collect();
    minimal
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
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
