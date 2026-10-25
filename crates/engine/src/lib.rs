//! L5 engine: commands, undo, document, panic guard.
//!
//! Public surface for M1:
//!   - [`Document`] re-exported from `graphcraft_format`
//!   - [`CommandRegistry`] with one trivial command (`doc.touch`)
//!   - [`guard`] catching panics and reporting them as `EngineError::Panic`.

pub mod cmd;
pub mod guard;

pub use graphcraft_format::Document;

use std::collections::BTreeMap;

/// Error type reported to the host (control channel, MCP, UI).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("command refused: {0}")]
    Refused(String),
    #[error("internal error (panic caught): {0}")]
    Panic(String),
    #[error(transparent)]
    Format(#[from] graphcraft_format::FormatError),
}

/// Mutable session state. Grows in later milestones.
pub struct Session {
    pub document: Document,
    pub dirty: bool,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            document: Document::empty(),
            dirty: false,
        }
    }
}

/// A command's run function. Takes the session, returns a JSON result.
pub type RunFn = fn(&mut Session, &serde_json::Value) -> Result<serde_json::Value, EngineError>;

/// Command metadata + runner. Mirrors the sibling-project convention.
pub struct Command {
    pub id: &'static str,
    pub label: &'static str,
    pub menu_path: &'static str,
    pub run: RunFn,
}

/// Compile-time registry of all engine commands.
pub struct CommandRegistry {
    by_id: BTreeMap<&'static str, &'static Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut by_id = BTreeMap::new();
        for c in cmd::ALL {
            by_id.insert(c.id, *c);
        }
        Self { by_id }
    }

    pub fn ids(&self) -> impl Iterator<Item = &&'static str> {
        self.by_id.keys()
    }

    pub fn get(&self, id: &str) -> Option<&Command> {
        self.by_id.get(id).copied()
    }

    /// Dispatch a command id. Any panic in the command's `run` is caught by
    /// `guard::call` and surfaced as `EngineError::Panic`.
    pub fn run(
        &self,
        session: &mut Session,
        id: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, EngineError> {
        let Some(cmd) = self.get(id) else {
            return Err(EngineError::UnknownCommand(id.to_string()));
        };
        guard::call(|| (cmd.run)(session, params))
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_doc_touch() {
        let reg = CommandRegistry::new();
        assert!(reg.get("doc.touch").is_some());
    }

    #[test]
    fn doc_touch_marks_dirty() {
        let reg = CommandRegistry::new();
        let mut s = Session::default();
        assert!(!s.dirty);
        let out = reg
            .run(&mut s, "doc.touch", &serde_json::Value::Null)
            .unwrap();
        assert!(s.dirty);
        assert_eq!(out, serde_json::json!({"ok": true}));
    }

    #[test]
    fn unknown_command_errs() {
        let reg = CommandRegistry::new();
        let mut s = Session::default();
        let err = reg
            .run(&mut s, "nope.nope", &serde_json::Value::Null)
            .unwrap_err();
        assert!(matches!(err, EngineError::UnknownCommand(_)));
    }
}
