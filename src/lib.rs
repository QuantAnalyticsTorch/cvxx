//! `cvxx` exposes `cvxrust` convex optimization to Excel through a Rust XLL.
//!
//! Module layout follows the layered architecture in the repository's
//! `copilot-instructions.md`:
//!
//! - [`core`] — handle registry, opaque identifiers, and shared error types.
//! - [`data`] — Excel range parsing and normalization into Rust types.
//! - [`excel`] — XLL registration and `XLOPER12` adapters (the only module
//!   that speaks Excel).

pub mod core;
pub mod data;
pub mod excel;
mod logging;
