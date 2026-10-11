//! Multi-class closed networks solved by exact multi-class mean value
//! analysis (Reiser & Lavenberg, 1980).
//!
//! Each class is a closed chain with its own population, its own routing
//! matrix, and its own mean service time at each node. The visit ratios of a
//! class come from its routing matrix (node 0 is the reference, with visit
//! ratio 1), and the service demand of class `c` at node `i` is
//! `D[c][i] = V[c][i] · S[c][i]`.
//!
//! The recursion runs over the population lattice `0 ≤ m ≤ N`, so its cost
//! grows with `Π(N_c + 1)`. [`MultiClassClosed::mva`] refuses lattices larger
//! than [`MAX_STATES`].
//!
//! # Exactness
//!
//! The recursion is exact for product-form nodes: processor-sharing nodes,
//! and FCFS nodes whose service is exponential with the same mean for every
//! class. Other service disciplines can differ from it.

use crate::error::QueueError;
use tpt_math_linalg_dense::{DMatrix, DVector};

/// Largest population lattice (`Π(N_c + 1)`) the exact recursion will build.
pub const MAX_STATES: usize = 100_000;

/// One closed customer class: its population, routing, and service times.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassSpec {
    /// Number of circulating customers of this class.
    pub population: u32,
    /// Row-stochastic routing: `routing[i][j]` is the probability a class job
    /// that leaves node `i` next visits node `j`.
    pub routing: Vec<Vec<f64>>,
    /// Mean service time of this class at each node, in the same units as
    /// the other classes' service times.
    pub service_times: Vec<f64>,
}

/// A multi-class closed network with its per-class service demands computed.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiClassClosed {
    populations: Vec<u32>,
    /// `demands[c][i]`: class `c`'s service demand at node `i`.
    demands: Vec<Vec<f64>>,
}

/// Exact MVA results at the full population.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct MultiClassResult {
    /// Throughput of each class, `X_c(N)`. Zero for a class with no customers.
    pub throughput: Vec<f64>,
    /// Mean number of class `c` customers at node `i`: `[c][i]`.
    pub queue_lengths: Vec<Vec<f64>>,
    /// Mean response time of class `c` at node `i` per visit: `[c][i]`.
    pub response_times: Vec<Vec<f64>>,
    /// Total utilization of each node, `Σ_c X_c·D[c][i]`.
    pub utilizations: Vec<f64>,
}

impl MultiClassClosed {
    /// Validate the classes and compute their service demands.
    ///
    /// # Errors
    ///
    /// * [`QueueError::InvalidParameter`] if there are no classes or nodes,
    ///   no class has customers, a service time is non-positive or
    ///   non-finite, or a class with customers has zero total demand.
    /// * [`QueueError::InvalidRoutingMatrix`] if a routing matrix has the
    ///   wrong shape, a negative or non-finite entry, a row that does not sum
    ///   to one, or is not irreducible (its visit-ratio system is singular).
    pub fn new(classes: &[ClassSpec]) -> Result<Self, QueueError> {
        let n = classes.first().map_or(0, |c| c.service_times.len());
        if classes.is_empty() || n == 0 || classes.iter().all(|c| c.population == 0) {
            return Err(QueueError::InvalidParameter);
        }

        let mut populations = Vec::with_capacity(classes.len());
        let mut demands = Vec::with_capacity(classes.len());
        for class in classes {
            validate_routing(&class.routing, n)?;
            if class.service_times.len() != n
                || class
                    .service_times
                    .iter()
                    .any(|&s| s <= 0.0 || !s.is_finite())
            {
                return Err(QueueError::InvalidParameter);
            }
            let visits = visit_ratios(&class.routing)?;
            let demand: Vec<f64> = visits
                .iter()
                .zip(&class.service_times)
                .map(|(&v, &s)| v * s)
                .collect();
            if class.population > 0 && demand.iter().sum::<f64>() <= 0.0 {
                return Err(QueueError::InvalidParameter);
            }
            populations.push(class.population);
            demands.push(demand);
        }
        Ok(Self {
            populations,
            demands,
        })
    }

    /// Number of classes.
    pub fn classes(&self) -> usize {
        self.populations.len()
    }

    /// Number of nodes.
    pub fn nodes(&self) -> usize {
        self.demands.first().map_or(0, Vec::len)
    }

    /// The service demand `D[c][i]` of class `c` at node `i`.
    pub fn demand(&self, class: usize, node: usize) -> f64 {
        self.demands[class][node]
    }

    /// Exact multi-class MVA at the full population.
    ///
    /// # Errors
    ///
    /// [`QueueError::StateSpaceTooLarge`] if `Π(N_c + 1)` exceeds
    /// [`MAX_STATES`].
    pub fn mva(&self) -> Result<MultiClassResult, QueueError> {
        let c_count = self.classes();
        let n = self.nodes();

        // Mixed-radix index over the population lattice: class `c` has radix
        // N_c + 1, and the last index is the full population N.
        let mut stride = Vec::with_capacity(c_count);
        let mut total: usize = 1;
        for &pop in &self.populations {
            stride.push(total);
            total = total
                .checked_mul(pop as usize + 1)
                .filter(|&t| t <= MAX_STATES)
                .ok_or(QueueError::StateSpaceTooLarge)?;
        }

        // lengths[idx][c·n + i]: mean class-c customers at node i with the
        // population encoded by idx.
        let mut lengths = vec![vec![0.0f64; c_count * n]; total];
        let last = total - 1;
        let mut final_w = vec![vec![0.0f64; n]; c_count];
        let mut final_x = vec![0.0f64; c_count];

        for idx in 1..total {
            for c in 0..c_count {
                let m_c = (idx / stride[c]) % (self.populations[c] as usize + 1);
                if m_c == 0 {
                    continue;
                }
                // Arriving class-c customer sees the lattice point m − e_c.
                let prev = idx - stride[c];
                let mut w = vec![0.0; n];
                let mut total_w = 0.0;
                for (i, wi) in w.iter_mut().enumerate() {
                    let queue_seen: f64 = (0..c_count).map(|k| lengths[prev][k * n + i]).sum();
                    *wi = self.demands[c][i] * (1.0 + queue_seen);
                    total_w += *wi;
                }
                let x = m_c as f64 / total_w;
                for (i, &wi) in w.iter().enumerate() {
                    lengths[idx][c * n + i] = x * wi;
                }
                if idx == last {
                    final_w[c] = w;
                    final_x[c] = x;
                }
            }
        }

        let queue_lengths: Vec<Vec<f64>> = (0..c_count)
            .map(|c| lengths[last][c * n..(c + 1) * n].to_vec())
            .collect();
        let utilizations = (0..n)
            .map(|i| {
                (0..c_count)
                    .map(|c| final_x[c] * self.demands[c][i])
                    .sum::<f64>()
            })
            .collect();
        Ok(MultiClassResult {
            throughput: final_x,
            queue_lengths,
            response_times: final_w,
            utilizations,
        })
    }
}

fn validate_routing(routing: &[Vec<f64>], n: usize) -> Result<(), QueueError> {
    if routing.len() != n || routing.iter().any(|r| r.len() != n) {
        return Err(QueueError::InvalidRoutingMatrix);
    }
    for row in routing {
        let mut sum = 0.0;
        for &p in row {
            if p < 0.0 || !p.is_finite() {
                return Err(QueueError::InvalidRoutingMatrix);
            }
            sum += p;
        }
        // A closed chain keeps every job in the network, so rows sum to one.
        if (sum - 1.0).abs() > 1e-9 {
            return Err(QueueError::InvalidRoutingMatrix);
        }
    }
    Ok(())
}

/// Visit ratios `v` with `v = Pᵀv` and `v[0] = 1`: one balance equation is
/// replaced by the normalization, which makes the solve well posed for an
/// irreducible chain.
fn visit_ratios(routing: &[Vec<f64>]) -> Result<Vec<f64>, QueueError> {
    let n = routing.len();
    let mut a = vec![0.0; n * n];
    let mut rhs = vec![0.0; n];
    for j in 0..n {
        for i in 0..n {
            // Row j of (I − Pᵀ): coefficient of v_i is δ_ij − P[i][j].
            a[j * n + i] = if i == j {
                1.0 - routing[i][j]
            } else {
                -routing[i][j]
            };
        }
    }
    // Replace row 0 with v_0 = 1.
    a[..n].fill(0.0);
    a[0] = 1.0;
    rhs[0] = 1.0;

    let matrix = DMatrix::from_row_slice(n, n, &a);
    let solution = matrix
        .solve(&DVector::from_vec(rhs))
        .map_err(|_| QueueError::InvalidRoutingMatrix)?;
    let visits: Vec<f64> = solution.iter().copied().collect();
    // A reducible chain gives visit ratios that are negative or not finite.
    if visits.iter().any(|&v| !v.is_finite() || v < -1e-9) {
        return Err(QueueError::InvalidRoutingMatrix);
    }
    Ok(visits.into_iter().map(|v| v.max(0.0)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::ClosedNetwork;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// A cycle of `n` nodes: node `i` always routes to `i+1 mod n`.
    fn cycle(n: usize) -> Vec<Vec<f64>> {
        (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| if j == (i + 1) % n { 1.0 } else { 0.0 })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn single_class_matches_single_chain_mva() {
        // A cycle of two nodes with service times (1, 1) and N = 4 customers
        // is the balanced network the single-chain MVA test solves: X = 0.8.
        let class = ClassSpec {
            population: 4,
            routing: cycle(2),
            service_times: vec![1.0, 1.0],
        };
        let multi = MultiClassClosed::new(&[class]).unwrap().mva().unwrap();
        let single = ClosedNetwork::new(&[1.0, 1.0]).unwrap().mva(4).unwrap();
        assert!(close(multi.throughput[0], single.throughput, 1e-12));
        assert!(close(
            multi.queue_lengths[0][0],
            single.queue_lengths[0],
            1e-12
        ));
        assert!(close(multi.utilizations[0], single.utilizations[0], 1e-12));
    }

    #[test]
    fn customers_are_conserved_across_classes() {
        let classes = [
            ClassSpec {
                population: 2,
                routing: cycle(3),
                service_times: vec![0.5, 1.0, 2.0],
            },
            ClassSpec {
                population: 3,
                routing: cycle(3),
                service_times: vec![2.0, 0.25, 0.5],
            },
        ];
        let r = MultiClassClosed::new(&classes).unwrap().mva().unwrap();
        let total: f64 = r.queue_lengths.iter().flatten().sum();
        assert!(close(total, 5.0, 1e-9), "Σ L = {total}, expected 5");
    }

    #[test]
    fn empty_class_contributes_nothing() {
        let busy = ClassSpec {
            population: 3,
            routing: cycle(2),
            service_times: vec![1.0, 2.0],
        };
        let idle = ClassSpec {
            population: 0,
            routing: cycle(2),
            service_times: vec![5.0, 5.0],
        };
        let with_idle = MultiClassClosed::new(&[busy.clone(), idle])
            .unwrap()
            .mva()
            .unwrap();
        let alone = MultiClassClosed::new(&[busy]).unwrap().mva().unwrap();
        assert_eq!(with_idle.throughput[1], 0.0);
        assert!(close(with_idle.throughput[0], alone.throughput[0], 1e-12));
    }

    #[test]
    fn visit_ratios_follow_routing() {
        // Node 0 sends half its output to node 1 and half back to itself;
        // node 1 returns everything to node 0. Visits: v0 = 1, v1 = 0.5.
        let routing = vec![vec![0.5, 0.5], vec![1.0, 0.0]];
        let v = visit_ratios(&routing).unwrap();
        assert!(close(v[0], 1.0, 1e-12));
        assert!(close(v[1], 0.5, 1e-12));
    }

    #[test]
    fn rejects_open_or_reducible_routing() {
        // Row sums to 0.5: jobs leave, which a closed chain cannot do.
        let leaky = ClassSpec {
            population: 1,
            routing: vec![vec![0.5, 0.0], vec![1.0, 0.0]],
            service_times: vec![1.0, 1.0],
        };
        assert_eq!(
            MultiClassClosed::new(&[leaky]),
            Err(QueueError::InvalidRoutingMatrix)
        );
        // Two absorbing-free islands: node 0 only ever routes to itself, so
        // node 1 is unreachable and the chain is reducible.
        let split = ClassSpec {
            population: 1,
            routing: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
            service_times: vec![1.0, 1.0],
        };
        assert_eq!(
            MultiClassClosed::new(&[split]),
            Err(QueueError::InvalidRoutingMatrix)
        );
    }

    #[test]
    fn oversized_lattice_is_refused() {
        // 40 × 40 × 40 = 64 000 states is fine; 40 × 40 × 40 × 40 is not.
        let class = |population| ClassSpec {
            population,
            routing: cycle(2),
            service_times: vec![1.0, 1.0],
        };
        let ok = [class(39), class(39), class(39)];
        assert!(MultiClassClosed::new(&ok).unwrap().mva().is_ok());
        let too_big = [class(39), class(39), class(39), class(39)];
        assert_eq!(
            MultiClassClosed::new(&too_big).unwrap().mva(),
            Err(QueueError::StateSpaceTooLarge)
        );
    }
}

#[cfg(test)]
mod symmetry_tests {
    use super::*;
    use crate::network::ClosedNetwork;

    /// Two identical classes with one customer each behave like one class
    /// with two customers: the product form depends only on the total.
    #[test]
    fn identical_classes_aggregate_to_single_chain() {
        let routing = vec![vec![0.0, 1.0], vec![1.0, 0.0]];
        let one_each = |population| ClassSpec {
            population,
            routing: routing.clone(),
            service_times: vec![1.0, 1.0],
        };
        let multi = MultiClassClosed::new(&[one_each(1), one_each(1)])
            .unwrap()
            .mva()
            .unwrap();
        let single = ClosedNetwork::new(&[1.0, 1.0]).unwrap().mva(2).unwrap();
        let total_x: f64 = multi.throughput.iter().sum();
        assert!((total_x - single.throughput).abs() < 1e-12, "{total_x}");
        let total_l: f64 = multi.queue_lengths.iter().flatten().sum();
        assert!((total_l - 2.0).abs() < 1e-12, "{total_l}");
    }
}
