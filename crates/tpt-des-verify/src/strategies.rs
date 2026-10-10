//! Reusable `proptest` strategies for stochastic models.
//!
//! Every strategy produces *valid* models only: stable queuing parameters,
//! row-stochastic matrices, sub-stochastic routing matrices with spectral
//! radius strictly below one, acyclic digraphs, and bounded Petri net
//! markings. These are the building blocks for the statistical verification
//! of stochastic laws across the workspace.

use proptest::prelude::*;

/// A finite, non-negative simulation time (raw units).
pub fn finite_time() -> impl Strategy<Value = f64> {
    (0.0f64..1e6).prop_map(|t| t)
}

/// Queuing parameters `(lambda, mu)` with `0 < lambda < mu` (stability).
pub fn stable_queue_params() -> impl Strategy<Value = (f64, f64)> {
    (0.05f64..0.95, 0.5f64..20.0).prop_map(|(rho, mu)| (rho * mu, mu))
}

/// An `n x n` row-stochastic matrix: every row is non-negative and sums to 1
/// (up to `f64` rounding, with the last entry absorbing the residual).
pub fn row_stochastic_matrix(n: usize) -> impl Strategy<Value = Vec<Vec<f64>>> {
    prop::collection::vec(prop::collection::vec(0.01f64..1.0, n..=n), n..=n).prop_map(move |rows| {
        rows.into_iter()
            .map(|row| {
                let sum: f64 = row.iter().sum();
                let mut norm: Vec<f64> = row.iter().map(|w| w / sum).collect();
                // Absorb rounding residue in the last entry so the row is as
                // close to exactly 1.0 as `f64` allows.
                let head: f64 = norm[..n - 1].iter().sum();
                norm[n - 1] = 1.0 - head;
                norm
            })
            .collect()
    })
}

/// An `n x n` routing matrix for an open Jackson network: non-negative
/// entries with every row summing to at most `0.99`, which guarantees the
/// spectral radius is below one (traffic strictly leaks out of the network).
pub fn sub_stochastic_routing(n: usize) -> impl Strategy<Value = Vec<Vec<f64>>> {
    prop::collection::vec(prop::collection::vec(0.0f64..0.9, n..=n), n..=n).prop_map(move |rows| {
        rows.into_iter()
            .map(|row| {
                let sum: f64 = row.iter().sum();
                if sum <= 0.99 {
                    row
                } else {
                    let scale = 0.99 / sum;
                    row.into_iter().map(|p| p * scale).collect()
                }
            })
            .collect()
    })
}

/// A random acyclic digraph on `n` nodes as an edge list: edges always go
/// from a lower index to a higher index, so no cycle can exist by
/// construction. Useful as the "known-good" case for cycle detectors.
pub fn acyclic_edge_list(n: usize, max_edges: usize) -> impl Strategy<Value = Vec<(usize, usize)>> {
    prop::collection::vec((0usize..n, 0usize..n), 0..=max_edges).prop_map(move |edges| {
        edges
            .into_iter()
            .filter_map(|(a, b)| {
                if a < b {
                    Some((a, b))
                } else if b < a {
                    Some((b, a))
                } else {
                    None
                }
            })
            .collect()
    })
}

/// A bounded Petri net marking: `n` places, `0..=max_tokens` tokens each.
pub fn marking(n: usize, max_tokens: u32) -> impl Strategy<Value = Vec<u32>> {
    prop::collection::vec(0u32..=max_tokens, n..=n)
}

/// A schedule plan: `(time, priority)` pairs for the event calendar tests.
pub fn schedule_plan(max_events: usize) -> impl Strategy<Value = Vec<(u32, u8)>> {
    prop::collection::vec((0u32..1000, proptest::num::u8::ANY), 0..=max_events)
}

/// A random bit vector (used for choosing which events to cancel).
pub fn cancellation_mask(len: usize) -> impl Strategy<Value = Vec<bool>> {
    prop::collection::vec(proptest::bool::ANY, len..=len)
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn row_stochastic_rows_sum_to_one(rows in row_stochastic_matrix(4)) {
            for row in &rows {
                let sum: f64 = row.iter().sum();
                prop_assert!((sum - 1.0).abs() < 1e-9);
                for w in row {
                    prop_assert!(*w >= 0.0);
                }
            }
        }

        #[test]
        fn routing_rows_are_substochastic(rows in sub_stochastic_routing(3)) {
            for row in &rows {
                let sum: f64 = row.iter().sum();
                prop_assert!(sum <= 0.99 + 1e-12);
            }
        }

        #[test]
        fn stable_params_are_stable((lambda, mu) in stable_queue_params()) {
            prop_assert!(lambda > 0.0 && lambda < mu);
        }
    }
}
