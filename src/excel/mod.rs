//! XLL registration and `XLOPER12` adapters. The only module that speaks
//! directly to Excel.

pub mod constraint;
pub mod expression;
pub mod inspect;
pub mod parameter;
pub mod problem;
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
        "CVX.SUM",
        "QQQ$",
        "operand, name",
        "cvxx",
        "Sums every entry of a cvxx expression into a single value and returns the resulting handle.",
        &[
            "Handle of the (possibly vector/matrix-shaped) expression to sum.",
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

    reg.add(
        "CVX.INDEX",
        "QQQQQQQ$",
        "operand, row, col, rows, cols, name",
        "cvxx",
        "Selects a contiguous rectangular sub-block (an entry, row, column, or sub-section) of a cvxx expression and returns the resulting handle.",
        &[
            "Handle of the (possibly vector/matrix-shaped) expression to select from.",
            "1-based starting row of the selection.",
            "1-based starting column of the selection.",
            "Optional number of rows to select (default 1).",
            "Optional number of columns to select (default 1).",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.MATMUL",
        "QQQ$",
        "left, right, name",
        "cvxx",
        "Matrix-multiplies two cvxx expressions and returns the resulting handle.",
        &[
            "Handle of the left-hand expression.",
            "Handle of the right-hand expression.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.TRANSPOSE",
        "QQ$",
        "operand, name",
        "cvxx",
        "Transposes a cvxx expression (rows become columns and vice versa) and returns the resulting handle.",
        &[
            "Handle of the expression to transpose.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.CONSTRAINT",
        "QQQ$",
        "constraint_string, name",
        "cvxx",
        "Parses a constraint string and returns a cvxx constraint handle.",
        &[
            "A relational expression such as 'x + y <= 10'.",
            "Optional unique name for the constraint.",
        ],
    );

    reg.add(
        "CVX.LESS_THAN",
        "QQQ$",
        "left, right, name",
        "cvxx",
        "Builds a cvxx constraint 'left <= right' and returns its handle.",
        &[
            "Handle, name, or numeric literal for the left-hand side.",
            "Handle, name, or numeric literal for the right-hand side.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.GREATER_THAN",
        "QQQ$",
        "left, right, name",
        "cvxx",
        "Builds a cvxx constraint 'left >= right' and returns its handle.",
        &[
            "Handle, name, or numeric literal for the left-hand side.",
            "Handle, name, or numeric literal for the right-hand side.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.EQUAL",
        "QQQ$",
        "left, right, name",
        "cvxx",
        "Builds a cvxx constraint 'left == right' and returns its handle.",
        &[
            "Handle, name, or numeric literal for the left-hand side.",
            "Handle, name, or numeric literal for the right-hand side.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.CONSTRAINTS",
        "QQQ$",
        "constraints, name",
        "cvxx",
        "Combines a range of cvxx constraint handles into a constraint set handle.",
        &[
            "A range of constraint handles or names. Blank cells are skipped.",
            "Optional unique name for the constraint set.",
        ],
    );

    reg.add(
        "CVX.MINIMIZE",
        "QQQ$",
        "objective, name",
        "cvxx",
        "Builds a minimization objective and returns a cvxx objective handle.",
        &[
            "Handle, name, or numeric literal for the objective expression.",
            "Optional unique name for the objective.",
        ],
    );

    reg.add(
        "CVX.MAXIMIZE",
        "QQQ$",
        "objective, name",
        "cvxx",
        "Builds a maximization objective and returns a cvxx objective handle.",
        &[
            "Handle, name, or numeric literal for the objective expression.",
            "Optional unique name for the objective.",
        ],
    );

    reg.add(
        "CVX.PROBLEM",
        "QQQQ$",
        "objective, constraints, name",
        "cvxx",
        "Combines an objective and constraints into a cvxx problem and returns its handle.",
        &[
            "Handle or name of an objective created by CVX.MINIMIZE or CVX.MAXIMIZE.",
            "Blank, a constraint set handle/name, or a range of constraint handles/names.",
            "Optional unique name for the problem.",
        ],
    );

    reg.add(
        "CVX.SOLVE",
        "QQQ$",
        "problem, name",
        "cvxx",
        "Solves a cvxx problem and returns a result handle.",
        &[
            "Handle or name of a problem created by CVX.PROBLEM.",
            "Optional unique name for the result.",
        ],
    );

    reg.add(
        "CVX.VALUE",
        "QQQ$",
        "result, variable",
        "cvxx",
        "Returns a solved variable's value(s) from a cvxx result.",
        &[
            "Handle or name of a result created by CVX.SOLVE.",
            "Handle or name of a variable that was part of the solved problem.",
        ],
    );

    reg.add(
        "CVX.STATUS",
        "QQ$",
        "result",
        "cvxx",
        "Returns the solve status of a cvxx result as a string.",
        &["Handle or name of a result created by CVX.SOLVE."],
    );

    reg.add(
        "CVX.OBJECTIVE_VALUE",
        "QQ$",
        "result",
        "cvxx",
        "Returns the objective value of an optimal cvxx result.",
        &["Handle or name of a result created by CVX.SOLVE."],
    );

    reg.add(
        "CVX.DESCRIBE",
        "QQ$",
        "handle",
        "cvxx",
        "Describes any cvxx registry object (parameter, variable, expression, constraint, constraint set, objective, problem, or result) as a diagnostic string.",
        &["Handle or name of any cvxx registry object."],
    );

    reg.add(
        "CVX.SHAPE",
        "QQ$",
        "handle",
        "cvxx",
        "Returns the \"rows x cols\" shape of a cvxx parameter, variable, or expression.",
        &["Handle or name of a parameter, variable, or expression."],
    );

    reg.add(
        "CVX.TYPE",
        "QQ$",
        "handle",
        "cvxx",
        "Returns the type name of any cvxx registry object.",
        &["Handle or name of any cvxx registry object."],
    );

    1
}
