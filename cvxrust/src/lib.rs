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
