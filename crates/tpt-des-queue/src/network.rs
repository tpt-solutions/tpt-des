//! Networks of queues.
//!
//! * [`JacksonNetwork`] — open networks with a routing matrix: solves the
//!   traffic equations `λ = γ + Pᵀλ` for the node arrival rates (via the
//!   in-house `tpt-math-linalg-dense` solver) and evaluates the
//!   product-form per-node M/M/1 metrics.
//! * [`ClosedNetwork`] — single-chain closed networks solved exactly by
//!   mean value analysis (MVA).

use crate::error::QueueError;
use tpt_math_linalg_dense::{DMatrix, DVector};

/// An open Jackson network: `n` single-server M/M/1 nodes, exogenous arrival
/// rates `external`, and a routing matrix `routing` where `routing[i][j]` is
/// the probability a job leaving node `i` next visits node `j`.
///
/// Rows must be sub-stochastic (sum ≤ 1; the remainder leaves the network)
/// and the spectral radius of the routing matrix must be below one, which
/// the traffic-equation solve enforces implicitly.
#[derive(Clone, Debug, PartialEq)]
pub struct JacksonNetwork {
    routing: Vec<Vec<f64>>,
    external: Vec<f64>,
    service_rates: Vec<f64>,
}

impl JacksonNetwork {
    /// Validate and construct a Jackson network.
    pub fn new(
        external: &[f64],
        routing: &[Vec<f64>],
        service_rates: &[f64],
    ) -> Result<Self, QueueError> {
        let n = external.len();
        if n == 0
            || routing.len() != n
            || service_rates.len() != n
            || routing.iter().any(|r| r.len() != n)
        {
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
            if sum > 1.0 + 1e-9 {
                return Err(QueueError::InvalidRoutingMatrix);
            }
        }
        if external.iter().any(|&g| g < 0.0 || !g.is_finite())
            || service_rates.iter().any(|&m| m <= 0.0 || !m.is_finite())
        {
            return Err(QueueError::InvalidParameter);
        }
        Ok(Self {
            routing: routing.to_vec(),
            external: external.to_vec(),
            service_rates: service_rates.to_vec(),
        })
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.external.len()
    }

    /// True if the network has no nodes.
    pub fn is_empty(&self) -> bool {
        self.external.is_empty()
    }

    /// Solve the traffic equations `λ = γ + Pᵀλ` for the total arrival rate
    /// at each node.
    ///
    /// Returns [`QueueError::NoTrafficSolution`] when the routing matrix is
    /// not transient (spectral radius ≥ 1), detected as a non-finite or
    /// non-positive solution.
    pub fn node_arrivals(&self) -> Result<Vec<f64>, QueueError> {
        let n = self.len();
        // (I − Pᵀ) λ = γ, i.e. λ_j = γ_j + Σ_i λ_i · P[i][j].
        let mut a = vec![0.0; n * n];
        for j in 0..n {
            for i in 0..n {
                a[j * n + i] = if i == j {
                    1.0 - self.routing[i][j]
                } else {
                    -self.routing[i][j]
                };
            }
        }
        let matrix = DMatrix::from_row_slice(n, n, &a);
        let gamma = DVector::from_vec(self.external.clone());
        let solution = matrix
            .solve(&gamma)
            .map_err(|_| QueueError::NoTrafficSolution)?;
        let rates: Vec<f64> = solution.iter().copied().collect();
        if rates.iter().any(|&l| !l.is_finite() || l < 0.0) {
            return Err(QueueError::NoTrafficSolution);
        }
        Ok(rates)
    }

    /// Per-node utilization `ρ_i = λ_i / μ_i`.
    pub fn utilizations(&self) -> Result<Vec<f64>, QueueError> {
        let arrivals = self.node_arrivals()?;
        Ok(arrivals
            .iter()
            .zip(&self.service_rates)
            .map(|(&l, &m)| l / m)
            .collect())
    }

    /// Per-node mean number of jobs `L_i = ρ_i / (1 − ρ_i)` (product form)
    /// and the network total. Errors if any node is unstable.
    pub fn mean_lengths(&self) -> Result<(Vec<f64>, f64), QueueError> {
        let rho = self.utilizations()?;
        if rho.iter().any(|&r| r >= 1.0) {
            return Err(QueueError::Unstable);
        }
        let per_node: Vec<f64> = rho.iter().map(|&r| r / (1.0 - r)).collect();
        let total = per_node.iter().sum();
        Ok((per_node, total))
    }
}

/// A single-chain closed network with `N` customers cycling forever.
///
/// Characterized by per-node *service demands* `D_i = V_i · S_i` (visit
/// ratio times mean service time). Solved exactly by mean value analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct ClosedNetwork {
    demands: Vec<f64>,
}

/// MVA results for a closed network at population `N`.
#[derive(Clone, Debug, PartialEq)]
pub struct MvaResult {
    /// System throughput `X(N)`.
    pub throughput: f64,
    /// Per-node mean number of customers `L_i(N)`.
    pub queue_lengths: Vec<f64>,
    /// Per-node mean response times `W_i(N)`.
    pub response_times: Vec<f64>,
    /// Per-node utilizations `X(N)·D_i`.
    pub utilizations: Vec<f64>,
}

impl ClosedNetwork {
    /// Construct from service demands; all must be positive and finite.
    pub fn new(demands: &[f64]) -> Result<Self, QueueError> {
        if demands.is_empty() || demands.iter().any(|&d| d <= 0.0 || !d.is_finite()) {
            return Err(QueueError::InvalidParameter);
        }
        Ok(Self {
            demands: demands.to_vec(),
        })
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.demands.len()
    }

    /// True if the network has no nodes.
    pub fn is_empty(&self) -> bool {
        self.demands.is_empty()
    }

    /// Mean value analysis for population `N ≥ 1`.
    ///
    /// Uses the exact recursions `W_i(m) = D_i·(1 + L_i(m−1))`,
    /// `X(m) = m / Σ W_i(m)`, `L_i(m) = X(m)·W_i(m)`.
    pub fn mva(&self, population: u32) -> Result<MvaResult, QueueError> {
        if population == 0 {
            return Err(QueueError::InvalidParameter);
        }
        let n = self.len();
        let mut lengths = vec![0.0; n];
        let mut responses = vec![0.0; n];
        let mut throughput = 0.0;
        for m in 1..=population {
            let m = f64::from(m);
            let mut total_response = 0.0;
            for i in 0..n {
                responses[i] = self.demands[i] * (1.0 + lengths[i]);
                total_response += responses[i];
            }
            throughput = m / total_response;
            for i in 0..n {
                lengths[i] = throughput * responses[i];
            }
        }
        let utilizations = self.demands.iter().map(|&d| throughput * d).collect();
        Ok(MvaResult {
            throughput,
            queue_lengths: lengths,
            response_times: responses,
            utilizations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn feedforward_traffic_equations() {
        // γ = (0.5, 0); node 1 feeds node 2 entirely.
        let net = JacksonNetwork::new(&[0.5, 0.0], &[vec![0.0, 1.0], vec![0.0, 0.0]], &[1.0, 2.0])
            .unwrap();
        let arrivals = net.node_arrivals().unwrap();
        assert!(close(arrivals[0], 0.5, 1e-12));
        assert!(close(arrivals[1], 0.5, 1e-12));
    }

    #[test]
    fn feedback_traffic_equations() {
        // γ = (1, 0); node 1 sends half its output to node 2.
        let net = JacksonNetwork::new(&[1.0, 0.0], &[vec![0.0, 0.5], vec![0.0, 0.0]], &[2.0, 1.0])
            .unwrap();
        let arrivals = net.node_arrivals().unwrap();
        assert!(close(arrivals[0], 1.0, 1e-12));
        assert!(close(arrivals[1], 0.5, 1e-12));
        // ρ = (0.5, 0.5) → L = 1 + 1 = 2.
        let (lengths, total) = net.mean_lengths().unwrap();
        assert!(close(lengths[0], 1.0, 1e-12));
        assert!(close(lengths[1], 1.0, 1e-12));
        assert!(close(total, 2.0, 1e-12));
    }

    #[test]
    fn rejects_bad_routing() {
        // Row sums above one.
        assert_eq!(
            JacksonNetwork::new(&[1.0], &[vec![1.5]], &[1.0]),
            Err(QueueError::InvalidRoutingMatrix)
        );
        // Shape mismatch.
        assert_eq!(
            JacksonNetwork::new(&[1.0, 0.0], &[vec![0.0]], &[1.0, 1.0]),
            Err(QueueError::InvalidRoutingMatrix)
        );
    }

    #[test]
    fn mva_single_node_matches_mm1_closed_form() {
        // One node, one customer: W = D, X = 1/D, L = 1.
        let net = ClosedNetwork::new(&[2.0]).unwrap();
        let r = net.mva(1).unwrap();
        assert!(close(r.throughput, 0.5, 1e-12));
        assert!(close(r.queue_lengths[0], 1.0, 1e-12));
    }

    #[test]
    fn mva_balanced_two_node_network() {
        // D = (1, 1), N = 4: bottleneck bound X ≤ 1/2; exact X = 4/(2+3)=0.8
        // by the recursion: m=1: W=(1,1), X=1/2, L=(0.5,0.5);
        // m=2: W=(1.5,1.5), X=2/3, L=(1,1);
        // m=3: W=(2,2), X=3/4, L=(1.5,1.5);
        // m=4: W=(2.5,2.5), X=0.8, L=(2,2).
        let net = ClosedNetwork::new(&[1.0, 1.0]).unwrap();
        let r = net.mva(4).unwrap();
        assert!(close(r.throughput, 0.8, 1e-12));
        assert!(close(r.queue_lengths[0], 2.0, 1e-12));
        assert!(close(r.utilizations[0], 0.8, 1e-12));
    }
}
