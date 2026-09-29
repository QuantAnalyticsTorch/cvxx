//! Handle registry, opaque identifiers, and shared error types.

pub mod error;
pub mod handle;
pub mod registry;

pub use error::CvxError;
pub use handle::{format_handle, parse_handle, HandleKind};
pub use registry::{ParameterEntry, Registry};
