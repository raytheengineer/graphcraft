//! L4 format: the `.graphcraft` native file format and CSV import.
//!
//! `.pzfx` is deliberately NOT supported (see `AGENTS.md` › Clean-room).

use serde::{Deserialize, Serialize};

pub const FORMAT_TAG: &str = "graphcraft";
pub const FORMAT_VERSION: u32 = 1;
pub const EXTENSION: &str = "graphcraft";

/// Top-level document stored in a `.graphcraft` file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Document {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub tables: Vec<serde_json::Value>,
    #[serde(default)]
    pub analyses: Vec<serde_json::Value>,
    #[serde(default)]
    pub graphs: Vec<serde_json::Value>,
    #[serde(default)]
    pub layouts: Vec<serde_json::Value>,
}

impl Document {
    pub fn empty() -> Self {
        Self {
            format: FORMAT_TAG.to_string(),
            version: FORMAT_VERSION,
            tables: Vec::new(),
            analyses: Vec::new(),
            graphs: Vec::new(),
            layouts: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("not a graphcraft document")]
    WrongFormat,
    #[error("unsupported format version {0}")]
    UnsupportedVersion(u32),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn parse(bytes: &[u8]) -> Result<Document, FormatError> {
    let doc: Document = serde_json::from_slice(bytes)?;
    if doc.format != FORMAT_TAG {
        return Err(FormatError::WrongFormat);
    }
    if doc.version != FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion(doc.version));
    }
    Ok(doc)
}

pub fn serialize(doc: &Document) -> Result<Vec<u8>, FormatError> {
    Ok(serde_json::to_vec_pretty(doc)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_doc_round_trips() {
        let doc = Document::empty();
        let bytes = serialize(&doc).unwrap();
        let parsed = parse(&bytes).unwrap();
        assert_eq!(doc, parsed);
    }

    #[test]
    fn wrong_format_rejected() {
        let bytes = br#"{"format":"notgraphcraft","version":1}"#;
        assert!(matches!(parse(bytes), Err(FormatError::WrongFormat)));
    }
}
