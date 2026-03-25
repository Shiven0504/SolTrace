//! Evaluator for the SolTrace filter DSL.
//!
//! Takes a parsed `Expr` and evaluates it against a `TransferContext` (a map of
//! field names to values). Returns `true` if the transfer matches the filter.

use std::collections::HashMap;

use crate::parser::{CompareOp, Condition, Expr, Value};

/// A transfer context provides field values for evaluation.
/// Fields are looked up by name (e.g., "program", "amount", "direction").
#[derive(Debug, Clone)]
pub struct TransferContext {
    pub fields: HashMap<String, FieldValue>,
}

/// A typed field value that supports comparison operations.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    String(String),
    Int(i64),
    Null,
}

impl TransferContext {
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    pub fn set_str(&mut self, key: &str, value: impl Into<String>) {
        self.fields
            .insert(key.to_string(), FieldValue::String(value.into()));
    }

    pub fn set_int(&mut self, key: &str, value: i64) {
        self.fields
            .insert(key.to_string(), FieldValue::Int(value));
    }

    pub fn set_null(&mut self, key: &str) {
        self.fields
            .insert(key.to_string(), FieldValue::Null);
    }

    /// Build a context from a typical transfer record.
    pub fn from_transfer(
        signature: &str,
        slot: i64,
        program_id: &str,
        source: &str,
        dest: &str,
        mint: Option<&str>,
        amount: i64,
        direction: &str,
        wallet: &str,
    ) -> Self {
        let mut ctx = Self::new();
        ctx.set_str("signature", signature);
        ctx.set_int("slot", slot);
        ctx.set_str("program", program_id);
        ctx.set_str("program_id", program_id);
        ctx.set_str("source", source);
        ctx.set_str("source_account", source);
        ctx.set_str("dest", dest);
        ctx.set_str("dest_account", dest);
        ctx.set_int("amount", amount);
        ctx.set_str("direction", direction);
        ctx.set_str("wallet", wallet);
        if let Some(m) = mint {
            ctx.set_str("mint", m);
        } else {
            ctx.set_null("mint");
        }
        ctx
    }
}

impl Default for TransferContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluate an expression against a transfer context.
/// Returns `true` if the transfer matches the filter.
pub fn evaluate(expr: &Expr, ctx: &TransferContext) -> bool {
    match expr {
        Expr::Condition(cond) => evaluate_condition(cond, ctx),
        Expr::And(left, right) => evaluate(left, ctx) && evaluate(right, ctx),
        Expr::Or(left, right) => evaluate(left, ctx) || evaluate(right, ctx),
        Expr::Not(inner) => !evaluate(inner, ctx),
    }
}

fn evaluate_condition(cond: &Condition, ctx: &TransferContext) -> bool {
    let field_val = ctx.fields.get(&cond.field);

    // If the field doesn't exist in the context, only Neq matches (NULL != something)
    let Some(field_val) = field_val else {
        return cond.op == CompareOp::Neq;
    };

    // Handle NULL field values
    if *field_val == FieldValue::Null {
        return cond.op == CompareOp::Neq;
    }

    match (&cond.value, field_val) {
        // String comparisons
        (Value::String(filter_val), FieldValue::String(field_str)) => {
            compare_strings(field_str, filter_val, cond.op)
        }
        // Int comparisons
        (Value::Int(filter_val), FieldValue::Int(field_int)) => {
            compare_ints(*field_int, *filter_val, cond.op)
        }
        // Ident on right side treated as string
        (Value::Ident(filter_val), FieldValue::String(field_str)) => {
            compare_strings(field_str, filter_val, cond.op)
        }
        // String filter against int field: try parsing
        (Value::String(filter_val), FieldValue::Int(field_int)) => {
            if let Ok(parsed) = filter_val.parse::<i64>() {
                compare_ints(*field_int, parsed, cond.op)
            } else {
                false
            }
        }
        // Int filter against string field: try parsing
        (Value::Int(filter_val), FieldValue::String(field_str)) => {
            if let Ok(parsed) = field_str.parse::<i64>() {
                compare_ints(parsed, *filter_val, cond.op)
            } else {
                false
            }
        }
        // Ident against int
        (Value::Ident(filter_val), FieldValue::Int(field_int)) => {
            if let Ok(parsed) = filter_val.parse::<i64>() {
                compare_ints(*field_int, parsed, cond.op)
            } else {
                false
            }
        }
        _ => false,
    }
}

fn compare_strings(field: &str, filter: &str, op: CompareOp) -> bool {
    match op {
        CompareOp::Eq => field == filter,
        CompareOp::Neq => field != filter,
        CompareOp::Gt => field > filter,
        CompareOp::Lt => field < filter,
        CompareOp::Gte => field >= filter,
        CompareOp::Lte => field <= filter,
    }
}

fn compare_ints(field: i64, filter: i64, op: CompareOp) -> bool {
    match op {
        CompareOp::Eq => field == filter,
        CompareOp::Neq => field != filter,
        CompareOp::Gt => field > filter,
        CompareOp::Lt => field < filter,
        CompareOp::Gte => field >= filter,
        CompareOp::Lte => field <= filter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn eval_query(input: &str, ctx: &TransferContext) -> bool {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let query = parser.parse().unwrap();
        evaluate(&query.filter, ctx)
    }

    fn sample_ctx() -> TransferContext {
        TransferContext::from_transfer(
            "5xSig123",
            200_000_000,
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "SourceAcct111",
            "DestAcct222",
            Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
            1_500_000_000,
            "deposit",
            "WalletPubkey999",
        )
    }

    #[test]
    fn simple_string_match() {
        let ctx = sample_ctx();
        assert!(eval_query(
            r#"TRACK transfers WHERE direction = "deposit""#,
            &ctx
        ));
        assert!(!eval_query(
            r#"TRACK transfers WHERE direction = "withdrawal""#,
            &ctx
        ));
    }

    #[test]
    fn numeric_comparison() {
        let ctx = sample_ctx();
        assert!(eval_query(
            "TRACK transfers WHERE amount > 1000000000",
            &ctx
        ));
        assert!(!eval_query(
            "TRACK transfers WHERE amount > 2000000000",
            &ctx
        ));
        assert!(eval_query(
            "TRACK transfers WHERE amount >= 1500000000",
            &ctx
        ));
        assert!(eval_query(
            "TRACK transfers WHERE amount = 1500000000",
            &ctx
        ));
    }

    #[test]
    fn and_filter() {
        let ctx = sample_ctx();
        assert!(eval_query(
            r#"TRACK transfers WHERE direction = "deposit" AND amount > 1000000000"#,
            &ctx
        ));
        assert!(!eval_query(
            r#"TRACK transfers WHERE direction = "withdrawal" AND amount > 1000000000"#,
            &ctx
        ));
    }

    #[test]
    fn or_filter() {
        let ctx = sample_ctx();
        assert!(eval_query(
            r#"TRACK transfers WHERE direction = "withdrawal" OR amount > 1000000000"#,
            &ctx
        ));
        assert!(!eval_query(
            r#"TRACK transfers WHERE direction = "withdrawal" OR amount > 9999999999"#,
            &ctx
        ));
    }

    #[test]
    fn not_filter() {
        let ctx = sample_ctx();
        assert!(eval_query(
            r#"TRACK transfers WHERE NOT direction = "withdrawal""#,
            &ctx
        ));
        assert!(!eval_query(
            r#"TRACK transfers WHERE NOT direction = "deposit""#,
            &ctx
        ));
    }

    #[test]
    fn complex_nested_filter() {
        let ctx = sample_ctx();
        assert!(eval_query(
            r#"TRACK transfers WHERE (direction = "deposit" OR direction = "withdrawal") AND amount > 1000000000"#,
            &ctx,
        ));
    }

    #[test]
    fn program_id_match() {
        let ctx = sample_ctx();
        assert!(eval_query(
            r#"TRACK transfers WHERE program = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA""#,
            &ctx
        ));
    }

    #[test]
    fn mint_match() {
        let ctx = sample_ctx();
        assert!(eval_query(
            r#"TRACK transfers WHERE mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v""#,
            &ctx
        ));
    }

    #[test]
    fn null_field_neq() {
        let mut ctx = TransferContext::new();
        ctx.set_null("mint");
        ctx.set_str("direction", "deposit");

        // NULL mint != "USDC" should be true
        assert!(eval_query(
            r#"TRACK transfers WHERE mint != "USDC""#,
            &ctx
        ));
        // NULL mint = "USDC" should be false
        assert!(!eval_query(
            r#"TRACK transfers WHERE mint = "USDC""#,
            &ctx
        ));
    }

    #[test]
    fn missing_field() {
        let ctx = TransferContext::new();
        // Unknown field → Neq returns true, Eq returns false
        assert!(eval_query(
            r#"TRACK transfers WHERE unknown_field != "x""#,
            &ctx
        ));
        assert!(!eval_query(
            r#"TRACK transfers WHERE unknown_field = "x""#,
            &ctx
        ));
    }
}
