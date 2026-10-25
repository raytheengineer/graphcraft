#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
//! Parity harness. Reads `tests/parity-matrix.csv` from the workspace
//! root and asserts each row.
//!
//! Only two analyses are wired for M1 (`mean`, `sum`), as proof of
//! pattern. Each later milestone adds analyses here with new rows backed
//! by published reference values (NIST StRD, textbook worked examples).

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR is .../crates/analysis
    manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[derive(Debug)]
struct Row {
    analysis: String,
    dataset: String,
    statistic: String,
    expected: f64,
    tol: f64,
    source: String,
}

fn parse_matrix(csv: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for line in csv.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        assert_eq!(parts.len(), 6, "bad row: {line}");
        rows.push(Row {
            analysis: parts[0].to_string(),
            dataset: parts[1].to_string(),
            statistic: parts[2].to_string(),
            expected: parts[3].parse().expect("expected f64"),
            tol: parts[4].parse().expect("tol f64"),
            source: parts[5].to_string(),
        });
    }
    rows
}

fn load_dataset(spec: &str) -> Vec<f64> {
    // Inline toy data:  inline:1;2;3
    if let Some(payload) = spec.strip_prefix("inline:") {
        return payload
            .split(';')
            .map(|s| s.parse::<f64>().expect("inline value f64"))
            .collect();
    }
    // NIST StRD univariate `.dat` file:  nist-univar:tests/data/nist/Lew.dat
    //
    // The StRD univariate `.dat` files carry a long human-readable
    // header followed by a `Data: Y` marker, a dashed separator, and
    // one f64 per non-blank line. We tolerate either `Data:` casing.
    if let Some(relative) = spec.strip_prefix("nist-univar:") {
        let path = workspace_root().join(relative);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        // StRD univariate files use a dashed separator line between the
        // header and the data block. Everything after the first all-`-`
        // line is one f64 per non-blank line.
        let mut found_separator = false;
        let mut xs = Vec::new();
        for line in text.lines() {
            let t = line.trim();
            if !found_separator {
                if !t.is_empty() && t.chars().all(|c| c == '-') {
                    found_separator = true;
                }
                continue;
            }
            if t.is_empty() {
                continue;
            }
            let v: f64 = t.parse().unwrap_or_else(|_| panic!("bad f64: {t:?}"));
            xs.push(v);
        }
        assert!(found_separator, "no dashed separator in {}", path.display());
        return xs;
    }
    panic!("unsupported dataset spec: {spec}");
}

fn load_synthetic(spec: &str) -> (Vec<f64>, Vec<f64>, std::collections::BTreeMap<String, f64>) {
    // `synthetic:p1=v1;p2=v2;xs=lo~hi~n` — build xs linearly from
    // lo to hi with n points, compute ys exactly from the model.
    let payload = spec
        .strip_prefix("synthetic:")
        .unwrap_or_else(|| panic!("not a synthetic spec: {spec}"));
    let mut params = std::collections::BTreeMap::new();
    let mut xs_spec: Option<String> = None;
    for pair in payload.split(';') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == "xs" {
                xs_spec = Some(v.to_string());
                continue;
            }
            let v: f64 = v
                .parse()
                .unwrap_or_else(|_| panic!("synthetic param {k} is not f64: {v:?}"));
            params.insert(k.to_string(), v);
        }
    }
    let xs_text = xs_spec.unwrap_or_else(|| panic!("synthetic spec needs xs=lo~hi~n: {spec}"));
    let parts: Vec<&str> = xs_text.split('~').collect();
    assert_eq!(parts.len(), 3, "xs spec is lo~hi~n: {xs_text}");
    let lo: f64 = parts[0].parse().unwrap();
    let hi: f64 = parts[1].parse().unwrap();
    let n: usize = parts[2].parse().unwrap();
    assert!(n >= 2, "xs n must be >= 2");
    let step = (hi - lo) / (n as f64 - 1.0);
    let xs: Vec<f64> = (0..n).map(|i| lo + i as f64 * step).collect();
    // ys is caller-filled; set to zero here, the model dispatcher
    // computes exact values from params.
    let ys = vec![0.0_f64; n];
    (xs, ys, params)
}

fn library_param_names(id: &str) -> &'static [&'static str] {
    use graphcraft_regression::NonlinearModel;
    match id {
        "one-site-binding" => graphcraft_regression::OneSiteBinding.param_names(),
        "hill-binding" => graphcraft_regression::HillBinding.param_names(),
        "exp-decay-one-phase" => graphcraft_regression::ExpDecayOnePhase.param_names(),
        "exp-growth" => graphcraft_regression::ExpGrowth.param_names(),
        "log-dose-response" => graphcraft_regression::LogDoseResponse.param_names(),
        "line" => graphcraft_regression::Line.param_names(),
        other => panic!("unknown library model id: {other}"),
    }
}

fn library_predict(id: &str, params: &[f64], x: f64) -> f64 {
    use graphcraft_regression::NonlinearModel;
    match id {
        "one-site-binding" => graphcraft_regression::OneSiteBinding.predict(params, x),
        "hill-binding" => graphcraft_regression::HillBinding.predict(params, x),
        "exp-decay-one-phase" => graphcraft_regression::ExpDecayOnePhase.predict(params, x),
        "exp-growth" => graphcraft_regression::ExpGrowth.predict(params, x),
        "log-dose-response" => graphcraft_regression::LogDoseResponse.predict(params, x),
        "line" => graphcraft_regression::Line.predict(params, x),
        other => panic!("unknown library model id: {other}"),
    }
}

fn fit_library(id: &str, xs: &[f64], ys: &[f64]) -> graphcraft_regression::LmFit {
    use graphcraft_regression::{fit_model, LmOptions};
    // Library parity rows are closed-form synthetic data; RSS at
    // the solution is essentially zero. The default LM tolerances
    // (ftol=1e-12 relative) become `≤ 1e-24` as `S → 0`, so loosen
    // the stopping rule to a modest absolute floor that still
    // guarantees the published 1e-6-ish parity.
    let opts = LmOptions {
        max_iter: 2000,
        ftol: 1e-20,
        ptol: 1e-14,
        ..LmOptions::default()
    };
    match id {
        "one-site-binding" => fit_model(&graphcraft_regression::OneSiteBinding, xs, ys, opts),
        "hill-binding" => fit_model(&graphcraft_regression::HillBinding, xs, ys, opts),
        "exp-decay-one-phase" => fit_model(&graphcraft_regression::ExpDecayOnePhase, xs, ys, opts),
        "exp-growth" => fit_model(&graphcraft_regression::ExpGrowth, xs, ys, opts),
        "log-dose-response" => fit_model(&graphcraft_regression::LogDoseResponse, xs, ys, opts),
        "line" => fit_model(&graphcraft_regression::Line, xs, ys, opts),
        other => panic!("unknown library model id: {other}"),
    }
    .unwrap_or_else(|e| panic!("library fit {id} failed: {e:?}"))
}

fn load_contingency(spec: &str) -> Vec<Vec<u64>> {
    // `contingency:a;b;c||d;e;f||...` — rows separated by `||`,
    // cells by `;`.
    let payload = spec
        .strip_prefix("contingency:")
        .unwrap_or_else(|| panic!("not a contingency spec: {spec}"));
    payload
        .split("||")
        .map(|row| {
            row.split(';')
                .map(|t| t.parse::<u64>().expect("contingency cell u64"))
                .collect()
        })
        .collect()
}

fn load_regression_pair(spec: &str) -> (Vec<f64>, Vec<f64>) {
    // NIST StRD regression (y, x) pairs:  nist-regr:tests/data/nist/Norris.dat
    //
    // The LLS .dat files declare the data block in the header as
    // "Data: y x". The data begins after a dashed separator, with one
    // whitespace-delimited (y, x) pair per non-blank line.
    if let Some(relative) = spec.strip_prefix("nist-regr:") {
        let path = workspace_root().join(relative);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        let mut in_data = false;
        for line in text.lines() {
            let t = line.trim();
            if !in_data {
                if t.starts_with("Data:") && t.contains('y') && t.contains('x') {
                    in_data = true;
                }
                continue;
            }
            if t.is_empty() {
                continue;
            }
            let parts: Vec<&str> = t.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let y: f64 = parts[0].parse().unwrap_or_else(|_| panic!("bad y: {t}"));
            let x: f64 = parts[1].parse().unwrap_or_else(|_| panic!("bad x: {t}"));
            xs.push(x);
            ys.push(y);
        }
        assert!(in_data, "no `Data: y x` marker in {}", path.display());
        return (xs, ys);
    }
    panic!("unsupported regression dataset: {spec}");
}

fn load_dataset_pair(spec: &str) -> (Vec<f64>, Vec<f64>) {
    // `inline2:a1;a2;...|b1;b2;...` — two inline sequences.
    let payload = spec
        .strip_prefix("inline2:")
        .unwrap_or_else(|| panic!("not an inline2 spec: {spec}"));
    let (left, right) = payload
        .split_once('|')
        .unwrap_or_else(|| panic!("inline2 needs a `|`: {spec}"));
    let parse = |s: &str| -> Vec<f64> {
        s.split(';')
            .map(|t| t.parse::<f64>().expect("inline2 value f64"))
            .collect()
    };
    (parse(left), parse(right))
}

fn load_dataset_groups(spec: &str) -> Vec<Vec<f64>> {
    // `inline-groups:a;b||c;d||e;f` — k groups separated by `||`.
    let payload = spec
        .strip_prefix("inline-groups:")
        .unwrap_or_else(|| panic!("not an inline-groups spec: {spec}"));
    payload
        .split("||")
        .map(|g| {
            g.split(';')
                .map(|t| t.parse::<f64>().expect("group value f64"))
                .collect()
        })
        .collect()
}

fn parse_params(spec: &str) -> std::collections::BTreeMap<String, f64> {
    let mut out = std::collections::BTreeMap::new();
    for pair in spec.split(';') {
        if let Some((k, v)) = pair.split_once('=') {
            let v: f64 = v
                .parse()
                .unwrap_or_else(|_| panic!("param {k:?} is not f64: {v:?}"));
            out.insert(k.to_string(), v);
        }
    }
    out
}

fn run(row: &Row) -> f64 {
    // Analyses that take parameter maps rather than a dataset slice.
    if row.analysis == "t-quantile" {
        let p = parse_params(&row.dataset);
        let df = *p.get("df").expect("t-quantile needs df");
        let pr = *p.get("p").expect("t-quantile needs p");
        return graphcraft_stats::distributions::t::t_inv_cdf(pr, df).unwrap();
    }
    // Two-sample analyses: dataset is `inline2:...|...`.
    if row.analysis == "welch-t" || row.analysis == "pooled-t" {
        let (xs, ys) = load_dataset_pair(&row.dataset);
        let kind = if row.analysis == "welch-t" {
            graphcraft_stats::TwoSampleKind::Welch
        } else {
            graphcraft_stats::TwoSampleKind::Pooled
        };
        let r = graphcraft_stats::unpaired_t(&xs, &ys, kind).unwrap();
        return match row.statistic.as_str() {
            "t" => r.t,
            "df" => r.df,
            "mean_diff" => r.mean_diff,
            "p" => r.p_two_sided,
            other => panic!("unknown two-sample statistic: {other:?}"),
        };
    }
    if let Some(model_id) = row.analysis.strip_prefix("library-fit:") {
        let (xs, _placeholder, params_map) = load_synthetic(&row.dataset);
        let names = library_param_names(model_id);
        let true_params: Vec<f64> = names
            .iter()
            .map(|n| {
                *params_map
                    .get(*n)
                    .unwrap_or_else(|| panic!("missing synthetic param {n} for {model_id}"))
            })
            .collect();
        let ys: Vec<f64> = xs
            .iter()
            .map(|&x| library_predict(model_id, &true_params, x))
            .collect();
        let fit = fit_library(model_id, &xs, &ys);
        // Formal `converged` can be false even when the parameters
        // are within double precision of truth (RSS → 0 defeats
        // the relative-ΔS stopping rule). The row-level tolerance
        // check is the real parity oracle.
        let idx = names
            .iter()
            .position(|n| *n == row.statistic)
            .unwrap_or_else(|| panic!("unknown param {:?} for {model_id}", row.statistic));
        return fit.params[idx];
    }
    if row.analysis == "misra1a" {
        // NIST StRD nonlinear benchmark Misra1a:
        //   y = b1 · (1 − exp(−b2 · x))
        // Start 2 from the dataset (b1 = 250, b2 = 0.0005).
        let (xs, ys) = load_regression_pair(&row.dataset);
        let model = |b: &[f64], x: f64| b[0] * (1.0 - (-b[1] * x).exp());
        let r = graphcraft_regression::levenberg_marquardt(
            model,
            &xs,
            &ys,
            &[250.0_f64, 0.0005],
            graphcraft_regression::LmOptions::default(),
        )
        .unwrap();
        assert!(r.converged, "misra1a LM did not converge");
        return match row.statistic.as_str() {
            "b1" => r.params[0],
            "b2" => r.params[1],
            "rss" => r.rss,
            "residual_se" => r.residual_se,
            "se_b1" => r.se[0],
            "se_b2" => r.se[1],
            other => panic!("unknown misra1a statistic: {other:?}"),
        };
    }
    if row.analysis == "simple-linear-regression" {
        let (xs, ys) = load_regression_pair(&row.dataset);
        let r = graphcraft_regression::simple_linear_regression(&xs, &ys).unwrap();
        return match row.statistic.as_str() {
            "b0" => r.coefficients[0],
            "b1" => r.coefficients[1],
            "se_b0" => r.se[0],
            "se_b1" => r.se[1],
            "r_squared" => r.r_squared,
            "residual_se" => r.residual_se,
            "f_stat" => r.f_stat,
            other => panic!("unknown simple-linear-regression statistic: {other:?}"),
        };
    }
    if row.analysis == "chi2-cdf" {
        let p = parse_params(&row.dataset);
        let x = *p.get("x").expect("chi2-cdf needs x");
        let df = *p.get("df").expect("chi2-cdf needs df");
        return graphcraft_stats::distributions::chi_squared::chi2_cdf(x, df).unwrap();
    }
    if row.analysis == "chi2-inv" {
        let p = parse_params(&row.dataset);
        let pr = *p.get("p").expect("chi2-inv needs p");
        let df = *p.get("df").expect("chi2-inv needs df");
        return graphcraft_stats::distributions::chi_squared::chi2_inv_cdf(pr, df).unwrap();
    }
    if row.analysis == "mann-whitney" {
        let (xs, ys) = load_dataset_pair(&row.dataset);
        let r = graphcraft_stats::mann_whitney(&xs, &ys).unwrap();
        return match row.statistic.as_str() {
            "u" => r.u,
            "z" => r.z,
            "p" => r.p_two_sided,
            "r1" => r.r1,
            other => panic!("unknown mann-whitney statistic: {other:?}"),
        };
    }
    if row.analysis == "wilcoxon-signed-rank" {
        let (xs, ys) = load_dataset_pair(&row.dataset);
        let r = graphcraft_stats::wilcoxon_signed_rank(&xs, &ys).unwrap();
        return match row.statistic.as_str() {
            "z" => r.z,
            "p" => r.p_two_sided,
            "w_plus" => r.w_plus,
            "w_minus" => r.w_minus,
            other => panic!("unknown wilcoxon statistic: {other:?}"),
        };
    }
    if row.analysis == "kruskal-wallis" {
        let groups = load_dataset_groups(&row.dataset);
        let r = graphcraft_stats::kruskal_wallis(&groups).unwrap();
        return match row.statistic.as_str() {
            "h" => r.h,
            "h_corrected" => r.h_corrected,
            "p" => r.p,
            "df" => r.df,
            other => panic!("unknown kruskal-wallis statistic: {other:?}"),
        };
    }
    if row.analysis == "pearson" {
        let (xs, ys) = load_dataset_pair(&row.dataset);
        let r = graphcraft_stats::pearson(&xs, &ys).unwrap();
        return match row.statistic.as_str() {
            "r" => r.r,
            "t" => r.t,
            "p" => r.p_two_sided,
            other => panic!("unknown pearson statistic: {other:?}"),
        };
    }
    if row.analysis == "spearman" {
        let (xs, ys) = load_dataset_pair(&row.dataset);
        let r = graphcraft_stats::spearman(&xs, &ys).unwrap();
        return match row.statistic.as_str() {
            "r" => r.r,
            "p" => r.p_two_sided,
            other => panic!("unknown spearman statistic: {other:?}"),
        };
    }
    if row.analysis == "chi2-indep" {
        let obs = load_contingency(&row.dataset);
        let r = graphcraft_stats::chi_squared_independence(&obs).unwrap();
        return match row.statistic.as_str() {
            "chi2" => r.chi2,
            "df" => r.df,
            "p" => r.p,
            other => panic!("unknown chi2-indep statistic: {other:?}"),
        };
    }
    if row.analysis == "fisher-exact" {
        let obs = load_contingency(&row.dataset);
        assert_eq!(obs.len(), 2, "fisher-exact needs 2 rows");
        assert_eq!(obs[0].len(), 2, "fisher-exact needs 2 cols");
        let r =
            graphcraft_stats::fisher_exact_2x2(obs[0][0], obs[0][1], obs[1][0], obs[1][1]).unwrap();
        return match row.statistic.as_str() {
            "p_two_sided" => r.p_two_sided,
            "p_one_sided" => r.p_one_sided,
            "odds_ratio" => r.odds_ratio,
            other => panic!("unknown fisher-exact statistic: {other:?}"),
        };
    }
    if row.analysis == "normal-cdf" {
        let p = parse_params(&row.dataset);
        let x = *p.get("x").expect("normal-cdf needs x");
        return graphcraft_stats::distributions::normal::big_phi(x);
    }
    if row.analysis == "dunnett-one-sided-cdf" || row.analysis == "dunnett-two-sided-cdf" {
        let p = parse_params(&row.dataset);
        let d = *p.get("d").expect("dunnett needs d");
        let m = *p.get("m").expect("dunnett needs m") as usize;
        let df = *p.get("df").expect("dunnett needs df");
        let side = if row.analysis == "dunnett-one-sided-cdf" {
            graphcraft_stats::DunnettSide::OneSided
        } else {
            graphcraft_stats::DunnettSide::TwoSided
        };
        return graphcraft_stats::distributions::dunnett::dunnett_cdf(d, m, df, side).unwrap();
    }
    if row.analysis == "studentized-range-cdf" {
        let p = parse_params(&row.dataset);
        let q = *p.get("q").expect("studentized-range-cdf needs q");
        let k = *p.get("k").expect("studentized-range-cdf needs k") as usize;
        let df = *p.get("df").expect("studentized-range-cdf needs df");
        return graphcraft_stats::distributions::studentized_range::q_cdf(q, k, df).unwrap();
    }
    if row.analysis == "f-cdf" {
        let p = parse_params(&row.dataset);
        let d1 = *p.get("d1").expect("f-cdf needs d1");
        let d2 = *p.get("d2").expect("f-cdf needs d2");
        let x = *p.get("x").expect("f-cdf needs x");
        return graphcraft_stats::distributions::f::f_cdf(x, d1, d2).unwrap();
    }
    if row.analysis == "bonferroni" {
        let p = parse_params(&row.dataset);
        let raw = *p.get("p").expect("bonferroni needs p");
        let m = *p.get("m").expect("bonferroni needs m") as usize;
        return graphcraft_stats::bonferroni(raw, m).unwrap();
    }
    if row.analysis == "sidak" {
        let p = parse_params(&row.dataset);
        let raw = *p.get("p").expect("sidak needs p");
        let m = *p.get("m").expect("sidak needs m") as usize;
        return graphcraft_stats::sidak(raw, m).unwrap();
    }
    if row.analysis == "one-way-anova" {
        let groups = load_dataset_groups(&row.dataset);
        let r = graphcraft_stats::one_way_anova(&groups).unwrap();
        return match row.statistic.as_str() {
            "f" => r.f,
            "p" => r.p,
            "ss_between" => r.ss_between,
            "ss_within" => r.ss_within,
            "ss_total" => r.ss_total,
            "df_between" => r.df_between,
            "df_within" => r.df_within,
            other => panic!("unknown anova statistic: {other:?}"),
        };
    }
    if row.analysis == "paired-t" {
        let (xs, ys) = load_dataset_pair(&row.dataset);
        let r = graphcraft_stats::paired_t(&xs, &ys).unwrap();
        return match row.statistic.as_str() {
            "t" => r.t,
            "df" => r.df,
            "mean_diff" => r.mean_diff,
            "p" => r.p_two_sided,
            other => panic!("unknown paired statistic: {other:?}"),
        };
    }

    let xs = load_dataset(&row.dataset);
    match (row.analysis.as_str(), row.statistic.as_str()) {
        ("mean", "mean") => graphcraft_numeric::mean(&xs).unwrap(),
        ("sum", "sum") => graphcraft_numeric::kahan_sum(&xs).unwrap(),
        ("descriptives", stat) => {
            let d = graphcraft_stats::Descriptives::of(&xs).unwrap();
            match stat {
                "n" => d.n as f64,
                "mean" => d.mean,
                "median" => d.median,
                "min" => d.min,
                "max" => d.max,
                "sd" => d.sd.expect("sd requires n>=2"),
                "variance" => d.variance.expect("variance requires n>=2"),
                "sem" => d.sem.expect("sem requires n>=2"),
                "range" => d.range(),
                other => panic!("unknown descriptive statistic: {other:?}"),
            }
        }
        other => panic!("unimplemented analysis: {other:?}"),
    }
}

#[test]
fn parity_matrix() {
    let path = workspace_root().join("tests").join("parity-matrix.csv");
    let csv =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let rows = parse_matrix(&csv);
    assert!(!rows.is_empty(), "parity matrix is empty");
    for row in &rows {
        let got = run(row);
        let diff = (got - row.expected).abs();
        let scale = row.expected.abs().max(1.0);
        let rel = diff / scale;
        assert!(
            rel <= row.tol,
            "{} / {} / {}: got {got}, expected {} (rel {rel:e} > tol {:e}); source {}",
            row.analysis,
            row.dataset,
            row.statistic,
            row.expected,
            row.tol,
            row.source,
        );
    }
}
