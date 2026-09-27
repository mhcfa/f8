use f8_core::compiler::CompiledModel;
use f8_core::compiler::emission::EmissionOptions;
use f8_core::model::id::{SheetId, TableId};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::error::{map_compiler_error, map_inspection_error, map_pythonize_error};

#[pyclass(name = "CompiledModel")]
pub struct PyCompiledModel {
    pub(crate) inner: CompiledModel,
}

impl PyCompiledModel {
    pub fn new(inner: CompiledModel) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCompiledModel {
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
