//! The `CVX.VARIABLE` Excel function.

use xladd::variant::Variant;
use xladd::xlcall::{xlerrValue, LPXLOPER12};

use crate::core::error::CvxError;
use crate::core::registry::Registry;
use crate::data;

/// `CVX.VARIABLE(rows, cols, [name])` — creates a shaped optimization
/// variable, stores it in the registry, and returns a `cvx:var:<uuid>`
/// handle.
#[export_name = "CVX.VARIABLE"]
pub extern "system" fn cvx_variable(
    rows: LPXLOPER12,
    cols: LPXLOPER12,
    name: LPXLOPER12,
) -> LPXLOPER12 {
    let result = match run(rows, cols, name) {
        Ok(handle) => Variant::from_str(&handle),
        Err(err) => {
            tracing::error!(error = %err, "CVX.VARIABLE failed");
            Variant::from_err(xlerrValue)
        }
    };

    Box::into_raw(Box::new(result)) as LPXLOPER12
}

fn run(rows: LPXLOPER12, cols: LPXLOPER12, name: LPXLOPER12) -> Result<String, CvxError> {
    let rows = data::parse_dimension(&Variant::from_xloper(rows))?;
    let cols = data::parse_dimension(&Variant::from_xloper(cols))?;

    let name = data::parse_optional_name(&Variant::from_xloper(name))?;

    Registry::global().insert_variable(name, (rows, cols))
}
