//! XLL registration and `XLOPER12` adapters. The only module that speaks
//! directly to Excel.

pub mod parameter;

use xladd::registrator::Reg;

/// Called by Excel when the add-in is loaded. Registers every `CVX.*`
/// function exposed by this crate.
#[no_mangle]
pub extern "system" fn xlAutoOpen() -> i32 {
    crate::logging::init();

    let reg = Reg::new();
    reg.add(
        "CVX.PARAMETER",
        "QQQ$",
        "range, name",
        "cvxx",
        "Creates a cached cvxx parameter from a numeric range and returns its handle.",
        &[
            "A rectangular range of numeric cells. Empty cells are treated as zero.",
            "Optional unique name for the parameter.",
        ],
    );

    1
}
