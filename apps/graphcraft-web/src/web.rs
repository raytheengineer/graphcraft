#![cfg(target_arch = "wasm32")]
//! wasm32-only entry point for the eframe-web runner.

use graphcraft_ui_egui::{AppState, FileInbox, FileService, InboxEvent};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Boot the GraphCraft web app. Call from JS with the id of a
/// `<canvas>` element.
#[wasm_bindgen]
pub async fn start(canvas_id: String) -> Result<(), JsValue> {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();

    let document = web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window"))?
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let canvas = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| JsValue::from_str("canvas id not found"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let web_options = eframe::WebOptions::default();
    eframe::WebRunner::new()
        .start(
            canvas,
            web_options,
            Box::new(|_cc| Ok(Box::new(WebApp::default()))),
        )
        .await
}

#[derive(Default)]
struct WebApp {
    state: AppState,
}

impl WebApp {
    fn default() -> Self {
        Self {
            state: AppState::new(Box::new(BrowserFileService)),
        }
    }
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.state.ui(ui);
    }
}

/// Browser-side `FileService`.
///
/// Open spawns `rfd::AsyncFileDialog`; the chosen file's bytes
/// arrive asynchronously and the service posts them to the inbox
/// for the UI to pick up on its next frame.
///
/// Save triggers a browser download by building a Blob and
/// clicking a hidden anchor.
struct BrowserFileService;

impl FileService for BrowserFileService {
    fn open_project(&self, inbox: &FileInbox) {
        let inbox = inbox.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let dialog = rfd::AsyncFileDialog::new()
                .set_title("Open GraphCraft project")
                .add_filter("GraphCraft project", &["graphcraft", "json"]);
            match dialog.pick_file().await {
                Some(handle) => {
                    let name = handle.file_name();
                    let bytes = handle.read().await;
                    inbox.push(InboxEvent::LoadGraphcraft { bytes, label: name });
                }
                None => {}
            }
        });
    }

    fn save_project(&self, default_name: &str, bytes: Vec<u8>, inbox: &FileInbox) {
        match download_blob(default_name, &bytes) {
            Ok(filename) => inbox.push(InboxEvent::Status(format!("saved {filename}"))),
            Err(e) => inbox.push(InboxEvent::Status(format!("save failed: {e}"))),
        }
    }
}

/// Trigger a browser download of `bytes` as `{name}.graphcraft`.
fn download_blob(name: &str, bytes: &[u8]) -> Result<String, String> {
    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let document = window.document().ok_or_else(|| "no document".to_string())?;

    let u8 = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    u8.copy_from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&u8.buffer());

    let props = web_sys::BlobPropertyBag::new();
    props.set_type("application/json");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &props)
        .map_err(|e| format!("blob: {e:?}"))?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("object url: {e:?}"))?;
    let filename = format!("{name}.graphcraft");

    let anchor = document
        .create_element("a")
        .map_err(|e| format!("create a: {e:?}"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "anchor cast".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(&filename);
    anchor.style().set_property("display", "none").ok();
    document
        .body()
        .ok_or_else(|| "no body".to_string())?
        .append_child(&anchor)
        .map_err(|e| format!("append: {e:?}"))?;
    anchor.click();
    anchor.remove();
    web_sys::Url::revoke_object_url(&url).ok();
    Ok(filename)
}
