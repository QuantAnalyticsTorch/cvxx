//! Recursive-descent parser for expression strings.
//!
//! Grammar (whitespace is ignored):
//!
//! ```text
//! expr     := add_sub
//! add_sub  := mul_div (('+' | '-') mul_div)*
//! mul_div  := unary (('*' | '/') unary)*
//! unary    := '-' unary | primary
//! primary  := number | identifier | '(' expr ')'
//! number   := decimal integer or float, optional leading '-'
//! identifier := sequence of letters, digits, underscores, or dots; must not
//!               start with a digit or dot
//! ```

use crate::analytics::ast::{Expr, ExprNode};
use crate::core::error::CvxError;

/// Parses an expression string into an AST.
pub fn parse(input: &str) -> Result<ExprNode, CvxError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(&tokens);
    let expr = parser.parse_expr()?;
    parser.expect_eof()?;
    Ok(expr)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Identifier(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, CvxError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            _ if c.is_ascii_digit() || c == '.' => {
                let start = i;
                let mut seen_dot = c == '.';
                i += 1;
                while i < chars.len() {
                    let ch = chars[i];
                    if ch.is_ascii_digit() {
                        i += 1;
                    } else if ch == '.' && !seen_dot {
                        seen_dot = true;
                        i += 1;
                    } else {
                        break;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                let value = text
                    .parse::<f64>()
                    .map_err(|_| CvxError::InvalidExpression(format!("invalid number '{text}'")))?;
                tokens.push(Token::Number(value));
            }
            _ if c.is_alphabetic() || c == '_' => {
                let start = i;
                i += 1;
                while i < chars.len() {
                    let ch = chars[i];
                    if ch.is_alphanumeric() || ch == '_' || ch == '.' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                tokens.push(Token::Identifier(text));
            }
            _ => {
                return Err(CvxError::InvalidExpression(format!(
                    "unexpected character '{c}'"
                )));
            }
        }
    }

    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.pos);
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, expected: Token) -> Result<(), CvxError> {
        match self.advance() {
            Some(token) if *token == expected => Ok(()),
            _ => Err(CvxError::InvalidExpression(format!(
                "expected {:?}",
                expected
            ))),
        }
    }

    fn expect_eof(&self) -> Result<(), CvxError> {
        if self.pos < self.tokens.len() {
            return Err(CvxError::InvalidExpression(
                "unexpected token at end of expression".to_string(),
            ));
        }
        Ok(())
    }

    fn parse_expr(&mut self) -> Result<ExprNode, CvxError> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> Result<ExprNode, CvxError> {
        let mut left = self.parse_mul_div()?;
        while let Some(token) = self.peek() {
            match token {
                Token::Plus => {
                    self.advance();
                    let right = self.parse_mul_div()?;
                    left = Expr::Add(left, right).node();
                }
                Token::Minus => {
                    self.advance();
                    let right = self.parse_mul_div()?;
                    left = Expr::Sub(left, right).node();
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_mul_div(&mut self) -> Result<ExprNode, CvxError> {
        let mut left = self.parse_unary()?;
        while let Some(token) = self.peek() {
            match token {
                Token::Star => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::Mul(left, right).node();
                }
                Token::Slash => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::Div(left, right).node();
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<ExprNode, CvxError> {
        match self.peek() {
            Some(Token::Minus) => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::Neg(operand).node())
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<ExprNode, CvxError> {
        match self.advance() {
            Some(Token::Number(value)) => Ok(Expr::Constant(*value).node()),
            Some(Token::Identifier(name)) => Ok(Expr::Identifier(name.clone()).node()),
            Some(Token::LParen) => {
                let inner = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(inner)
            }
            _ => Err(CvxError::InvalidExpression(
                "expected number, identifier, or '('".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant(value: f64) -> ExprNode {
        Expr::Constant(value).node()
    }

    fn ident(name: &str) -> ExprNode {
        Expr::Identifier(name.to_string()).node()
    }

    #[test]
    fn parses_scalar_constant() {
        let expr = parse("42").unwrap();
        assert_eq!(expr, constant(42.0));
    }

    #[test]
    fn parses_float_constant() {
        let expr = parse("2.5").unwrap();
        assert_eq!(expr, constant(2.5));
    }

    #[test]
    fn parses_identifier() {
        let expr = parse("x").unwrap();
        assert_eq!(expr, ident("x"));
    }

    #[test]
    fn parses_addition() {
        let expr = parse("x + y").unwrap();
        assert_eq!(expr, Expr::Add(ident("x"), ident("y")).node());
    }

    #[test]
    fn parses_operator_precedence() {
        let expr = parse("x + y * z").unwrap();
        assert_eq!(
            expr,
            Expr::Add(ident("x"), Expr::Mul(ident("y"), ident("z")).node()).node()
        );
    }

    #[test]
    fn parses_parentheses() {
        let expr = parse("(x + y) * z").unwrap();
        assert_eq!(
            expr,
            Expr::Mul(Expr::Add(ident("x"), ident("y")).node(), ident("z")).node()
        );
    }

    #[test]
    fn parses_unary_minus() {
        let expr = parse("-x").unwrap();
        assert_eq!(expr, Expr::Neg(ident("x")).node());
    }

    #[test]
    fn parses_unary_minus_with_precedence() {
        let expr = parse("-x * y").unwrap();
        assert_eq!(
            expr,
            Expr::Mul(Expr::Neg(ident("x")).node(), ident("y")).node()
        );
    }

    #[test]
    fn rejects_invalid_syntax() {
        assert!(parse("x +").is_err());
        assert!(parse("(x + y").is_err());
    }

    #[test]
    fn rejects_empty_expression() {
        assert!(parse("").is_err());
    }
}
