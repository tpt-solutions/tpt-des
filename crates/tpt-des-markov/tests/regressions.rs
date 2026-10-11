//! Regression tests for the Phase 6 Markov fixes.

use tpt_des_markov::{absorb, ctmc::matrix_exponential, Ctmc, Dtmc, MarkovError};
use tpt_math_linalg_dense::DMatrix;

/// Period-2 chain: 0 → 1, 1 → {0, 2} (½ each), 2 → 1. Its stationary
/// distribution is (¼, ½, ¼). Power iteration from the uniform start
/// oscillates between (⅓,⅓,⅓) and (1/6,⅔,1/6) forever, so it never converged.
#[test]
fn stationary_converges_for_periodic_chain() {
    let chain = Dtmc::from_rows(vec![
        vec![0.0, 1.0, 0.0],
        vec![0.5, 0.0, 0.5],
        vec![0.0, 1.0, 0.0],
    ])
    .unwrap();
    let pi = chain
        .stationary(1e-12, 100)
        .expect("periodic chain must solve");
    assert!((pi[0] - 0.25).abs() < 1e-12, "pi = {pi:?}");
    assert!((pi[1] - 0.5).abs() < 1e-12, "pi = {pi:?}");
    assert!((pi[2] - 0.25).abs() < 1e-12, "pi = {pi:?}");
}

/// Two disjoint absorbing states: no unique stationary distribution.
#[test]
fn stationary_of_reducible_chain_is_singular() {
    let chain = Dtmc::from_rows(vec![vec![1.0, 0.0], vec![0.0, 1.0]]).unwrap();
    assert_eq!(chain.stationary(1e-12, 100), Err(MarkovError::Singular));
}

#[test]
fn stationary_rejects_nan_tolerance() {
    let chain = Dtmc::from_rows(vec![vec![0.9, 0.1], vec![0.5, 0.5]]).unwrap();
    assert_eq!(
        chain.stationary(f64::NAN, 100),
        Err(MarkovError::InvalidTolerance)
    );
}

/// `exp(A)` for A = c·J (J the all-ones matrix) has the closed form
/// I + ((e^{nc} − 1)/n)·J. With n = 50, c = 0.1 the infinity norm is 5 while
/// the max entry is 0.1. Scaling by the max entry skips scaling entirely and
/// the 24-term Taylor truncation then misses the closed form by ~1e-8.
#[test]
fn expm_scales_by_infinity_norm() {
    let n = 50;
    let c = 0.1;
    let a = DMatrix::from_fn(n, n, |_, _| c);
    let e = matrix_exponential(&a);
    let scale = ((n as f64 * c).exp() - 1.0) / n as f64;
    let mut worst = 0.0f64;
    for i in 0..n {
        for j in 0..n {
            let expected = (if i == j { 1.0 } else { 0.0 }) + scale;
            worst = worst.max((e[(i, j)] - expected).abs());
        }
    }
    assert!(worst < 1e-12, "max error {worst:e}");
}

#[test]
fn ctmc_rejects_nan_tolerance() {
    // Row sums are 1 and 1, so this is invalid; a NaN tolerance used to
    // disable the row-sum check entirely and accept it.
    let rows = vec![vec![-1.0, 2.0], vec![1.0, -1.0]];
    assert_eq!(
        Ctmc::new(&rows, f64::NAN),
        Err(MarkovError::InvalidTolerance)
    );
}

/// State 0 absorbs; states 1 and 2 cycle between each other forever. The
/// transient block (I − Q) is singular, so absorption is not certain.
#[test]
fn absorbing_analysis_reports_singular_for_non_absorbing_class() {
    let chain = Dtmc::from_rows(vec![
        vec![1.0, 0.0, 0.0],
        vec![0.0, 0.0, 1.0],
        vec![0.0, 1.0, 0.0],
    ])
    .unwrap();
    assert_eq!(absorb::analyze(&chain).err(), Some(MarkovError::Singular));
}
