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

use graphcraft_table::{Column, ColumnTable};

use crate::{Document, FormatError, FORMAT_TAG, FORMAT_VERSION};

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
        title: String::new(),
        columns,
    };
    let table_value = serde_json::to_value(&table)?;
    Ok(Document {
        format: FORMAT_TAG.to_string(),
        version: FORMAT_VERSION,
        tables: vec![table_value],
        analyses: Vec::new(),
        graphs: Vec::new(),
        layouts: Vec::new(),
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
        serde_json::from_value(doc.tables[0].clone()).unwrap()
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
