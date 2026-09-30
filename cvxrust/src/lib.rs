//! `cvxrust` — placeholder convex optimization modeling layer.
//!
//! This crate is a minimal stand-in for the real `cvxrust` dependency. It
//! provides the variable and expression types needed by `cvxx` so that the
//! Excel add-in can be built and tested without a full solver implementation.
//!
//! When the real `cvxrust` crate is available, this module can be replaced or
//! extended; the public types are intentionally small and stable.

/// A decision variable of a given shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// `(rows, cols)`.
    pub shape: (usize, usize),
}

impl Variable {
    /// Creates a new variable with the requested shape.
    pub fn new(shape: (usize, usize)) -> Self {
        Variable { shape }
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

    /// Creates a parameter expression.
    pub fn from_parameter(shape: (usize, usize), data: Vec<f64>) -> Self {
        Expression::Parameter { shape, data }
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

/// A convex optimization problem: an objective with a sense, a list of
/// constraints, and the ordered list of variables the caller wants solved
/// values for.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub sense: Sense,
    pub objective: Expression,
    pub constraints: Vec<Constraint>,
    /// Ordered, positionally aligned with `Solution::variable_values`.
    pub variables: Vec<Variable>,
}

/// The outcome of attempting to solve a [`Problem`].
#[derive(Debug, Clone, PartialEq)]
pub enum SolveStatus {
    Optimal,
    Infeasible,
    Unbounded,
    Error(String),
}

/// The result of a solve attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    pub status: SolveStatus,
    pub objective_value: Option<f64>,
    /// Aligned by index with `Problem::variables`; each inner `Vec<f64>` is
    /// row-major data matching that variable's shape. Empty when `status`
    /// is not `Optimal`.
    pub variable_values: Vec<Vec<f64>>,
}

/// Attempts to solve `problem`. This placeholder crate has no solver
/// implementation yet, so this always reports that solving is not
/// implemented; a real `cvxrust` would replace this with an actual convex
/// solver.
pub fn solve(_problem: &Problem) -> Solution {
    Solution {
        status: SolveStatus::Error("not implemented".to_string()),
        objective_value: None,
        variable_values: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solve_reports_not_implemented() {
        let problem = Problem {
            sense: Sense::Minimize,
            objective: Expression::constant(0.0),
            constraints: Vec::new(),
            variables: Vec::new(),
        };
        let solution = solve(&problem);
        assert_eq!(
            solution.status,
            SolveStatus::Error("not implemented".to_string())
        );
        assert_eq!(solution.objective_value, None);
        assert!(solution.variable_values.is_empty());
    }

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
