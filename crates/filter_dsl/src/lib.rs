//! SolTrace Filter DSL — a small query language for filtering indexed transfers.
//!
//! Example:
//! ```text
//! TRACK transfers WHERE program = "TokenkegQ..." AND amount > 1_000_000_000
//! TRACK transfers WHERE direction = "deposit" AND (mint = "USDC" OR mint = "SOL")
//! TRACK transfers WHERE NOT direction = "withdrawal"
//! ```
//!
//! # Usage
//! ```
//! use soltrace_filter_dsl::{compile, evaluate, TransferContext};
//!
//! let filter = compile(r#"TRACK transfers WHERE amount > 1000"#).unwrap();
//! let mut ctx = TransferContext::new();
//! ctx.set_int("amount", 5000);
//! ctx.set_str("direction", "deposit");
//! assert!(evaluate(&filter.filter, &ctx));
//! ```

pub mod evaluator;
pub mod lexer;
pub mod parser;

pub use evaluator::{evaluate, FieldValue, TransferContext};
pub use parser::{CompareOp, Condition, Expr, FilterQuery, Value};

use lexer::Lexer;
use parser::Parser;

/// Compile a DSL query string into a `FilterQuery`.
///
/// # Errors
/// Returns an error if the input cannot be lexed or parsed.
pub fn compile(input: &str) -> Result<FilterQuery, String> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize().map_err(|e| e.to_string())?;
    let mut parser = Parser::new(tokens);
    parser.parse().map_err(|e| e.to_string())
}

/// Compile and immediately evaluate a DSL query against a context.
///
/// # Errors
/// Returns an error if compilation fails.
pub fn matches(input: &str, ctx: &TransferContext) -> Result<bool, String> {
    let query = compile(input)?;
    Ok(evaluate(&query.filter, ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_and_evaluate() {
        let ctx = TransferContext::from_transfer(
            "sig123",
            100,
            "11111111111111111111111111111111",
            "src",
            "dst",
            None,
            5_000_000_000,
            "deposit",
            "wallet1",
        );

        assert!(matches(
            r#"TRACK transfers WHERE direction = "deposit" AND amount > 1000000000"#,
            &ctx,
        )
        .unwrap());

        assert!(!matches(
            r#"TRACK transfers WHERE direction = "withdrawal""#,
            &ctx,
        )
        .unwrap());
    }

    #[test]
    fn compile_error_on_invalid_input() {
        let result = compile("this is not valid DSL %%");
        assert!(result.is_err());
    }
}
