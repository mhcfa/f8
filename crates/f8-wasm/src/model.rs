use f8_core::compiler::emission::EmissionOptions;
use f8_core::compiler::{CompiledModel, apply_unit_suffix};
use f8_core::model::id::{SheetId, TableId};
use f8_core::value::CellValue;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::error::{
    WasmErrorPayload, diagnostic_to_js_value, map_compiler_error, map_inspection_error,
    map_serde_error,
};

fn serialize_json_compatible<T: Serialize>(val: &T) -> Result<JsValue, JsValue> {
    val.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(map_serde_error)
}

fn serialize_sheet_json_compatible<T: Serialize>(val: &T) -> Result<JsValue, JsValue> {
    let json_val = serde_json::to_value(val).map_err(|e| {
        diagnostic_to_js_value(&WasmErrorPayload::SpecificationError {
            message: e.to_string(),
            table: None,
            reason: e.to_string(),
        })
    })?;
    json_val
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(map_serde_error)
}

/// Compiled, verified, and evaluated financial model ready for auditing and `.xlsx` emission.
#[wasm_bindgen]
pub struct WasmCompiledModel {
    inner: CompiledModel,
}

impl WasmCompiledModel {
    #[must_use]
    pub fn new(inner: CompiledModel) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl WasmCompiledModel {
    /// Emits the compiled OpenXML (`.xlsx`) workbook into an in-memory byte buffer.
    #[wasm_bindgen(js_name = "emitWorkbook")]
    pub fn emit_workbook(&self, options: JsValue) -> Result<Vec<u8>, JsValue> {
        let opts: EmissionOptions = if options.is_undefined() || options.is_null() {
            EmissionOptions::default()
        } else {
            serde_wasm_bindgen::from_value(options).map_err(map_serde_error)?
        };

        self.inner
            .emit_workbook(&opts)
            .map_err(|e| map_compiler_error(&e))
    }

    /// Returns high-level structural and cell classification statistics for the model.
    pub fn summary(&self) -> Result<JsValue, JsValue> {
        let res = self.inner.summary();
        serialize_json_compatible(&res)
    }

    /// Returns the unboxed scalar value (`number | string | boolean | null`) for a cell coordinate.
    pub fn value(
        &self,
        target: &str,
        row: Option<String>,
        col: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row.as_deref(), col.as_deref())
            .map_err(|e| map_inspection_error(&e))?;

        let val = self
            .inner
            .cell_value(&cell_id.table, &cell_id.row, &cell_id.col)
            .map_err(|e| map_inspection_error(&e))?;

        match val {
            CellValue::Number(n) => Ok(JsValue::from_f64(n)),
            CellValue::Text(s) => Ok(JsValue::from_str(&s)),
            CellValue::Boolean(b) => Ok(JsValue::from_bool(b)),
            CellValue::Empty => Ok(JsValue::NULL),
            CellValue::Error(e) => Ok(JsValue::from_str(e.as_str())),
        }
    }

    /// Returns the evaluated numeric cell value as a number (throws TypeError otherwise).
    pub fn num(
        &self,
        target: &str,
        row: Option<String>,
        col: Option<String>,
    ) -> Result<f64, JsValue> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row.as_deref(), col.as_deref())
            .map_err(|e| map_inspection_error(&e))?;

        let val = self
            .inner
            .cell_value(&cell_id.table, &cell_id.row, &cell_id.col)
            .map_err(|e| map_inspection_error(&e))?;

        match val {
            CellValue::Number(n) => Ok(n),
            other @ (CellValue::Text(_)
            | CellValue::Boolean(_)
            | CellValue::Empty
            | CellValue::Error(_)) => Err(diagnostic_to_js_value(&WasmErrorPayload::TypeError {
                message: format!("Cell {cell_id} did not evaluate to a float (got {other:?})"),
            })),
        }
    }

    /// Returns the formatted display string for a coordinate with an optional unit suffix.
    pub fn formatted(
        &self,
        target: &str,
        row: Option<String>,
        col: Option<String>,
        unit: Option<String>,
    ) -> Result<String, JsValue> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row.as_deref(), col.as_deref())
            .map_err(|e| map_inspection_error(&e))?;

        let formatted = self
            .inner
            .cell_formatted(&cell_id.table, &cell_id.row, &cell_id.col)
            .map_err(|e| map_inspection_error(&e))?;

        let unit_str = match &unit {
            Some(u) => u.as_str(),
            None => "",
        };

        Ok(apply_unit_suffix(formatted, unit_str))
    }

    /// Returns the cell inspection payload including formulas, formatting, and neighbors.
    #[wasm_bindgen(js_name = "inspectCell")]
    pub fn inspect_cell(
        &self,
        target: &str,
        row: Option<String>,
        col: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row.as_deref(), col.as_deref())
            .map_err(|e| map_inspection_error(&e))?;

        let inspection = self
            .inner
            .inspect_cell(&cell_id.table, &cell_id.row, &cell_id.col)
            .map_err(|e| map_inspection_error(&e))?;

        serialize_json_compatible(&inspection)
    }

    /// Returns the raw 2D `TableInspection` record for `table`.
    #[wasm_bindgen(js_name = "inspectTable")]
    pub fn inspect_table(&self, table: &str) -> Result<JsValue, JsValue> {
        let table_id = TableId::from(table);
        let inspection = self
            .inner
            .inspect_table(&table_id)
            .map_err(|e| map_inspection_error(&e))?;

        serialize_json_compatible(&inspection)
    }

    /// Returns the physical grid `SheetInspection` record for `sheet`.
    #[wasm_bindgen(js_name = "inspectSheet")]
    pub fn inspect_sheet(&self, sheet: &str) -> Result<JsValue, JsValue> {
        let sheet_id = SheetId::from(sheet);
        let inspection = self
            .inner
            .inspect_sheet(&sheet_id)
            .map_err(|e| map_inspection_error(&e))?;

        serialize_sheet_json_compatible(&inspection)
    }

    /// Traces direct or recursive upstream precedent cells for the target coordinate.
    #[wasm_bindgen(js_name = "precedentsOf")]
    pub fn precedents_of(
        &self,
        target: &str,
        row: Option<String>,
        col: Option<String>,
        recursive: Option<bool>,
    ) -> Result<JsValue, JsValue> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row.as_deref(), col.as_deref())
            .map_err(|e| map_inspection_error(&e))?;

        let is_recursive = recursive.unwrap_or_default();

        let tree = self
            .inner
            .precedents_of(&cell_id, is_recursive)
            .map_err(|e| map_inspection_error(&e))?;

        serialize_json_compatible(&tree)
    }

    /// Traces direct or recursive downstream dependent cells for the target coordinate.
    #[wasm_bindgen(js_name = "dependentsOf")]
    pub fn dependents_of(
        &self,
        target: &str,
        row: Option<String>,
        col: Option<String>,
        recursive: Option<bool>,
    ) -> Result<JsValue, JsValue> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row.as_deref(), col.as_deref())
            .map_err(|e| map_inspection_error(&e))?;

        let is_recursive = recursive.unwrap_or_default();

        let tree = self
            .inner
            .dependents_of(&cell_id, is_recursive)
            .map_err(|e| map_inspection_error(&e))?;

        serialize_json_compatible(&tree)
    }
}
