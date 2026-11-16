use pyo3::prelude::*;
mod git;

/// Return the version of the Rust core.
#[pyfunction]
fn core_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(core_version, m)?)?;
    m.add_function(wrap_pyfunction!(print_history, m)?)?;
    Ok(())
}

#[pyfunction]
fn print_history(path: &str, limit: usize) -> PyResult<Vec<String>> {
    git::print_history(path, limit)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}
