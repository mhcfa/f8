pub mod error;
pub mod model;

use f8_core::compiler::Compiler;
use f8_core::compiler::error::CompilerError;
use f8_core::model::manifest::ProjectManifest;
use f8_core::model::table::TableSpec;
use f8_core::model::theme::Theme;
use pyo3::prelude::*;

use crate::error::{map_compiler_error, map_pythonize_error, register_exceptions};
use crate::model::PyCompiledModel;

#[pyfunction]
#[pyo3(signature = (manifest, tables, theme = None))]
pub fn compile<'py>(
    py: Python<'py>,
    manifest: &Bound<'py, PyAny>,
    tables: &Bound<'py, PyAny>,
    theme: Option<&Bound<'py, PyAny>>,
) -> PyResult<PyCompiledModel> {
    let project_manifest: ProjectManifest =
        pythonize::depythonize(manifest).map_err(|e| map_pythonize_error(py, e))?;

    let table_specs: Vec<TableSpec> =
        pythonize::depythonize(tables).map_err(|e| map_pythonize_error(py, e))?;

    let resolved_theme: Theme = match theme {
        None => project_manifest
            .resolve_theme()
            .map_err(|e| map_compiler_error(py, &CompilerError::Model(e)))?,
        Some(th) if th.is_none() => project_manifest
            .resolve_theme()
            .map_err(|e| map_compiler_error(py, &CompilerError::Model(e)))?,
        Some(th) => {
            if let Ok(theme_name) = th.extract::<&str>() {
                Theme::resolve_builtin(theme_name)
                    .map_err(|e| map_compiler_error(py, &CompilerError::Model(e)))?
            } else {
                pythonize::depythonize::<Theme>(th).map_err(|e| map_pythonize_error(py, e))?
            }
        }
    };

    let compiled = py
        .allow_threads(|| Compiler::compile(&project_manifest, &table_specs, &resolved_theme))
        .map_err(|e| map_compiler_error(py, &e))?;

    Ok(PyCompiledModel::new(compiled))
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_exceptions(m)?;
    m.add_class::<PyCompiledModel>()?;
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    Ok(())
}
