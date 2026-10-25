#![allow(
    clippy::needless_range_loop,
    clippy::unwrap_used,
    clippy::vec_init_then_push
)]
//! Perf-budget harness.
//!
//! Runs a handful of representative workloads with `std::time::Instant`
//! and prints each one's median wall-time over N iterations. The
//! numbers are advisory — CI uses this only as a smoke test (does
//! it run and emit output) because wall-time on hosted runners is
//! too variable for a hard budget. For local benchmarking, the
//! `tests/perf-budgets.md` document records the targets GraphCraft
//! is written to meet on a developer laptop.
//!
//! Workloads:
//! - OLS fit on NIST StRD *Norris* (small-regression baseline).
//! - LM fit on NIST StRD *Misra1a* (small-nonlinear baseline).
//! - Build + render a 1 000-point XY scatter to SVG.
//! - Build + render the same scatter to PDF.
//! - Kruskal-Wallis on 3 × 100 points.
//! - Chi-squared independence on a 10×10 count table.

use std::time::{Duration, Instant};

use graphcraft_plot::{xy_scene_from_points, XyOptions};
use graphcraft_regression::{
    fit_model, levenberg_marquardt, simple_linear_regression, LmOptions, OneSiteBinding,
};

fn main() {
    let mut rows: Vec<(&'static str, Duration)> = Vec::new();
    rows.push(bench("ols/norris", 50, bench_ols_norris));
    rows.push(bench("lm/misra1a", 25, bench_lm_misra1a));
    rows.push(bench("xy/svg_1k", 20, bench_xy_svg_1k));
    rows.push(bench("xy/pdf_1k", 20, bench_xy_pdf_1k));
    rows.push(bench("nonpar/kw_3x100", 20, bench_kw_3x100));
    rows.push(bench("contingency/chi2_10x10", 50, bench_chi2_10x10));
    rows.push(bench(
        "library/one_site_binding_fit",
        20,
        bench_one_site_fit,
    ));

    println!("{:<30}  median wall-time", "workload");
    for (label, d) in &rows {
        println!("{:<30}  {:>7.3} ms", label, d.as_secs_f64() * 1000.0);
    }
}

fn bench(label: &'static str, iters: usize, mut body: impl FnMut()) -> (&'static str, Duration) {
    // Warm up once so JIT caches / allocator stabilise.
    body();
    let mut samples: Vec<Duration> = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t0 = Instant::now();
        body();
        samples.push(t0.elapsed());
    }
    samples.sort();
    #[allow(clippy::integer_division)]
    let median = samples[samples.len() / 2];
    (label, median)
}

// ---------------------------------------------------------------
// Workloads.
// ---------------------------------------------------------------

fn bench_ols_norris() {
    // Hand-coded minimal Norris data (36 obs, same as parity).
    let (xs, ys) = norris_pairs();
    let r = simple_linear_regression(&xs, &ys).unwrap();
    std::hint::black_box(r);
}

fn bench_lm_misra1a() {
    let (xs, ys) = misra1a_pairs();
    let model = |b: &[f64], x: f64| b[0] * (1.0 - (-b[1] * x).exp());
    let r =
        levenberg_marquardt(model, &xs, &ys, &[250.0_f64, 0.0005], LmOptions::default()).unwrap();
    std::hint::black_box(r);
}

fn make_xy_scene() -> graphcraft_plot::Scene {
    let n = 1000;
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let x = i as f64 * 0.01;
            let y = x.sin() + 0.1 * (3.0 * x).cos();
            (x, y)
        })
        .collect();
    xy_scene_from_points(&pts, &XyOptions::default(), "bench".into()).unwrap()
}

fn bench_xy_svg_1k() {
    let scene = make_xy_scene();
    let out = graphcraft_render::render_svg(&scene);
    std::hint::black_box(out);
}

fn bench_xy_pdf_1k() {
    let scene = make_xy_scene();
    let out = graphcraft_render::render_pdf(&scene);
    std::hint::black_box(out);
}

fn bench_kw_3x100() {
    let g1: Vec<f64> = (0..100).map(|i| i as f64).collect();
    let g2: Vec<f64> = (0..100).map(|i| i as f64 + 2.0).collect();
    let g3: Vec<f64> = (0..100).map(|i| i as f64 + 4.0).collect();
    let r = graphcraft_stats::kruskal_wallis(&[g1, g2, g3]).unwrap();
    std::hint::black_box(r);
}

fn bench_chi2_10x10() {
    let mut obs = vec![vec![0_u64; 10]; 10];
    for i in 0..10 {
        for j in 0..10 {
            obs[i][j] = ((i + 1) * (j + 2)) as u64;
        }
    }
    let r = graphcraft_stats::chi_squared_independence(&obs).unwrap();
    std::hint::black_box(r);
}

fn bench_one_site_fit() {
    let b_true = [100.0_f64, 10.0];
    let xs: Vec<f64> = (1..=12).map(|k| k as f64 * 5.0).collect();
    let ys: Vec<f64> = xs
        .iter()
        .map(|&x| b_true[0] * x / (b_true[1] + x))
        .collect();
    let r = fit_model(&OneSiteBinding, &xs, &ys, LmOptions::default()).unwrap();
    std::hint::black_box(r);
}

// Keep the data literals here (tiny, baked in) rather than reading
// files, so the bench cost is pure numeric.

fn norris_pairs() -> (Vec<f64>, Vec<f64>) {
    const DATA: &[(f64, f64)] = &[
        (0.2, 0.1),
        (337.4, 338.8),
        (118.2, 118.1),
        (884.6, 888.0),
        (10.1, 9.2),
        (226.5, 228.1),
        (666.3, 668.5),
        (996.3, 998.5),
        (448.6, 449.1),
        (777.0, 778.9),
        (558.2, 559.2),
        (0.4, 0.3),
        (0.6, 0.1),
        (775.5, 778.1),
        (666.9, 668.8),
        (338.0, 339.3),
        (447.5, 448.9),
        (11.6, 10.8),
        (556.0, 557.7),
        (228.1, 228.3),
        (995.8, 998.0),
        (887.6, 888.8),
        (120.2, 119.6),
        (0.3, 0.3),
        (0.3, 0.6),
        (556.8, 557.6),
        (339.1, 339.3),
        (887.2, 888.0),
        (999.0, 998.5),
        (779.0, 778.9),
        (11.1, 10.8),
        (118.3, 118.1),
        (229.2, 228.3),
        (669.1, 668.5),
        (448.9, 448.6),
        (0.5, 0.0),
    ];
    let xs = DATA.iter().map(|p| p.0).collect();
    let ys = DATA.iter().map(|p| p.1).collect();
    (xs, ys)
}

fn misra1a_pairs() -> (Vec<f64>, Vec<f64>) {
    const DATA: &[(f64, f64)] = &[
        (77.6, 10.07),
        (114.9, 14.73),
        (141.1, 17.94),
        (190.8, 23.93),
        (239.9, 29.61),
        (289.0, 35.18),
        (332.8, 40.02),
        (378.4, 44.82),
        (434.8, 50.76),
        (477.3, 55.05),
        (536.8, 61.01),
        (593.1, 66.40),
        (689.1, 75.47),
        (760.0, 81.78),
    ];
    let xs = DATA.iter().map(|p| p.0).collect();
    let ys = DATA.iter().map(|p| p.1).collect();
    (xs, ys)
}
