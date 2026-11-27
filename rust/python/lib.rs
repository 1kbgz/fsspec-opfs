use pyo3::prelude::*;

/// Python distributions contain browser assets; OPFS executes in the browser.
#[pymodule]
fn fsspec_opfs(m: &Bound<PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
