//! Distribution numerics: log-gamma, incomplete beta, Student-t and normal
//! quantiles, all implemented from scratch (Lanczos gamma, Lentz continued
//! fractions, bisection inversion).

/// Lanczos log-gamma (g=7, n=9 coefficients; |ε| < 1e-13 for x > 0).
#[must_use]
pub fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    // Literature constants: full published precision retained on purpose.
    #[allow(clippy::excessive_precision)]
    const COEFFS: [f64; 9] = [
        0.999_999_999_999_809_93,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_13,
        -176.615_029_162_140_59,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_576_2e-6,
        1.505_632_735_149_311_6e-7,
    ];
    // Γ has no finite log for x ≤ 0 (and NaN is outside the domain too).
    if x.is_nan() || x <= 0.0 {
        return f64::NAN;
    }
    if x < 0.5 {
        // Reflection: Γ(x)Γ(1−x) = π/sin(πx)
        core::f64::consts::PI.ln() - (core::f64::consts::PI * x).sin().ln() - ln_gamma(1.0 - x)
    } else {
        let xx = x - 1.0;
        let mut a = COEFFS[0];
        let t = xx + G + 0.5;
        for (i, &c) in COEFFS.iter().enumerate().skip(1) {
            a += c / (xx + i as f64);
        }
        0.5 * (2.0 * core::f64::consts::PI).ln() + (xx + 0.5) * t.ln() - t + a.ln()
    }
}

/// Continued fraction for the regularized incomplete beta.
///
/// Evaluates (modified Lentz) the DLMF 8.17.5 fraction
/// `F = 1/(1 + d₁/(1 + d₂/(1 + …)))` with
/// `d_{2k+1} = −(a+k)(a+b+k)x/((a+2k)(a+2k+1))` and
/// `d_{2k} = k(b−k)x/((a+2k−1)(a+2k))`, so that
/// `I_x(a,b) = x^a (1−x)^b F / (a·B(a,b))` on the forward branch.
fn beta_cont_fraction(a: f64, b: f64, x: f64) -> f64 {
    const MAX_ITER: usize = 400;
    const EPS: f64 = 1.0e-14;
    const TINY: f64 = 1.0e-300;

    let dcoef = |j: usize| -> f64 {
        if j % 2 == 1 {
            let k = (j - 1) / 2;
            let kf = k as f64;
            -(a + kf) * (a + b + kf) * x / ((a + 2.0 * kf) * (a + 2.0 * kf + 1.0))
        } else {
            let k = j / 2;
            let kf = k as f64;
            kf * (b - kf) * x / ((a + 2.0 * kf - 1.0) * (a + 2.0 * kf))
        }
    };

    // F = b0 + a1/(b1 + a2/(b2 + …)) with b0 = 0, a1 = 1, b_j = 1,
    // a_{j+1} = d_j.
    let mut f = TINY;
    let mut c = f;
    let mut d = 0.0_f64;
    for j in 1..=MAX_ITER {
        let a_j = if j == 1 { 1.0 } else { dcoef(j - 1) };
        d = 1.0 + a_j * d;
        if d.abs() < TINY {
            d = TINY;
        }
        d = 1.0 / d;
        c = 1.0 + a_j / c;
        if c.abs() < TINY {
            c = TINY;
        }
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() < EPS {
            break;
        }
    }
    f
}

/// Regularized incomplete beta function `I_x(a, b)` (|ε| < 1e-12).
#[must_use]
pub fn beta_inc(a: f64, b: f64, x: f64) -> f64 {
    if !(0.0..=1.0).contains(&x) {
        return if x < 0.0 { 0.0 } else { 1.0 };
    }
    if x == 0.0 {
        return 0.0;
    }
    if x == 1.0 {
        return 1.0;
    }
    let factor = (a * x.ln() + b * (1.0 - x).ln() - ln_beta(a, b)).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        factor * beta_cont_fraction(a, b, x) / a
    } else {
        1.0 - factor * beta_cont_fraction(b, a, 1.0 - x) / b
    }
}

/// `ln B(a, b) = ln Γ(a) + ln Γ(b) − ln Γ(a+b)`.
#[must_use]
fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// Student-t CDF with `df` degrees of freedom.
#[must_use]
pub fn student_t_cdf(t: f64, df: f64) -> f64 {
    let x = df / (df + t * t);
    let tail = 0.5 * beta_inc(df / 2.0, 0.5, x);
    if t > 0.0 {
        1.0 - tail
    } else {
        tail
    }
}

/// Student-t quantile (inverse CDF) by bisection — monotone and robust.
/// The bracket starts at ±10⁶ and halves until its width is below
/// `1e-12·max(1, |t|)`, about 45 halvings, each one CDF evaluation.
#[must_use]
pub fn student_t_ppf(p: f64, df: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) || df.is_nan() || df <= 0.0 {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    if (p - 0.5).abs() < 1e-15 {
        return 0.0;
    }
    let (mut lo, mut hi): (f64, f64) = (-1.0e6, 1.0e6);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if hi - lo <= 1e-12 * mid.abs().max(1.0) {
            break;
        }
        if student_t_cdf(mid, df) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Standard normal CDF `Φ(x)`.
///
/// Computed through `erfc` (private), so the lower tail keeps relative accuracy
/// (`Φ(−5) ≈ 2.8665e-7` is correct to many digits, where the previous
/// Abramowitz–Stegun 7.1.26 approximation was only good to about 1.5e-7
/// absolute).
#[must_use]
pub fn normal_cdf(x: f64) -> f64 {
    0.5 * erfc(-x / core::f64::consts::SQRT_2)
}

/// Complementary error function. A Taylor series covers `|x| < 3`; beyond
/// that the Laplace continued fraction (evaluated backwards) converges
/// quickly and keeps the tail accurate.
fn erfc(x: f64) -> f64 {
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    if x < 3.0 {
        1.0 - erf_series(x)
    } else {
        erfc_continued_fraction(x)
    }
}

/// `erf(x) = (2/√π) Σ (−1)ⁿ x^{2n+1} / (n!(2n+1))`, for moderate `x`.
fn erf_series(x: f64) -> f64 {
    let x2 = x * x;
    let mut term = x; // (−1)ⁿ x^{2n+1} / n!, starting at n = 0
    let mut sum = x;
    for n in 1..200 {
        term *= -x2 / n as f64;
        let contribution = term / (2 * n + 1) as f64;
        sum += contribution;
        if contribution.abs() < 1e-17 * sum.abs() {
            break;
        }
    }
    2.0 / core::f64::consts::PI.sqrt() * sum
}

/// `erfc(x) = e^{−x²}/√π · 1/(x + (1/2)/(x + 1/(x + (3/2)/(x + …))))`,
/// for `x ≥ 3`, evaluated from the deepest term outwards.
fn erfc_continued_fraction(x: f64) -> f64 {
    let mut t = x;
    for k in (1..=60).rev() {
        t = x + (k as f64 / 2.0) / t;
    }
    (-x * x).exp() / (core::f64::consts::PI.sqrt() * t)
}

/// Standard normal quantile by bisection on [`normal_cdf`].
#[must_use]
pub fn normal_ppf(p: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    if (p - 0.5).abs() < 1e-15 {
        return 0.0;
    }
    let (mut lo, mut hi) = (-40.0, 40.0);
    for _ in 0..120 {
        let mid = 0.5 * (lo + hi);
        if normal_cdf(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    #[test]
    fn log_gamma_known_values() {
        assert!(close(ln_gamma(1.0), 0.0, 1e-10));
        assert!(close(ln_gamma(5.0), 24.0f64.ln(), 1e-10)); // Γ(5)=24
        assert!(close(
            ln_gamma(0.5),
            (core::f64::consts::PI.sqrt()).ln(),
            1e-10
        ));
    }

    #[test]
    fn beta_inc_symmetry_and_bounds() {
        assert!(close(beta_inc(2.0, 3.0, 0.0), 0.0, 1e-12));
        assert!(close(beta_inc(2.0, 3.0, 1.0), 1.0, 1e-12));
        // I_x(a,b) + I_{1−x}(b,a) = 1
        let x = 0.3;
        assert!(close(
            beta_inc(2.5, 4.0, x) + beta_inc(4.0, 2.5, 1.0 - x),
            1.0,
            1e-10
        ));
    }

    #[test]
    fn student_t_quantiles_match_tables() {
        // Standard table values.
        assert!(close(student_t_ppf(0.975, 10.0), 2.228_139, 1e-4));
        assert!(close(student_t_ppf(0.95, 30.0), 1.697_261, 1e-4));
        assert!(close(student_t_ppf(0.975, 1.0), 12.706_205, 1e-3)); // Cauchy
                                                                     // Large df → normal.
        assert!(close(student_t_ppf(0.975, 10_000.0), 1.959_964, 1e-3));
        // CDF/quantile round trip.
        let q = student_t_ppf(0.99, 7.0);
        assert!(close(student_t_cdf(q, 7.0), 0.99, 1e-9));
    }

    #[test]
    fn normal_quantiles() {
        assert!(close(normal_ppf(0.975), 1.959_964, 1e-5));
        assert!(close(normal_ppf(0.5), 0.0, 1e-12));
        assert!(close(normal_cdf(1.96), 0.975_002, 1e-4));
    }
}
