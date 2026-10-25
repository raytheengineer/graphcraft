//! GraphCraft in the browser.
//!
//! Boots the shared `graphcraft_ui_egui` UI under eframe's web
//! runner. One canvas, same `AppState` the desktop app uses.
//!
//! The `.graphcraft` Open/Save path uses browser-native primitives:
//! * Save packages the project as a `.graphcraft` file via a Blob
//!   + object URL + anchor click.
//! * Open is handled through drag-and-drop onto the window (egui
//!   already surfaces dropped files). A full browser file picker
//!   is a follow-up.
//!
//! Build with `apps/graphcraft-web/build.sh` (wrapping wasm-pack).

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::start;
