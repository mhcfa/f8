use f8_core::compiler::InspectionError as CoreInspectionError;
use f8_core::compiler::error::CompilerError;
pub use f8_core::compiler::error::ErrorDiagnostic as WasmErrorPayload;
use serde::Serialize;
use wasm_bindgen::JsValue;

#[must_use]
pub fn diagnostic_to_js_value(diag: &WasmErrorPayload) -> JsValue {
    match diag.serialize(&serde_wasm_bindgen::Serializer::json_compatible()) {
        Ok(val) => val,
        Err(e) => JsValue::from_str(&e.to_string()),
    }
}

#[must_use]
pub fn map_inspection_error(err: &CoreInspectionError) -> JsValue {
    diagnostic_to_js_value(&err.diagnostic())
}

#[must_use]
pub fn map_compiler_error(err: &CompilerError) -> JsValue {
    diagnostic_to_js_value(&err.diagnostic())
}

#[must_use]
pub fn map_serde_error(err: serde_wasm_bindgen::Error) -> JsValue {
    let msg = err.to_string();
    diagnostic_to_js_value(&WasmErrorPayload::SpecificationError {
        message: msg.clone(),
        table: None,
        reason: msg,
    })
}
