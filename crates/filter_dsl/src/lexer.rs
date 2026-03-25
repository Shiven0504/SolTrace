//! Lexer for the SolTrace filter DSL.
//!
//! Tokenizes input like:
//!   `TRACK transfers WHERE program = "TokenkegQ..." AND amount > 1000000000`
//!
//! Token types: keywords (TRACK, WHERE, AND, OR, NOT), identifiers (field names),
//! operators (=, !=, >, <, >=, <=), literals (strings, integers).

use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Track,
    Where,
    And,
    Or,
    Not,

    // Identifiers (field names like `program`, `amount`, `direction`)
    Ident(String),

    // Literals
    StringLit(String),
    IntLit(i64),

    // Operators
    Eq,       // =
    Neq,      // !=
    Gt,       // >
    Lt,       // <
    Gte,      // >=
    Lte,      // <=

    // Delimiters
    LParen,
    RParen,

    // End of input
    Eof,
}

#[derive(Debug, Error)]
pub enum LexError {
    #[error("unexpected character '{0}' at position {1}")]
    UnexpectedChar(char, usize),
    #[error("unterminated string literal starting at position {0}")]
    UnterminatedString(usize),
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    /// Tokenize the entire input into a list of tokens.
    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        loop {
            let tok = self.next_token()?;
            let is_eof = tok == Token::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }

        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        self.pos += 1;
        ch
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(|c| c.is_whitespace()) {
            self.advance();
        }
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace();

        let Some(ch) = self.peek() else {
            return Ok(Token::Eof);
        };

        match ch {
            '(' => {
                self.advance();
                Ok(Token::LParen)
            }
            ')' => {
                self.advance();
                Ok(Token::RParen)
            }
            '=' => {
                self.advance();
                Ok(Token::Eq)
            }
            '!' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::Neq)
                } else {
                    Err(LexError::UnexpectedChar('!', self.pos - 1))
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::Gte)
                } else {
                    Ok(Token::Gt)
                }
            }
            '<' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::Lte)
                } else {
                    Ok(Token::Lt)
                }
            }
            '"' => self.read_string(),
            c if c.is_ascii_digit() => self.read_number(),
            c if c.is_ascii_alphabetic() || c == '_' => self.read_ident_or_keyword(),
            _ => {
                let pos = self.pos;
                self.advance();
                Err(LexError::UnexpectedChar(ch, pos))
            }
        }
    }

    fn read_string(&mut self) -> Result<Token, LexError> {
        let start = self.pos;
        self.advance(); // consume opening quote

        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => return Ok(Token::StringLit(s)),
                Some('\\') => {
                    // Simple escape handling
                    match self.advance() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('\\') => s.push('\\'),
                        Some('"') => s.push('"'),
                        Some(c) => {
                            s.push('\\');
                            s.push(c);
                        }
                        None => return Err(LexError::UnterminatedString(start)),
                    }
                }
                Some(c) => s.push(c),
                None => return Err(LexError::UnterminatedString(start)),
            }
        }
    }

    fn read_number(&mut self) -> Result<Token, LexError> {
        let mut num_str = String::new();
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
            let c = self.advance().unwrap();
            if c != '_' {
                num_str.push(c);
            }
        }
        // Safe to unwrap: we only collected digits
        let val: i64 = num_str.parse().unwrap_or(0);
        Ok(Token::IntLit(val))
    }

    fn read_ident_or_keyword(&mut self) -> Result<Token, LexError> {
        let mut ident = String::new();
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            ident.push(self.advance().unwrap());
        }

        // Match keywords (case-insensitive)
        let token = match ident.to_uppercase().as_str() {
            "TRACK" => Token::Track,
            "WHERE" => Token::Where,
            "AND" => Token::And,
            "OR" => Token::Or,
            "NOT" => Token::Not,
            _ => Token::Ident(ident),
        };

        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_query() {
        let mut lexer = Lexer::new(r#"TRACK transfers WHERE program = "TokenkegQ""#);
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Track,
                Token::Ident("transfers".into()),
                Token::Where,
                Token::Ident("program".into()),
                Token::Eq,
                Token::StringLit("TokenkegQ".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn tokenize_numeric_and_operators() {
        let mut lexer = Lexer::new("TRACK transfers WHERE amount > 1000 AND direction = \"deposit\"");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Track,
                Token::Ident("transfers".into()),
                Token::Where,
                Token::Ident("amount".into()),
                Token::Gt,
                Token::IntLit(1000),
                Token::And,
                Token::Ident("direction".into()),
                Token::Eq,
                Token::StringLit("deposit".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn tokenize_all_operators() {
        let mut lexer = Lexer::new("= != > < >= <=");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Eq,
                Token::Neq,
                Token::Gt,
                Token::Lt,
                Token::Gte,
                Token::Lte,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn tokenize_case_insensitive_keywords() {
        let mut lexer = Lexer::new("track transfers where amount = 1 and direction = \"deposit\"");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::Track);
        assert_eq!(tokens[2], Token::Where);
        assert_eq!(tokens[6], Token::And);
    }

    #[test]
    fn unterminated_string_error() {
        let mut lexer = Lexer::new(r#"TRACK transfers WHERE program = "unterminated"#);
        let result = lexer.tokenize();
        assert!(result.is_err());
    }

    #[test]
    fn tokenize_underscore_numbers() {
        let mut lexer = Lexer::new("1_000_000_000");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens[0], Token::IntLit(1_000_000_000));
    }

    #[test]
    fn tokenize_parenthesized_expression() {
        let mut lexer = Lexer::new("TRACK transfers WHERE (amount > 100 OR amount < 10)");
        let tokens = lexer.tokenize().unwrap();
        assert!(tokens.contains(&Token::LParen));
        assert!(tokens.contains(&Token::RParen));
        assert!(tokens.contains(&Token::Or));
    }
}
