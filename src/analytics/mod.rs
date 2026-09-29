//! Convex optimization modeling: expression AST, parser, and conversion to
//! `cvxrust` expressions.

pub mod ast;
pub mod parser;
pub mod resolve;

pub use ast::{Expr, ExprNode};
pub use parser::parse;
pub use resolve::{resolve_expr, ResolvedExpr};
