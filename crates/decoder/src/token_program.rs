//! Decodes SPL Token Program instructions: Transfer and TransferChecked.

use crate::{DecoderError, TransferEvent};
use chrono::{DateTime, Utc};

/// SPL Token Program ID.
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// SPL Token 2022 Program ID.
pub const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

/// SPL Token instruction discriminants.
const TRANSFER: u8 = 3;
const TRANSFER_CHECKED: u8 = 12;

/// Returns true if the given program ID is an SPL Token program.
pub fn is_token_program(program_id: &str) -> bool {
    program_id == TOKEN_PROGRAM_ID || program_id == TOKEN_2022_PROGRAM_ID
}

/// Attempt to decode an SPL Token instruction as a transfer.
///
/// Returns `None` if the instruction is not a Transfer or TransferChecked.
///
/// # Account layout
/// - Transfer: `[source, dest, authority, ...]`
/// - TransferChecked: `[source, mint, dest, authority, ...]`
pub fn decode_transfer(
    data: &[u8],
    accounts: &[String],
    program_id: &str,
    signature: &str,
    slot: u64,
    block_time: Option<DateTime<Utc>>,
    instruction_idx: u32,
) -> Result<Option<TransferEvent>, DecoderError> {
    if data.is_empty() {
        return Ok(None);
    }

    match data[0] {
        TRANSFER => decode_spl_transfer(data, accounts, program_id, signature, slot, block_time, instruction_idx),
        TRANSFER_CHECKED => decode_spl_transfer_checked(data, accounts, program_id, signature, slot, block_time, instruction_idx),
        _ => Ok(None),
    }
}

/// Decode SPL Token `Transfer` (discriminant 3).
/// Data layout: `[discriminant(1), amount(8)]`
/// Accounts: `[source, destination, authority, ...]`
fn decode_spl_transfer(
    data: &[u8],
    accounts: &[String],
    program_id: &str,
    signature: &str,
    slot: u64,
    block_time: Option<DateTime<Utc>>,
    instruction_idx: u32,
) -> Result<Option<TransferEvent>, DecoderError> {
    if data.len() < 9 {
        return Err(DecoderError::DecodeError(
            "SPL Transfer instruction too short".into(),
        ));
    }

    let amount = u64::from_le_bytes(
        data[1..9]
            .try_into()
            .map_err(|e| DecoderError::DecodeError(format!("invalid amount: {e}")))?,
    );

    if accounts.len() < 2 {
        return Err(DecoderError::DecodeError(
            "SPL Transfer requires at least 2 accounts".into(),
        ));
    }

    Ok(Some(TransferEvent {
        signature: signature.to_string(),
        slot,
        block_time,
        instruction_idx,
        program_id: program_id.to_string(),
        source_account: accounts[0].clone(),
        dest_account: accounts[1].clone(),
        mint: None, // Transfer doesn't include mint; resolved later via account mapping
        amount,
    }))
}

/// Decode SPL Token `TransferChecked` (discriminant 12).
/// Data layout: `[discriminant(1), amount(8), decimals(1)]`
/// Accounts: `[source, mint, destination, authority, ...]`
fn decode_spl_transfer_checked(
    data: &[u8],
    accounts: &[String],
    program_id: &str,
    signature: &str,
    slot: u64,
    block_time: Option<DateTime<Utc>>,
    instruction_idx: u32,
) -> Result<Option<TransferEvent>, DecoderError> {
    if data.len() < 10 {
        return Err(DecoderError::DecodeError(
            "SPL TransferChecked instruction too short".into(),
        ));
    }

    let amount = u64::from_le_bytes(
        data[1..9]
            .try_into()
            .map_err(|e| DecoderError::DecodeError(format!("invalid amount: {e}")))?,
    );

    if accounts.len() < 3 {
        return Err(DecoderError::DecodeError(
            "SPL TransferChecked requires at least 3 accounts".into(),
        ));
    }

    Ok(Some(TransferEvent {
        signature: signature.to_string(),
        slot,
        block_time,
        instruction_idx,
        program_id: program_id.to_string(),
        source_account: accounts[0].clone(),
        dest_account: accounts[2].clone(), // mint is accounts[1]
        mint: Some(accounts[1].clone()),
        amount,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_spl_transfer() {
        let mut data = vec![TRANSFER];
        data.extend_from_slice(&500_000u64.to_le_bytes());

        let accounts = vec![
            "SourceTokenAccount".to_string(),
            "DestTokenAccount".to_string(),
            "Authority".to_string(),
        ];

        let result = decode_transfer(
            &data, &accounts, TOKEN_PROGRAM_ID, "sig1", 200, None, 0,
        )
        .unwrap()
        .unwrap();

        assert_eq!(result.amount, 500_000);
        assert_eq!(result.source_account, "SourceTokenAccount");
        assert_eq!(result.dest_account, "DestTokenAccount");
        assert!(result.mint.is_none()); // Transfer doesn't carry mint
    }

    #[test]
    fn test_decode_spl_transfer_checked() {
        let mut data = vec![TRANSFER_CHECKED];
        data.extend_from_slice(&1_000_000u64.to_le_bytes());
        data.push(6); // decimals

        let accounts = vec![
            "SourceTokenAccount".to_string(),
            "MintAddress".to_string(),
            "DestTokenAccount".to_string(),
            "Authority".to_string(),
        ];

        let result = decode_transfer(
            &data, &accounts, TOKEN_PROGRAM_ID, "sig2", 300, None, 1,
        )
        .unwrap()
        .unwrap();

        assert_eq!(result.amount, 1_000_000);
        assert_eq!(result.source_account, "SourceTokenAccount");
        assert_eq!(result.dest_account, "DestTokenAccount");
        assert_eq!(result.mint.as_deref(), Some("MintAddress"));
    }

    #[test]
    fn test_unknown_instruction_returns_none() {
        let data = vec![99]; // unknown discriminant
        let accounts = vec!["A".to_string()];
        let result = decode_transfer(&data, &accounts, TOKEN_PROGRAM_ID, "sig", 1, None, 0).unwrap();
        assert!(result.is_none());
    }
}
