pub mod error;
pub mod model;

use f8_core::compiler::Compiler;
use f8_core::model::manifest::ProjectManifest;
use f8_core::model::table::TableSpec;
use f8_core::model::theme::Theme;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::error::{map_compiler_error, map_serde_error};
use crate::model::WasmCompiledModel;

/// Returns the package version string.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns the built-in `corporate_navy` theme specification dictionary.
#[wasm_bindgen(js_name = "defaultTheme")]
pub fn default_theme() -> Result<JsValue, JsValue> {
    Theme::corporate_navy()
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(map_serde_error)
}

/// Compiles a project manifest and table specifications into an evaluated `WasmCompiledModel`.
#[wasm_bindgen]
pub fn compile(manifest: JsValue, tables: JsValue) -> Result<WasmCompiledModel, JsValue> {
    let project_manifest: ProjectManifest =
        serde_wasm_bindgen::from_value(manifest).map_err(map_serde_error)?;

    let table_specs: Vec<TableSpec> =
        serde_wasm_bindgen::from_value(tables).map_err(map_serde_error)?;

    let compiled =
        Compiler::compile(&project_manifest, &table_specs).map_err(|e| map_compiler_error(&e))?;

    Ok(WasmCompiledModel::new(compiled))
}
