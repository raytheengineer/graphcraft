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
