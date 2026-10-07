//! The data model: decision variables, lazy expressions, constraints, and
//! problems. Pure data definitions — no reduction or solving logic (see
//! [`crate::reduce`] and [`crate::solver`]).

/// A decision variable of a given shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Variable {
    /// Identity distinguishing this variable from any other, even one of
    /// the same shape.
    pub id: u64,
    /// `(rows, cols)`.
    pub shape: (usize, usize),
}

impl Variable {
    /// Creates a new variable with the requested id and shape.
    pub fn new(id: u64, shape: (usize, usize)) -> Self {
        Variable { id, shape }
    }
}

/// A lazy convex optimization expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A scalar numeric constant.
    Constant(f64),
    /// A reference to a variable.
    Variable(Variable),
    /// A reference to a parameter (dense numeric data).
    Parameter {
        /// Identity distinguishing this parameter reference from any
        /// other, even one of the same shape and data. Not used by the
        /// solver; read only for display purposes by `cvxx`.
        id: u64,
        shape: (usize, usize),
        data: Vec<f64>,
    },
    /// Addition of two expressions.
    Add(Box<Expression>, Box<Expression>),
    /// Subtraction of two expressions.
    Sub(Box<Expression>, Box<Expression>),
    /// Multiplication of two expressions.
    Mul(Box<Expression>, Box<Expression>),
    /// Division of two expressions.
    Div(Box<Expression>, Box<Expression>),
    /// Unary negation.
    Neg(Box<Expression>),
    /// Scaling by a scalar constant.
    Scale { scalar: f64, expr: Box<Expression> },
    /// The sum of every entry of a (possibly vector/matrix-shaped)
    /// expression, reduced to a `(1, 1)` value (SPEC-0014).
    Sum(Box<Expression>),
    /// A contiguous, rectangular, row-major sub-block of a (possibly
    /// vector/matrix-shaped) expression: `rows` rows starting at
    /// `row_start`, `cols` columns starting at `col_start`, all 0-based
    /// (SPEC-0015). Covers a single entry (`rows == cols == 1`), a single
    /// row (`rows == 1`) or column (`cols == 1`), or any other rectangular
    /// sub-section.
    Index {
        expr: Box<Expression>,
        row_start: usize,
        col_start: usize,
        rows: usize,
        cols: usize,
    },
    /// Standard (2-D) matrix multiplication of two (possibly
    /// vector/matrix-shaped) expressions: `left` contributes `rows`, which
    /// must match `right`'s row count against `left`'s column count (see
    /// `cvxrust::reduce::matmul_shape`); the result has `left`'s row count
    /// and `right`'s column count (SPEC-0018).
    MatMul(Box<Expression>, Box<Expression>),
    /// The row/column transpose of a (possibly vector/matrix-shaped)
    /// expression: a `rows x cols` operand becomes `cols x rows`
    /// (SPEC-0018).
    Transpose(Box<Expression>),
}

impl Expression {
    /// Creates a constant expression.
    pub fn constant(value: f64) -> Self {
        Expression::Constant(value)
    }

    /// Creates a variable expression.
    pub fn from_variable(variable: Variable) -> Self {
        Expression::Variable(variable)
    }

    /// Creates a parameter expression with the given opaque identity.
    pub fn from_parameter(id: u64, shape: (usize, usize), data: Vec<f64>) -> Self {
        Expression::Parameter { id, shape, data }
    }

    /// Creates an addition expression.
    #[allow(clippy::should_implement_trait)]
    pub fn add(left: Expression, right: Expression) -> Self {
        Expression::Add(Box::new(left), Box::new(right))
    }

    /// Creates a subtraction expression.
    #[allow(clippy::should_implement_trait)]
    pub fn sub(left: Expression, right: Expression) -> Self {
        Expression::Sub(Box::new(left), Box::new(right))
    }

    /// Creates a multiplication expression.
    #[allow(clippy::should_implement_trait)]
    pub fn mul(left: Expression, right: Expression) -> Self {
        Expression::Mul(Box::new(left), Box::new(right))
    }

    /// Creates a division expression.
    #[allow(clippy::should_implement_trait)]
    pub fn div(left: Expression, right: Expression) -> Self {
        Expression::Div(Box::new(left), Box::new(right))
    }

    /// Creates a negation expression.
    #[allow(clippy::should_implement_trait)]
    pub fn neg(expr: Expression) -> Self {
        Expression::Neg(Box::new(expr))
    }

    /// Creates a scalar-scaling expression.
    pub fn scale(scalar: f64, expr: Expression) -> Self {
        Expression::Scale {
            scalar,
            expr: Box::new(expr),
        }
    }

    /// Creates a sum-reduction expression: every entry of `expr` (which may
    /// be vector/matrix-shaped) collapsed into a single `(1, 1)` value
    /// (SPEC-0014).
    pub fn sum(expr: Expression) -> Self {
        Expression::Sum(Box::new(expr))
    }

    /// Creates a sub-block index expression. `rows`/`cols` are always
    /// `>= 1` for any `Index` built through `CVX.INDEX` (enforced at the
    /// `cvxx` boundary by reusing `data::parse_dimension`/
    /// `parse_optional_dimension`); this constructor does not itself
    /// re-validate positivity or bounds, consistent with how
    /// `Variable::new`/`Expression::from_parameter` never re-validate
    /// `shape` either (SPEC-0015).
    pub fn index(
        expr: Expression,
        row_start: usize,
        col_start: usize,
        rows: usize,
        cols: usize,
    ) -> Self {
        Expression::Index {
            expr: Box::new(expr),
            row_start,
            col_start,
            rows,
            cols,
        }
    }

    /// Creates a matrix-multiplication expression.
    pub fn matmul(left: Expression, right: Expression) -> Self {
        Expression::MatMul(Box::new(left), Box::new(right))
    }

    /// Creates a transpose expression.
    pub fn transpose(expr: Expression) -> Self {
        Expression::Transpose(Box::new(expr))
    }
}

/// The optimization sense of a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sense {
    Minimize,
    Maximize,
}

/// A relational operator between two expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    LessEqual,
    GreaterEqual,
    Equal,
}

/// A single constraint: a relation between two expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub relation: Relation,
    pub lhs: Expression,
    pub rhs: Expression,
}

/// Which discrete values a variable's elements are restricted to.
/// Ordered so that `Binary` is treated as strictly more restrictive than
/// `Integer` when overlapping domain restrictions apply to the same
/// scalar entry (SPEC-0019): `Domain::Binary > Domain::Integer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    Integer,
    Binary,
}

/// Restricts a rectangular sub-block of a variable's scalar entries to a
/// discrete domain. Uses the same row/col addressing as
/// [`Expression::Index`] (SPEC-0015): `rows` rows starting at `row_start`,
/// `cols` columns starting at `col_start`, all 0-based, within
/// `variable`'s shape (SPEC-0019).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainConstraint {
    pub variable: Variable,
    pub row_start: usize,
    pub col_start: usize,
    pub rows: usize,
    pub cols: usize,
    pub domain: Domain,
}

/// A convex optimization problem: an objective with a sense, a list of
/// constraints, and the ordered list of variables the caller wants solved
/// values for.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub sense: Sense,
    pub objective: Expression,
    pub constraints: Vec<Constraint>,
    /// Integer/binary domain restrictions (SPEC-0019). Empty for every
    /// problem built before this specification; a non-empty list routes
    /// the problem to the `microlp` translation path instead of
    /// `clarabel` (see `crate::solver::solve`).
    pub domains: Vec<DomainConstraint>,
    /// Ordered, positionally aligned with `Solution::variable_values`.
    pub variables: Vec<Variable>,
}

/// The outcome of attempting to solve a [`Problem`].
#[derive(Debug, Clone, PartialEq)]
pub enum SolveStatus {
    Optimal,
    Infeasible,
    Unbounded,
    /// A feasible solution honoring every domain restriction was found,
    /// but the `microlp` branch-and-bound search (SPEC-0019) was stopped
    /// (time limit or node limit) before optimality could be proven.
    /// `Solution::objective_value`/`Solution::variable_values` are
    /// populated with the best incumbent found, exactly as for `Optimal`.
    StoppedAtLimit,
    Error(String),
}

/// The result of a solve attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    pub status: SolveStatus,
    pub objective_value: Option<f64>,
    /// Aligned by index with `Problem::variables`; each inner `Vec<f64>` is
    /// row-major data matching that variable's shape. Populated for
    /// `Optimal` and `StoppedAtLimit`; empty otherwise.
    pub variable_values: Vec<Vec<f64>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_constraint() {
        let constraint = Constraint {
            relation: Relation::LessEqual,
            lhs: Expression::constant(1.0),
            rhs: Expression::constant(2.0),
        };
        assert_eq!(constraint.relation, Relation::LessEqual);
    }
}
