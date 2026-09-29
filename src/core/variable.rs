//! Re-export of the `cvxrust` variable type.
//!
//! Keeping the re-export in one place lets the rest of `cvxx` refer to a
//! single `core::Variable` alias while the real `cvxrust` crate supplies the
//! implementation.

pub use cvxrust::Variable;
