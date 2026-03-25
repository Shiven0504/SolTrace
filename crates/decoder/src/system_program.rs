//! Decodes native SOL transfer instructions from the System Program.

use crate::{DecoderError, TransferEvent};
use chrono::{DateTime, Utc};

/// The System Program ID.
pub const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

/// System instruction discriminants we care about.
const TRANSFER_DISCRIMINANT: u32 = 2;

/// Attempt to decode a System Program instruction as a SOL transfer.
///
/// Returns `None` if the instruction is not a transfer.
pub fn decode_transfer(
    data: &[u8],
    accounts: &[String],
    signature: &str,
    slot: u64,
    block_time: Option<DateTime<Utc>>,
    instruction_idx: u32,
) -> Result<Option<TransferEvent>, DecoderError> {
    if data.len() < 4 {
        return Ok(None);
    }

    let discriminant = u32::from_le_bytes(
        data[..4]
            .try_into()
            .map_err(|e| DecoderError::DecodeError(format!("invalid discriminant: {e}")))?,
    );

    if discriminant != TRANSFER_DISCRIMINANT {
        return Ok(None);
    }

    if data.len() < 12 {
        return Err(DecoderError::DecodeError(
            "transfer instruction too short".into(),
        ));
    }

    let lamports = u64::from_le_bytes(
        data[4..12]
            .try_into()
            .map_err(|e| DecoderError::DecodeError(format!("invalid lamports: {e}")))?,
    );

    if accounts.len() < 2 {
        return Err(DecoderError::DecodeError(
            "transfer requires at least 2 accounts".into(),
        ));
    }

    Ok(Some(TransferEvent {
        signature: signature.to_string(),
        slot,
        block_time,
        instruction_idx,
        program_id: SYSTEM_PROGRAM_ID.to_string(),
        source_account: accounts[0].clone(),
        dest_account: accounts[1].clone(),
        mint: None, // native SOL
        amount: lamports,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_sol_transfer() {
        // discriminant = 2 (transfer), lamports = 1_000_000_000 (1 SOL)
        let mut data = Vec::new();
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&1_000_000_000u64.to_le_bytes());

        let accounts = vec![
            "SenderPubkey111111111111111111111111111111".to_string(),
            "ReceiverPubkey11111111111111111111111111111".to_string(),
        ];

        let result = decode_transfer(&data, &accounts, "txsig123", 100, None, 0)
            .unwrap()
            .unwrap();

        assert_eq!(result.amount, 1_000_000_000);
        assert_eq!(result.source_account, accounts[0]);
        assert_eq!(result.dest_account, accounts[1]);
        assert!(result.mint.is_none());
    }

    #[test]
    fn test_non_transfer_returns_none() {
        // discriminant = 0 (CreateAccount)
        let mut data = Vec::new();
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 48]);

        let accounts = vec!["A".to_string(), "B".to_string()];
        let result = decode_transfer(&data, &accounts, "sig", 1, None, 0).unwrap();
        assert!(result.is_none());
    }
}
