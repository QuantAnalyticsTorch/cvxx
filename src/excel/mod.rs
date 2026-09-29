//! XLL registration and `XLOPER12` adapters. The only module that speaks
//! directly to Excel.

pub mod expression;
pub mod parameter;
pub mod variable;

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

    reg.add(
        "CVX.VARIABLE",
        "QQQQ$",
        "rows, cols, name",
        "cvxx",
        "Creates a shaped cvxx optimization variable and returns its handle.",
        &[
            "Number of rows in the variable (positive integer).",
            "Number of columns in the variable (positive integer).",
            "Optional unique name for the variable.",
        ],
    );

    reg.add(
        "CVX.EXPRESSION",
        "QQQ$",
        "expr_string, name",
        "cvxx",
        "Parses an expression string and returns a cvxx expression handle.",
        &[
            "A mathematical expression such as 'x + y' or '2.5 * (A - b)'.",
            "Optional unique name for the expression.",
        ],
    );

    reg.add(
        "CVX.ADD",
        "QQQ$",
        "left, right, name",
        "cvxx",
        "Adds two cvxx expressions and returns the resulting handle.",
        &[
            "Handle of the left-hand expression.",
            "Handle of the right-hand expression.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.SUB",
        "QQQ$",
        "left, right, name",
        "cvxx",
        "Subtracts two cvxx expressions and returns the resulting handle.",
        &[
            "Handle of the left-hand expression.",
            "Handle of the right-hand expression.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.MUL",
        "QQQ$",
        "left, right, name",
        "cvxx",
        "Multiplies two cvxx expressions and returns the resulting handle.",
        &[
            "Handle of the left-hand expression.",
            "Handle of the right-hand expression.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.DIV",
        "QQQ$",
        "left, right, name",
        "cvxx",
        "Divides two cvxx expressions and returns the resulting handle.",
        &[
            "Handle of the left-hand expression.",
            "Handle of the right-hand expression.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.NEG",
        "QQQ$",
        "operand, name",
        "cvxx",
        "Negates a cvxx expression and returns the resulting handle.",
        &[
            "Handle of the expression to negate.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.SCALE",
        "QQQ$",
        "operand, scalar, name",
        "cvxx",
        "Scales a cvxx expression by a numeric constant.",
        &[
            "Handle of the expression to scale.",
            "Numeric scalar multiplier.",
            "Optional unique name for the result.",
        ],
    );

    1
}
