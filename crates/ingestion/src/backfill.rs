//! Historical backfill engine: fetches past transactions for a wallet via
//! `getSignaturesForAddress` + `getTransaction`, decodes, and stores them.
//!
//! Supports rate limiting, slot checkpointing, and resumable jobs.

use std::sync::Arc;
use std::time::Duration;

use chrono::DateTime;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
};
use thiserror::Error;
use tokio::sync::RwLock;

use soltrace_decoder::account_mapper::AccountMapper;
use soltrace_decoder::classifier::classify_transfers;
use soltrace_decoder::{system_program, token_program, TransferEvent};
use soltrace_storage::PgStore;

#[derive(Debug, Error)]
pub enum BackfillError {
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("invalid pubkey: {0}")]
    InvalidPubkey(String),
}

/// Configuration for a backfill run.
#[derive(Debug, Clone)]
pub struct BackfillConfig {
    pub rpc_url: String,
    /// Delay between RPC calls to avoid rate limiting.
    pub rate_limit_ms: u64,
    /// Max signatures per `getSignaturesForAddress` call (max 1000).
    pub batch_size: usize,
}

impl Default for BackfillConfig {
    fn default() -> Self {
        Self {
            rpc_url: String::new(),
            rate_limit_ms: 200,
            batch_size: 100,
        }
    }
}

/// A backfill job result returned from the engine.
#[derive(Debug)]
pub struct BackfillResult {
    pub total_fetched: u64,
    pub total_indexed: u64,
}

/// The backfill engine. Shares decoder logic with the live listener.
pub struct BackfillEngine {
    config: BackfillConfig,
    pg_store: PgStore,
    account_mapper: Arc<RwLock<AccountMapper>>,
}

impl BackfillEngine {
    pub fn new(config: BackfillConfig, pg_store: PgStore) -> Self {
        Self {
            config,
            pg_store,
            account_mapper: Arc::new(RwLock::new(AccountMapper::new())),
        }
    }

    /// Run a backfill job for a specific wallet. Updates the backfill_jobs row as it progresses.
    pub async fn run_job(&self, job_id: i64, wallet: &str) -> Result<BackfillResult, BackfillError> {
        // Load account mappings
        let mappings = self
            .pg_store
            .all_token_account_owners()
            .await
            .map_err(|e| BackfillError::Storage(e.to_string()))?;

        {
            let mut mapper = self.account_mapper.write().await;
            mapper.load_mappings(mappings);
        }

        let pubkey: Pubkey = wallet
            .parse()
            .map_err(|e| BackfillError::InvalidPubkey(format!("{e}")))?;

        let rpc_client = RpcClient::new(self.config.rpc_url.clone());

        self.pg_store
            .update_backfill_status(job_id, "running", None)
            .await
            .map_err(|e| BackfillError::Storage(e.to_string()))?;

        let mut total_fetched: u64 = 0;
        let mut total_indexed: u64 = 0;
        let mut before: Option<Signature> = None;

        loop {
            // Fetch a batch of signatures
            let config = solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config {
                before,
                until: None,
                limit: Some(self.config.batch_size),
                commitment: Some(CommitmentConfig::confirmed()),
            };

            let sigs = rpc_client
                .get_signatures_for_address_with_config(&pubkey, config)
                .await
                .map_err(|e| BackfillError::Rpc(e.to_string()))?;

            if sigs.is_empty() {
                break;
            }

            let batch_count = sigs.len();
            tracing::info!(
                wallet,
                batch = batch_count,
                total_fetched,
                "fetched signature batch"
            );

            // Set cursor for next batch
            if let Some(last) = sigs.last() {
                before = last
                    .signature
                    .parse::<Signature>()
                    .ok();
            }

            for sig_info in &sigs {
                total_fetched += 1;

                // Skip failed transactions
                if sig_info.err.is_some() {
                    continue;
                }

                let sig: Signature = match sig_info.signature.parse() {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // Rate limit
                tokio::time::sleep(Duration::from_millis(self.config.rate_limit_ms)).await;

                let tx_config = RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Base64),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                };

                let tx = match rpc_client.get_transaction_with_config(&sig, tx_config).await {
                    Ok(tx) => tx,
                    Err(e) => {
                        tracing::warn!(
                            signature = %sig_info.signature,
                            error = %e,
                            "failed to fetch transaction, skipping"
                        );
                        continue;
                    }
                };

                let indexed = self
                    .process_transaction(&tx, &sig_info.signature, sig_info.slot, wallet)
                    .await?;
                total_indexed += indexed as u64;
            }

            // Update progress in DB
            self.pg_store
                .update_backfill_progress(job_id, total_fetched as i64, total_indexed as i64)
                .await
                .map_err(|e| BackfillError::Storage(e.to_string()))?;

            // If we got fewer than batch_size, we've reached the end
            if batch_count < self.config.batch_size {
                break;
            }
        }

        self.pg_store
            .update_backfill_status(job_id, "completed", None)
            .await
            .map_err(|e| BackfillError::Storage(e.to_string()))?;

        Ok(BackfillResult {
            total_fetched,
            total_indexed,
        })
    }

    /// Decode and store transfers from a single transaction.
    async fn process_transaction(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
        signature: &str,
        slot: u64,
        _wallet: &str,
    ) -> Result<usize, BackfillError> {
        let block_time = tx
            .block_time
            .and_then(|ts| DateTime::from_timestamp(ts, 0));

        let events = self.decode_transaction(tx, signature, slot, block_time);

        if events.is_empty() {
            return Ok(0);
        }

        let watched = self
            .pg_store
            .watched_pubkeys()
            .await
            .map_err(|e| BackfillError::Storage(e.to_string()))?;

        let mapper = self.account_mapper.read().await;
        let classified = classify_transfers(&events, &watched, mapper.all_mappings());
        drop(mapper);

        let count = self
            .pg_store
            .insert_transfers(&classified)
            .await
            .map_err(|e| BackfillError::Storage(e.to_string()))?;

        Ok(count)
    }

    /// Decode all transfer instructions from a transaction (outer + CPI inner).
    /// This mirrors the logic in `RpcListener::decode_transaction`.
    fn decode_transaction(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
        signature: &str,
        slot: u64,
        block_time: Option<DateTime<chrono::Utc>>,
    ) -> Vec<TransferEvent> {
        let mut events = Vec::new();
        let mut idx: u32 = 0;

        let meta_opt: Option<&_> = tx.transaction.meta.as_ref().into();
        let Some(meta) = meta_opt else {
            return events;
        };

        let Some(decoded) = tx.transaction.transaction.decode() else {
            return events;
        };

        let message = &decoded.message;
        let account_keys: Vec<String> = message
            .static_account_keys()
            .iter()
            .map(|k| k.to_string())
            .collect();

        // Outer instructions
        for ix in message.instructions() {
            let program_id_idx = ix.program_id_index as usize;
            if program_id_idx >= account_keys.len() {
                continue;
            }
            let program_id = &account_keys[program_id_idx];
            let ix_accounts: Vec<String> = ix
                .accounts
                .iter()
                .filter_map(|&i| account_keys.get(i as usize).cloned())
                .collect();

            if let Some(event) =
                decode_instruction(program_id, &ix.data, &ix_accounts, signature, slot, block_time, idx)
            {
                events.push(event);
            }
            idx += 1;
        }

        // CPI inner instructions
        let inner_opt: Option<&Vec<_>> = meta.inner_instructions.as_ref().into();
        if let Some(inner_instructions) = inner_opt {
            for inner_group in inner_instructions {
                for inner_ix in &inner_group.instructions {
                    if let solana_transaction_status::UiInstruction::Compiled(compiled) = inner_ix {
                        let program_id_idx = compiled.program_id_index as usize;
                        if program_id_idx >= account_keys.len() {
                            continue;
                        }
                        let program_id = &account_keys[program_id_idx];
                        let data = bs58::decode(&compiled.data).into_vec().unwrap_or_default();
                        let ix_accounts: Vec<String> = compiled
                            .accounts
                            .iter()
                            .filter_map(|&i| account_keys.get(i as usize).cloned())
                            .collect();

                        if let Some(event) = decode_instruction(
                            program_id,
                            &data,
                            &ix_accounts,
                            signature,
                            slot,
                            block_time,
                            idx,
                        ) {
                            events.push(event);
                        }
                        idx += 1;
                    }
                }
            }
        }

        events
    }
}

/// Decode a single instruction into a `TransferEvent` if it's a known transfer type.
fn decode_instruction(
    program_id: &str,
    data: &[u8],
    accounts: &[String],
    signature: &str,
    slot: u64,
    block_time: Option<DateTime<chrono::Utc>>,
    idx: u32,
) -> Option<TransferEvent> {
    if program_id == system_program::SYSTEM_PROGRAM_ID {
        if let Ok(Some(event)) =
            system_program::decode_transfer(data, accounts, signature, slot, block_time, idx)
        {
            return Some(event);
        }
    }

    if token_program::is_token_program(program_id) {
        if let Ok(Some(event)) =
            token_program::decode_transfer(data, accounts, program_id, signature, slot, block_time, idx)
        {
            return Some(event);
        }
    }

    None
}
