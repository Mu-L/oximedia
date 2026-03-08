//! Utilities for WASM bindings.

use oximedia_core::OxiError;
use wasm_bindgen::JsValue;

/// Convert `OxiError` to `JsValue` for JavaScript exception handling.
///
/// This allows Rust errors to be propagated to JavaScript as exceptions.
pub fn to_js_error(err: OxiError) -> JsValue {
    JsValue::from_str(&format!("OxiMedia Error: {err}"))
}

/// Convert `OxiError` to JavaScript `Error` object.
///
/// Provides more detailed error information for JavaScript consumers.
#[allow(dead_code)]
pub fn to_js_error_object(err: OxiError) -> js_sys::Error {
    js_sys::Error::new(&format!("OxiMedia Error: {err}"))
}
