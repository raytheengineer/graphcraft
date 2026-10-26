//! Minimal RFC-4180-ish CSV importer.
//!
//! Scope: comma-separated, UTF-8, first row is the header, LF or CRLF
//! line endings, double-quote-escaped fields with `""` for a literal
//! quote. Numeric cells parse as `f64`; empty cells become `None`
//! (missing). Anything else is an error carrying the (1-indexed) line
//! and column of the offending cell.
//!
//! This is deliberately not a full CSV library — we do not try to be
//! lenient. If users bring more exotic inputs we add a dep later; for
//! now the small clean implementation wins.
//!
//! Input is capped at 64 MiB to bound allocation on untrusted input.

use graphcraft_table::{
    AnyTable, Column, ColumnTable, ContingencyTable, GroupColumn, GroupedTable, SurvivalGroup,
    SurvivalRow, SurvivalTable, XyTable, YDataset, YFormat,
};

use crate::{Document, FormatError, Project};

const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;

/// Parse CSV bytes into a single-table `.graphcraft` [`Document`].
pub fn import_csv(bytes: &[u8]) -> Result<Document, FormatError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(FormatError::CsvTooLarge(bytes.len()));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| FormatError::CsvNotUtf8)?;
    let mut rows = parse_rows(text)?;
    if rows.is_empty() {
        return Err(FormatError::CsvEmpty);
    }
    let header = rows.remove(0);
    let mut columns: Vec<Column> = header.into_iter().map(|t| Column::new(t.trim())).collect();
    for (row_idx, row) in rows.into_iter().enumerate() {
        let line = row_idx + 2;
        if row.len() > columns.len() {
            return Err(FormatError::CsvRowTooLong {
                line,
                got: row.len(),
                header: columns.len(),
            });
        }
        for (col_idx, cell) in row.iter().enumerate() {
            let trimmed = cell.trim();
            let value = if trimmed.is_empty() {
                None
            } else {
                Some(
                    trimmed
                        .parse::<f64>()
                        .map_err(|_| FormatError::CsvNotNumeric {
                            line,
                            column: col_idx + 1,
                            cell: trimmed.to_string(),
                        })?,
                )
            };
            if let Some(col) = columns.get_mut(col_idx) {
                col.values.push(value);
            }
        }
        for col in columns.iter_mut().skip(row.len()) {
            col.values.push(None);
        }
    }

    let table = ColumnTable {
        title: "Imported CSV".into(),
        row_titles: Vec::new(),
        columns,
    };
    // A fresh CSV import becomes a single-sheet project. The CLI
    // and web adapters already treat this document shape as a
    // one-table carrier; the project model preserves that.
    let mut project = Project::new("Imported CSV");
    project.add_data_sheet("Imported CSV", table.into());
    Ok(Document::from_project(project))
}

/// Parse a CSV like [`import_csv`] but produce a column table
/// directly, with no `Project` wrapper. Useful when the caller is
/// about to convert to another table shape (e.g. [`csv_to_xy`]) or
/// append to an existing project.
pub fn parse_csv_to_column_table(bytes: &[u8]) -> Result<ColumnTable, FormatError> {
    let doc = import_csv(bytes)?;
    match doc.project.sheets.into_iter().next().map(|s| s.body) {
        Some(crate::SheetBody::Data {
            table: AnyTable::Column(t),
        }) => Ok(t),
        _ => Err(FormatError::CsvEmpty),
    }
}

/// Reshape a parsed CSV into an `XyTable`: the first column of
/// `t` becomes the shared x; every remaining column becomes a
/// `SingleY` dataset. Returns `Err(CsvEmpty)` on fewer than two
/// columns.
pub fn csv_to_xy(t: ColumnTable) -> Result<XyTable, FormatError> {
    if t.columns.len() < 2 {
        return Err(FormatError::CsvEmpty);
    }
    let mut iter = t.columns.into_iter();
    // `len >= 2` was checked above; the first `next()` is guaranteed Some.
    let x_col = match iter.next() {
        Some(c) => c,
        None => return Err(FormatError::CsvEmpty),
    };
    let datasets = iter
        .map(|col| YDataset {
            title: col.title,
            format: YFormat::SingleY,
            subcolumns: vec![Column {
                title: String::new(),
                values: col.values,
            }],
        })
        .collect();
    Ok(XyTable {
        title: t.title,
        x_title: x_col.title,
        x: x_col.values,
        datasets,
    })
}

/// Reshape a parsed CSV into a `GroupedTable`. Columns whose
/// headers share a prefix before the first `.` go into the same
/// group as replicate subcolumns (so `A.1, A.2, B.1` → group `A`
/// with two replicates and group `B` with one). Columns without
/// a `.` form a group of one replicate each. Order follows
/// first-appearance in the header. Returns `Err(CsvEmpty)` on an
/// empty table.
pub fn csv_to_grouped(t: ColumnTable) -> Result<GroupedTable, FormatError> {
    if t.columns.is_empty() {
        return Err(FormatError::CsvEmpty);
    }
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<Column>> =
        std::collections::HashMap::new();
    for col in t.columns {
        let (group, sub) = match col.title.split_once('.') {
            Some((g, _)) => (g.trim().to_string(), col),
            None => (col.title.trim().to_string(), col),
        };
        if !groups.contains_key(&group) {
            order.push(group.clone());
        }
        groups.entry(group).or_default().push(sub);
    }
    let mut out = Vec::with_capacity(order.len());
    for title in order {
        if let Some(subs) = groups.remove(&title) {
            out.push(GroupColumn {
                title,
                subcolumns: subs,
            });
        }
    }
    Ok(GroupedTable {
        title: t.title,
        row_titles: t.row_titles,
        groups: out,
    })
}

/// Reshape a parsed CSV into a `SurvivalTable`. The CSV must have
/// an even number of columns, paired `<group>_time, <group>_event`
/// (or any two consecutive columns, in which case the group title
/// is taken from the time column with any trailing `_time` /
/// `_t` / ` time` stripped). `event` values are read as 1 =
/// event, 0 = censored; a missing cell is treated as censored.
/// Returns `Err(CsvEmpty)` on fewer than two columns or an odd
/// column count.
pub fn csv_to_survival(t: ColumnTable) -> Result<SurvivalTable, FormatError> {
    if t.columns.len() < 2 || t.columns.len() % 2 != 0 {
        return Err(FormatError::CsvEmpty);
    }
    let mut time_title = String::new();
    let mut groups: Vec<SurvivalGroup> = Vec::new();
    for pair in t.columns.chunks(2) {
        let time_col = &pair[0];
        let event_col = &pair[1];
        let name = time_col
            .title
            .trim_end_matches("_time")
            .trim_end_matches("_t")
            .trim_end_matches(" time")
            .to_string();
        if time_title.is_empty() {
            time_title = "time".to_string();
        }
        let n = time_col.values.len().max(event_col.values.len());
        let mut rows = Vec::with_capacity(n);
        for i in 0..n {
            let time = match time_col.values.get(i).and_then(|v| *v) {
                Some(t) if t.is_finite() => t,
                _ => continue,
            };
            let event = event_col
                .values
                .get(i)
                .and_then(|v| *v)
                .map(|e| e != 0.0)
                .unwrap_or(false);
            rows.push(SurvivalRow {
                time,
                censored: !event,
            });
        }
        groups.push(SurvivalGroup { title: name, rows });
    }
    Ok(SurvivalTable {
        title: t.title,
        time_title,
        groups,
    })
}

/// Reshape a parsed CSV into a `ContingencyTable`. Every data
/// cell must be a non-negative integer; `None` is read as `0`.
/// Returns `Err(CsvEmpty)` on an empty table or
/// `Err(CsvNotNumeric { .. })` on a negative / non-integer cell.
pub fn csv_to_contingency(t: ColumnTable) -> Result<ContingencyTable, FormatError> {
    if t.columns.is_empty() {
        return Err(FormatError::CsvEmpty);
    }
    let height = t.columns.iter().map(|c| c.values.len()).max().unwrap_or(0);
    let column_titles: Vec<String> = t.columns.iter().map(|c| c.title.clone()).collect();
    let mut counts: Vec<Vec<u64>> = Vec::with_capacity(height);
    for r in 0..height {
        let mut row = Vec::with_capacity(t.columns.len());
        for (col_idx, c) in t.columns.iter().enumerate() {
            let v = c.values.get(r).and_then(|v| *v).unwrap_or(0.0);
            if !v.is_finite() || v < 0.0 || v.fract() != 0.0 {
                return Err(FormatError::CsvNotNumeric {
                    line: r + 2,
                    column: col_idx + 1,
                    cell: format!("{v}"),
                });
            }
            row.push(v as u64);
        }
        counts.push(row);
    }
    Ok(ContingencyTable {
        title: t.title,
        row_titles: t.row_titles,
        column_titles,
        counts,
    })
}

fn parse_rows(text: &str) -> Result<Vec<Vec<String>>, FormatError> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut in_quotes = false;
    let mut line = 1usize;

    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cell.push(c);
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            ',' => {
                row.push(std::mem::take(&mut cell));
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
                line += 1;
            }
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
                line += 1;
            }
            _ => cell.push(c),
        }
    }
    if in_quotes {
        return Err(FormatError::CsvUnterminatedQuote { line });
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    // Drop trailing all-empty row that a final newline produces.
    if rows
        .last()
        .is_some_and(|r| r.iter().all(|c| c.is_empty()) && r.len() <= 1)
    {
        rows.pop();
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_of(doc: &Document) -> ColumnTable {
        match &doc.project.sheets[0].body {
            crate::SheetBody::Data {
                table: graphcraft_table::AnyTable::Column(c),
            } => c.clone(),
            other => panic!("expected Column data sheet, got {other:?}"),
        }
    }

    #[test]
    fn two_columns_three_rows() {
        let csv = b"a,b\n1,2\n3,4\n5,6\n";
        let doc = import_csv(csv).unwrap();
        let t = table_of(&doc);
        assert_eq!(t.columns.len(), 2);
        assert_eq!(t.columns[0].title, "a");
        assert_eq!(t.columns[1].title, "b");
        assert_eq!(t.columns[0].values, vec![Some(1.0), Some(3.0), Some(5.0)]);
        assert_eq!(t.columns[1].values, vec![Some(2.0), Some(4.0), Some(6.0)]);
    }

    #[test]
    fn missing_cell_is_none() {
        let csv = b"a,b\n1,\n,2\n";
        let t = table_of(&import_csv(csv).unwrap());
        assert_eq!(t.columns[0].values, vec![Some(1.0), None]);
        assert_eq!(t.columns[1].values, vec![None, Some(2.0)]);
    }

    #[test]
    fn short_row_pads_with_none() {
        let csv = b"a,b,c\n1,2\n";
        let t = table_of(&import_csv(csv).unwrap());
        assert_eq!(t.columns[0].values, vec![Some(1.0)]);
        assert_eq!(t.columns[1].values, vec![Some(2.0)]);
        assert_eq!(t.columns[2].values, vec![None]);
    }

    #[test]
    fn crlf_line_endings() {
        let csv = b"a,b\r\n1,2\r\n3,4\r\n";
        let t = table_of(&import_csv(csv).unwrap());
        assert_eq!(t.height(), 2);
    }

    #[test]
    fn quoted_header_with_comma() {
        let csv = b"\"Dose, mg\",response\n1,10\n";
        let t = table_of(&import_csv(csv).unwrap());
        assert_eq!(t.columns[0].title, "Dose, mg");
        assert_eq!(t.columns[0].values, vec![Some(1.0)]);
    }

    #[test]
    fn escaped_quote_in_header() {
        let csv = b"\"x\"\"y\",b\n1,2\n";
        let t = table_of(&import_csv(csv).unwrap());
        assert_eq!(t.columns[0].title, "x\"y");
    }

    #[test]
    fn non_numeric_rejected() {
        let csv = b"a,b\n1,oops\n";
        let err = import_csv(csv).unwrap_err();
        match err {
            FormatError::CsvNotNumeric { line, column, cell } => {
                assert_eq!(line, 2);
                assert_eq!(column, 2);
                assert_eq!(cell, "oops");
            }
            other => panic!("wrong error: {other:?}"),
        }
    }

    #[test]
    fn too_many_cells_rejected() {
        let csv = b"a,b\n1,2,3\n";
        assert!(matches!(
            import_csv(csv).unwrap_err(),
            FormatError::CsvRowTooLong { .. }
        ));
    }

    #[test]
    fn unterminated_quote_rejected() {
        let csv = b"a,b\n\"unterminated,2\n";
        assert!(matches!(
            import_csv(csv).unwrap_err(),
            FormatError::CsvUnterminatedQuote { .. }
        ));
    }

    #[test]
    fn empty_input_rejected() {
        assert!(matches!(
            import_csv(b"").unwrap_err(),
            FormatError::CsvEmpty
        ));
    }

    #[test]
    fn grouped_prefix_before_dot() {
        let csv = b"A.1,A.2,B.1\n1,2,10\n3,4,30\n";
        let t = parse_csv_to_column_table(csv).unwrap();
        let g = csv_to_grouped(t).unwrap();
        assert_eq!(g.groups.len(), 2);
        assert_eq!(g.groups[0].title, "A");
        assert_eq!(g.groups[0].replicates(), 2);
        assert_eq!(g.groups[1].title, "B");
        assert_eq!(g.groups[1].replicates(), 1);
    }

    #[test]
    fn survival_pairs_time_event() {
        let csv = b"Arm1_time,Arm1_event,Arm2_time,Arm2_event\n5,1,8,0\n10,0,12,1\n";
        let t = parse_csv_to_column_table(csv).unwrap();
        let s = csv_to_survival(t).unwrap();
        assert_eq!(s.groups.len(), 2);
        assert_eq!(s.groups[0].title, "Arm1");
        assert_eq!(s.groups[0].rows[0].time, 5.0);
        assert!(!s.groups[0].rows[0].censored); // event=1 → not censored
        assert!(s.groups[0].rows[1].censored); // event=0 → censored
        assert_eq!(s.groups[1].rows[1].time, 12.0);
    }

    #[test]
    fn contingency_integer_cells() {
        let csv = b"placebo,treatment\n12,25\n30,8\n";
        let t = parse_csv_to_column_table(csv).unwrap();
        let c = csv_to_contingency(t).unwrap();
        assert_eq!(c.rows(), 2);
        assert_eq!(c.cols(), 2);
        assert_eq!(c.cell(0, 0), Some(12));
        assert_eq!(c.cell(1, 1), Some(8));
        assert_eq!(c.column_titles, vec!["placebo", "treatment"]);
    }

    #[test]
    fn contingency_rejects_fractional() {
        let csv = b"a,b\n1.5,2\n";
        let t = parse_csv_to_column_table(csv).unwrap();
        assert!(matches!(
            csv_to_contingency(t).unwrap_err(),
            FormatError::CsvNotNumeric { .. }
        ));
    }

    #[test]
    fn too_large_rejected() {
        // Fabricate a slice that lies about its length? No — we just
        // test the guard with a vec of maximum-plus-one zero bytes.
        let big = vec![b' '; MAX_INPUT_BYTES + 1];
        assert!(matches!(
            import_csv(&big).unwrap_err(),
            FormatError::CsvTooLarge(_)
        ));
    }
}
