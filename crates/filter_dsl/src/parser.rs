//! Parser for the SolTrace filter DSL.
//!
//! Grammar:
//! ```text
//! query      = "TRACK" ident "WHERE" expr
//! expr       = and_expr
//! and_expr   = or_expr ("AND" or_expr)*
//! or_expr    = unary_expr ("OR" unary_expr)*
//! unary_expr = "NOT" unary_expr | primary
//! primary    = condition | "(" expr ")"
//! condition  = ident op value
//! op         = "=" | "!=" | ">" | "<" | ">=" | "<="
//! value      = STRING | INT | ident
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::lexer::Token;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("expected {expected}, got {got:?}")]
    Expected { expected: String, got: Token },
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("unexpected token: {0:?}")]
    UnexpectedToken(Token),
}

/// A complete parsed filter query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterQuery {
    /// The resource being tracked (e.g., "transfers").
    pub resource: String,
    /// The filter expression tree.
    pub filter: Expr,
}

/// Expression AST node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    /// field op value
    Condition(Condition),
    /// left AND right
    And(Box<Expr>, Box<Expr>),
    /// left OR right
    Or(Box<Expr>, Box<Expr>),
    /// NOT expr
    Not(Box<Expr>),
}

/// A single comparison condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub op: CompareOp,
    pub value: Value,
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOp {
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
}

impl std::fmt::Display for CompareOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eq => write!(f, "="),
            Self::Neq => write!(f, "!="),
            Self::Gt => write!(f, ">"),
            Self::Lt => write!(f, "<"),
            Self::Gte => write!(f, ">="),
            Self::Lte => write!(f, "<="),
        }
    }
}

/// A literal value in a condition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
    Ident(String),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Int(n) => write!(f, "{n}"),
            Self::Ident(s) => write!(f, "{s}"),
        }
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse a complete filter query.
    pub fn parse(&mut self) -> Result<FilterQuery, ParseError> {
        // Optional TRACK keyword
        if self.peek() == Some(&Token::Track) {
            self.advance();
        }

        // Resource name (e.g., "transfers")
        let resource = self.expect_ident()?;

        // WHERE keyword
        self.expect_token(&Token::Where)?;

        // Filter expression
        let filter = self.parse_expr()?;

        // Should be at EOF
        if self.peek() != Some(&Token::Eof) && self.peek().is_some() {
            return Err(ParseError::UnexpectedToken(self.peek().unwrap().clone()));
        }

        Ok(FilterQuery { resource, filter })
    }

    /// Parse a standalone expression (without TRACK/WHERE prefix).
    /// Useful for evaluating filter conditions directly.
    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_and()
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_or()?;

        while self.peek() == Some(&Token::And) {
            self.advance();
            let right = self.parse_or()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;

        while self.peek() == Some(&Token::Or) {
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.peek() == Some(&Token::Not) {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(Expr::Not(Box::new(expr)));
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        if self.peek() == Some(&Token::LParen) {
            self.advance();
            let expr = self.parse_expr()?;
            self.expect_token(&Token::RParen)?;
            return Ok(expr);
        }

        self.parse_condition()
    }

    fn parse_condition(&mut self) -> Result<Expr, ParseError> {
        let field = self.expect_ident()?;
        let op = self.parse_op()?;
        let value = self.parse_value()?;

        Ok(Expr::Condition(Condition { field, op, value }))
    }

    fn parse_op(&mut self) -> Result<CompareOp, ParseError> {
        let tok = self.advance_or_eof()?;
        match tok {
            Token::Eq => Ok(CompareOp::Eq),
            Token::Neq => Ok(CompareOp::Neq),
            Token::Gt => Ok(CompareOp::Gt),
            Token::Lt => Ok(CompareOp::Lt),
            Token::Gte => Ok(CompareOp::Gte),
            Token::Lte => Ok(CompareOp::Lte),
            other => Err(ParseError::Expected {
                expected: "operator (=, !=, >, <, >=, <=)".into(),
                got: other,
            }),
        }
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        let tok = self.advance_or_eof()?;
        match tok {
            Token::StringLit(s) => Ok(Value::String(s)),
            Token::IntLit(n) => Ok(Value::Int(n)),
            Token::Ident(s) => Ok(Value::Ident(s)),
            other => Err(ParseError::Expected {
                expected: "value (string, integer, or identifier)".into(),
                got: other,
            }),
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let tok = self.advance_or_eof()?;
        match tok {
            Token::Ident(s) => Ok(s),
            other => Err(ParseError::Expected {
                expected: "identifier".into(),
                got: other,
            }),
        }
    }

    fn expect_token(&mut self, expected: &Token) -> Result<(), ParseError> {
        let tok = self.advance_or_eof()?;
        if std::mem::discriminant(&tok) == std::mem::discriminant(expected) {
            Ok(())
        } else {
            Err(ParseError::Expected {
                expected: format!("{expected:?}"),
                got: tok,
            })
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        self.pos += 1;
        tok
    }

    fn advance_or_eof(&mut self) -> Result<Token, ParseError> {
        self.advance().ok_or(ParseError::UnexpectedEof)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_query(input: &str) -> Result<FilterQuery, ParseError> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    #[test]
    fn parse_simple_equality() {
        let query = parse_query(r#"TRACK transfers WHERE program = "TokenkegQ""#).unwrap();
        assert_eq!(query.resource, "transfers");

        if let Expr::Condition(c) = &query.filter {
            assert_eq!(c.field, "program");
            assert_eq!(c.op, CompareOp::Eq);
            assert_eq!(c.value, Value::String("TokenkegQ".into()));
        } else {
            panic!("expected Condition");
        }
    }

    #[test]
    fn parse_and_expression() {
        let query = parse_query(
            r#"TRACK transfers WHERE program = "TokenkegQ" AND amount > 1000000000"#,
        )
        .unwrap();

        if let Expr::And(left, right) = &query.filter {
            assert!(matches!(left.as_ref(), Expr::Condition(_)));
            if let Expr::Condition(c) = right.as_ref() {
                assert_eq!(c.field, "amount");
                assert_eq!(c.op, CompareOp::Gt);
                assert_eq!(c.value, Value::Int(1_000_000_000));
            } else {
                panic!("expected Condition on right");
            }
        } else {
            panic!("expected And");
        }
    }

    #[test]
    fn parse_or_expression() {
        let query = parse_query(
            r#"TRACK transfers WHERE direction = "deposit" OR direction = "withdrawal""#,
        )
        .unwrap();

        assert!(matches!(&query.filter, Expr::Or(_, _)));
    }

    #[test]
    fn parse_not_expression() {
        let query =
            parse_query(r#"TRACK transfers WHERE NOT direction = "withdrawal""#).unwrap();

        if let Expr::Not(inner) = &query.filter {
            assert!(matches!(inner.as_ref(), Expr::Condition(_)));
        } else {
            panic!("expected Not");
        }
    }

    #[test]
    fn parse_nested_parentheses() {
        let query = parse_query(
            r#"TRACK transfers WHERE (amount > 100 OR amount < 10) AND direction = "deposit""#,
        )
        .unwrap();

        if let Expr::And(left, _right) = &query.filter {
            assert!(matches!(left.as_ref(), Expr::Or(_, _)));
        } else {
            panic!("expected And with Or on left");
        }
    }

    #[test]
    fn parse_triple_and() {
        let query = parse_query(
            r#"TRACK transfers WHERE program = "sys" AND amount > 0 AND direction = "deposit""#,
        )
        .unwrap();

        // AND is left-associative: (program AND amount) AND direction
        if let Expr::And(left, right) = &query.filter {
            assert!(matches!(left.as_ref(), Expr::And(_, _)));
            assert!(matches!(right.as_ref(), Expr::Condition(_)));
        } else {
            panic!("expected nested And");
        }
    }

    #[test]
    fn missing_where_keyword() {
        let result = parse_query(r#"TRACK transfers program = "x""#);
        assert!(result.is_err());
    }
}
