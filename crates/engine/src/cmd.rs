//! The engine command registry.
//!
//! M1 ships exactly one command (`doc.touch`) so the registry, panic guard
//! and control channel are exercised end-to-end. Later milestones add real
//! analysis / plot / format commands here.

use crate::{Command, EngineError, Session};

fn run_doc_touch(
    session: &mut Session,
    _params: &serde_json::Value,
) -> Result<serde_json::Value, EngineError> {
    session.dirty = true;
    Ok(serde_json::json!({"ok": true}))
}

const DOC_TOUCH: Command = Command {
    id: "doc.touch",
    label: "Mark Document Dirty",
    menu_path: "File > (hidden) > Touch",
    run: run_doc_touch,
};

pub(crate) const ALL: &[&Command] = &[&DOC_TOUCH];
