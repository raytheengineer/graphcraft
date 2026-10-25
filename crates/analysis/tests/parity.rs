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
    // Only inline form is wired for M1: inline:1;2;3
    if let Some(payload) = spec.strip_prefix("inline:") {
        return payload
            .split(';')
            .map(|s| s.parse::<f64>().expect("inline value f64"))
            .collect();
    }
    panic!("unsupported dataset spec: {spec}");
}

fn run(row: &Row) -> f64 {
    let xs = load_dataset(&row.dataset);
    match (row.analysis.as_str(), row.statistic.as_str()) {
        ("mean", "mean") => graphcraft_numeric::mean(&xs).unwrap(),
        ("sum", "sum") => graphcraft_numeric::kahan_sum(&xs).unwrap(),
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
