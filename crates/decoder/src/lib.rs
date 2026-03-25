//! Transaction decoding and event classification for Solana blockchain data.
//!
//! Parses System Program (native SOL) and SPL Token Program instructions,
//! merges CPI inner instructions, and classifies transfers as deposits or withdrawals.

pub mod account_mapper;
pub mod classifier;
pub mod idl_decoder;
pub mod system_program;
pub mod token_program;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A decoded transfer event ready for storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferEvent {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<DateTime<Utc>>,
    pub instruction_idx: u32,
    pub program_id: String,
    pub source_account: String,
    pub dest_account: String,
    /// `None` for native SOL transfers.
    pub mint: Option<String>,
    pub amount: u64,
}

/// Direction of a transfer relative to a watched wallet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Deposit,
    Withdrawal,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deposit => write!(f, "deposit"),
            Self::Withdrawal => write!(f, "withdrawal"),
        }
    }
}

/// A classified transfer event with direction and wallet context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedTransfer {
    pub event: TransferEvent,
    pub direction: Direction,
    pub wallet: String,
}

#[derive(Debug, Error)]
pub enum DecoderError {
    #[error("failed to decode instruction: {0}")]
    DecodeError(String),
    #[error("unsupported program: {0}")]
    UnsupportedProgram(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}
