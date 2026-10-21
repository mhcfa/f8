pub mod error;
pub mod model;

use f8_core::compiler::Compiler;
use f8_core::model::manifest::ProjectManifest;
use f8_core::model::table::TableSpec;
use pyo3::prelude::*;

use crate::error::{map_compiler_error, map_pythonize_error, register_exceptions};
use crate::model::PyCompiledModel;

/// Compiles a project manifest and table specifications into an evaluated `CompiledModel`.
#[pyfunction]
#[pyo3(signature = (manifest, tables))]
pub fn compile<'py>(
    py: Python<'py>,
    manifest: &Bound<'py, PyAny>,
    tables: &Bound<'py, PyAny>,
) -> PyResult<PyCompiledModel> {
    let project_manifest: ProjectManifest =
        pythonize::depythonize(manifest).map_err(|e| map_pythonize_error(py, e))?;

    let table_specs: Vec<TableSpec> =
        pythonize::depythonize(tables).map_err(|e| map_pythonize_error(py, e))?;

    let compiled = py
        .allow_threads(|| Compiler::compile(&project_manifest, &table_specs))
        .map_err(|e| map_compiler_error(py, &e))?;

    Ok(PyCompiledModel::new(compiled))
}

/// Native Rust compiler bindings (`f8._core`).
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    register_exceptions(m)?;
    m.add_class::<PyCompiledModel>()?;
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    Ok(())
}
