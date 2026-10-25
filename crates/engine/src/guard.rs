//! Last-line-of-defence panic guard.
//!
//! Every entry point (command dispatch, control request, UI frame, MCP)
//! calls through [`call`]. A panic in the inner closure is caught and
//! surfaced as `EngineError::Panic`, so the host does not abort.
//!
//! WASM aborts on panic — the no-panic clippy lints are the real defence
//! on the web build. This guard exists for native and for debugging.

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::EngineError;

pub fn call<F, T>(f: F) -> Result<T, EngineError>
where
    F: FnOnce() -> Result<T, EngineError>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&'static str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            Err(EngineError::Panic(msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_passes_through() {
        let v = call(|| Ok::<_, EngineError>(42)).unwrap();
        assert_eq!(v, 42);
    }

    #[test]
    fn panic_is_caught() {
        let err = call::<_, ()>(|| panic!("boom")).unwrap_err();
        match err {
            EngineError::Panic(m) => assert!(m.contains("boom")),
            other => panic!("expected Panic, got {other:?}"),
        }
    }
}
