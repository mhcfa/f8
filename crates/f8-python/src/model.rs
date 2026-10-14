use std::path::PathBuf;

use f8_core::compiler::CompiledModel;
use f8_core::compiler::emission::EmissionOptions;
use f8_core::compiler::error::CompilerError;
use f8_core::model::id::{SheetId, TableId};
use f8_core::value::CellValue;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error::{
    make_emission_error, map_compiler_error, map_inspection_error, map_pythonize_error,
};

fn apply_unit_suffix(formatted: String, unit: &str) -> String {
    if formatted.is_empty() || unit.is_empty() {
        return formatted;
    }
    if let Some(stem) = formatted.strip_suffix(')') {
        format!("{stem}{unit})")
    } else {
        format!("{formatted}{unit}")
    }
}

#[pyclass(name = "CompiledModel")]
pub struct PyCompiledModel {
    pub(crate) inner: CompiledModel,
}

impl PyCompiledModel {
    pub fn new(inner: CompiledModel) -> Self {
        Self { inner }
    }
}

enum SaveError {
    Compiler(CompilerError),
    Io(String),
}

#[pymethods]
impl PyCompiledModel {
    #[pyo3(signature = (path, options = None))]
    pub fn save<'py>(
        &self,
        py: Python<'py>,
        path: PathBuf,
        options: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<()> {
        let opts = match options {
            Some(opt) if !opt.is_none() => pythonize::depythonize::<EmissionOptions>(opt)
                .map_err(|e| map_pythonize_error(py, e))?,
            _ => EmissionOptions::default(),
        };

        py.allow_threads(|| -> Result<(), SaveError> {
            let bytes = self
                .inner
                .emit_workbook(&opts)
                .map_err(SaveError::Compiler)?;

            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
                && !parent.exists()
            {
                std::fs::create_dir_all(parent).map_err(|e| {
                    SaveError::Io(format!(
                        "Failed to create parent directory '{}': {}",
                        parent.display(),
                        e
                    ))
                })?;
            }

            std::fs::write(&path, &bytes).map_err(|e| {
                SaveError::Io(format!(
                    "Failed to write workbook to '{}': {}",
                    path.display(),
                    e
                ))
            })?;

            Ok(())
        })
        .map_err(|err| match err {
            SaveError::Compiler(compiler_err) => map_compiler_error(py, &compiler_err),
            SaveError::Io(io_err) => make_emission_error(py, io_err),
        })?;

        Ok(())
    }
    #[pyo3(signature = (options = None))]
    pub fn emit_workbook<'py>(
        &self,
        py: Python<'py>,
        options: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let opts = match options {
            Some(opt) if !opt.is_none() => pythonize::depythonize::<EmissionOptions>(opt)
                .map_err(|e| map_pythonize_error(py, e))?,
            _ => EmissionOptions::default(),
        };

        let bytes = py
            .allow_threads(|| self.inner.emit_workbook(&opts))
            .map_err(|e| map_compiler_error(py, &e))?;

        Ok(PyBytes::new(py, &bytes))
    }

    pub fn summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let res = self.inner.summary();
        pythonize::pythonize(py, &res).map_err(|e| map_pythonize_error(py, e))
    }

    #[pyo3(signature = (target, row = None, col = None))]
    pub fn value<'py>(
        &self,
        py: Python<'py>,
        target: &str,
        row: Option<&str>,
        col: Option<&str>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row, col)
            .map_err(|e| map_inspection_error(py, &e))?;

        let val = self
            .inner
            .cell_value(&cell_id.table, &cell_id.row, &cell_id.col)
            .map_err(|e| map_inspection_error(py, &e))?;

        match val {
            CellValue::Number(n) => Ok(n.into_pyobject(py)?.into_any()),
            CellValue::Text(s) => Ok(s.into_pyobject(py)?.into_any()),
            CellValue::Boolean(b) => Ok(b.into_pyobject(py)?.to_owned().into_any()),
            CellValue::Empty => Ok(py.None().into_bound(py)),
            CellValue::Error(e) => Ok(e.as_str().into_pyobject(py)?.into_any()),
        }
    }

    #[pyo3(signature = (target, row = None, col = None))]
    pub fn num(
        &self,
        py: Python<'_>,
        target: &str,
        row: Option<&str>,
        col: Option<&str>,
    ) -> PyResult<f64> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row, col)
            .map_err(|e| map_inspection_error(py, &e))?;

        let val = self
            .inner
            .cell_value(&cell_id.table, &cell_id.row, &cell_id.col)
            .map_err(|e| map_inspection_error(py, &e))?;

        match val {
            CellValue::Number(n) => Ok(n),
            other => Err(PyTypeError::new_err(format!(
                "Cell {cell_id} did not evaluate to a float (got {other:?})"
            ))),
        }
    }

    #[pyo3(signature = (target, row = None, col = None, *, unit = ""))]
    pub fn formatted(
        &self,
        py: Python<'_>,
        target: &str,
        row: Option<&str>,
        col: Option<&str>,
        unit: &str,
    ) -> PyResult<String> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row, col)
            .map_err(|e| map_inspection_error(py, &e))?;

        let formatted = self
            .inner
            .cell_formatted(&cell_id.table, &cell_id.row, &cell_id.col)
            .map_err(|e| map_inspection_error(py, &e))?;

        Ok(apply_unit_suffix(formatted, unit))
    }

    #[pyo3(signature = (target, row = None, col = None))]
    pub fn inspect_cell<'py>(
        &self,
        py: Python<'py>,
        target: &str,
        row: Option<&str>,
        col: Option<&str>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row, col)
            .map_err(|e| map_inspection_error(py, &e))?;

        let inspection = self
            .inner
            .inspect_cell(&cell_id.table, &cell_id.row, &cell_id.col)
            .map_err(|e| map_inspection_error(py, &e))?;

        pythonize::pythonize(py, &inspection).map_err(|e| map_pythonize_error(py, e))
    }

    #[pyo3(signature = (table,))]
    pub fn inspect_table<'py>(&self, py: Python<'py>, table: &str) -> PyResult<Bound<'py, PyAny>> {
        let table_id = TableId::from(table);
        let inspection = self
            .inner
            .inspect_table(&table_id)
            .map_err(|e| map_inspection_error(py, &e))?;

        pythonize::pythonize(py, &inspection).map_err(|e| map_pythonize_error(py, e))
    }

    #[pyo3(signature = (table,))]
    pub fn table<'py>(&self, py: Python<'py>, table: &str) -> PyResult<Bound<'py, PyAny>> {
        let raw = self.inspect_table(py, table)?;
        let audit_mod = py.import("f8.audit")?;
        let auditor_cls = audit_mod.getattr("TableAuditor")?;
        auditor_cls.call1((raw,))
    }

    #[pyo3(signature = (sheet,))]
    pub fn inspect_sheet<'py>(&self, py: Python<'py>, sheet: &str) -> PyResult<Bound<'py, PyAny>> {
        let sheet_id = SheetId::from(sheet);
        let inspection = self
            .inner
            .inspect_sheet(&sheet_id)
            .map_err(|e| map_inspection_error(py, &e))?;

        pythonize::pythonize(py, &inspection).map_err(|e| map_pythonize_error(py, e))
    }

    #[pyo3(signature = (target, row = None, col = None, recursive = false))]
    pub fn precedents_of<'py>(
        &self,
        py: Python<'py>,
        target: &str,
        row: Option<&str>,
        col: Option<&str>,
        recursive: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row, col)
            .map_err(|e| map_inspection_error(py, &e))?;

        let tree = self
            .inner
            .precedents_of(&cell_id, recursive)
            .map_err(|e| map_inspection_error(py, &e))?;

        pythonize::pythonize(py, &tree).map_err(|e| map_pythonize_error(py, e))
    }

    #[pyo3(signature = (target, row = None, col = None, recursive = false))]
    pub fn dependents_of<'py>(
        &self,
        py: Python<'py>,
        target: &str,
        row: Option<&str>,
        col: Option<&str>,
        recursive: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let cell_id = self
            .inner
            .parse_coordinate(target, row, col)
            .map_err(|e| map_inspection_error(py, &e))?;

        let tree = self
            .inner
            .dependents_of(&cell_id, recursive)
            .map_err(|e| map_inspection_error(py, &e))?;

        pythonize::pythonize(py, &tree).map_err(|e| map_pythonize_error(py, e))
    }
}
