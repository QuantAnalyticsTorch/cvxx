//! Convex optimization modeling: expression AST, parser, and conversion to
//! `cvxrust` expressions.

pub mod ast;
pub mod parser;
pub mod resolve;
pub mod shape;

pub use ast::{Constraint, Expr, ExprNode, Relation};
pub use parser::{parse, parse_constraint};
pub use resolve::{resolve_constraint, resolve_expr, ResolvedConstraint, ResolvedExpr};
pub use shape::{infer_shape, render_expression};
