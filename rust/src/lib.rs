//! Async browser OPFS backend for fsspec-rs.
//!
//! Build for wasm32-unknown-unknown. Native Python packages contain browser
//! assets and do not expose the browser filesystem to native Python.

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod browser;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use browser::{Opfs, OpfsWriter};
