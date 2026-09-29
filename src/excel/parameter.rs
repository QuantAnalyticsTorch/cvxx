//! The `CVX.PARAMETER` Excel function.

use xladd::variant::Variant;
use xladd::xlcall::{xlerrValue, LPXLOPER12};

use crate::core::error::CvxError;
use crate::core::registry::Registry;
use crate::data;

/// `CVX.PARAMETER(range, [name])` — reads a numeric range, caches it as a
/// parameter, and returns a `cvx:param:<uuid>` handle.
#[export_name = "CVX.PARAMETER"]
pub extern "system" fn cvx_parameter(range: LPXLOPER12, name: LPXLOPER12) -> LPXLOPER12 {
    let result = match run(range, name) {
        Ok(handle) => Variant::from_str(&handle),
        Err(err) => {
            tracing::error!(error = %err, "CVX.PARAMETER failed");
            Variant::from_err(xlerrValue)
        }
    };

    Box::into_raw(Box::new(result)) as LPXLOPER12
}

fn run(range: LPXLOPER12, name: LPXLOPER12) -> Result<String, CvxError> {
    let range = Variant::from_xloper(range);
    let name = Variant::from_xloper(name);

    let parsed = data::parse_range(&range)?;
    let name = name.as_string().filter(|s| !s.trim().is_empty());

    Registry::global().insert_parameter(name, parsed.shape, parsed.data)
}
