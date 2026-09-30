//! Expression AST used before conversion to `cvxrust::Expression`.

use std::sync::Arc;

/// An AST node for a user-supplied expression string.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A scalar numeric constant.
    Constant(f64),
    /// A reference to a registry object by its user-supplied name or handle.
    Identifier(String),
    /// Addition.
    Add(ExprNode, ExprNode),
    /// Subtraction.
    Sub(ExprNode, ExprNode),
    /// Multiplication.
    Mul(ExprNode, ExprNode),
    /// Division.
    Div(ExprNode, ExprNode),
    /// Unary negation.
    Neg(ExprNode),
}

/// A shared AST node. `Arc` makes the tree cheap to clone during parsing and
/// resolution.
pub type ExprNode = Arc<Expr>;

impl Expr {
    /// Wraps this expression in an [`ExprNode`].
    pub fn node(self) -> ExprNode {
        Arc::new(self)
    }
}

/// A relational operator used in constraint strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    LessEqual,
    GreaterEqual,
    Equal,
}

/// A parsed constraint: a relation between two expression subtrees.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub relation: Relation,
    pub lhs: ExprNode,
    pub rhs: ExprNode,
}
