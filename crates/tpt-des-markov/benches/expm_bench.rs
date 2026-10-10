//! Criterion benchmark for the from-scratch matrix exponential — the hot
//! path of CTMC transient analysis.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tpt_des_markov::ctmc::matrix_exponential;
use tpt_math_linalg_dense::DMatrix;

fn dense_generator(n: usize, seed: u64) -> DMatrix {
    // Deterministic pseudo-random generator matrix: negative diagonal
    // compensating positive off-diagonals.
    let mut data = vec![0.0; n * n];
    let mut s = seed;
    for i in 0..n {
        let mut row_sum = 0.0;
        for j in 0..n {
            if i != j {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let v = ((s >> 33) % 100) as f64 / 1000.0;
                data[i * n + j] = v;
                row_sum += v;
            }
        }
        data[i * n + i] = -row_sum;
    }
    DMatrix::from_row_slice(n, n, &data)
}

fn bench_expm(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_exponential");
    for n in [8usize, 32, 128] {
        let q = dense_generator(n, 42);
        group.bench_function(format!("n={n}"), |b| {
            b.iter(|| matrix_exponential(black_box(&q)))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_expm);
criterion_main!(benches);
