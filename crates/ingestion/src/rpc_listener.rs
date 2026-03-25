//! WebSocket subscriptions to Solana RPC with reconnect + exponential backoff.
//!
//! Subscribes to `logsSubscribe` for all transactions mentioning watched programs,
//! fetches full transaction details via RPC, decodes transfers, and persists them.

use std::sync::Arc;
use std::time::Duration;

use chrono::DateTime;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_transaction_status::UiTransactionEncoding;
use thiserror::Error;
use tokio::sync::{watch, RwLock};

use soltrace_decoder::account_mapper::AccountMapper;
use soltrace_decoder::classifier::classify_transfers;
use soltrace_decoder::{system_program, token_program, TransferEvent};
use soltrace_storage::{PgStore, RedisCache};

#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("WebSocket connection failed: {0}")]
    ConnectionFailed(String),
    #[error("RPC error: {0}")]
    RpcError(String),
    #[error("subscription ended")]
    SubscriptionEnded,
    #[error("storage error: {0}")]
    Storage(String),
}

/// Configuration for the RPC listener.
#[derive(Debug, Clone)]
pub struct ListenerConfig {
    pub rpc_url: String,
    pub ws_url: String,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

/// The main RPC WebSocket listener.
pub struct RpcListener {
    config: ListenerConfig,
    pg_store: PgStore,
    redis_cache: RedisCache,
    account_mapper: Arc<RwLock<AccountMapper>>,
    /// Receives notifications when watched wallets change.
    wallet_changed_rx: watch::Receiver<()>,
}

impl RpcListener {
    pub fn new(
        config: ListenerConfig,
        pg_store: PgStore,
        redis_cache: RedisCache,
        wallet_changed_rx: watch::Receiver<()>,
    ) -> Self {
        Self {
            config,
            pg_store,
            redis_cache,
            account_mapper: Arc::new(RwLock::new(AccountMapper::new())),
            wallet_changed_rx,
        }
    }

    /// Load existing token account mappings from the database.
    async fn load_account_mappings(&self) -> Result<(), IngestionError> {
        let mappings = self
            .pg_store
            .all_token_account_owners()
            .await
            .map_err(|e| IngestionError::Storage(e.to_string()))?;

        tracing::info!(count = mappings.len(), "loaded token account mappings from DB");

        let mut mapper = self.account_mapper.write().await;
        mapper.load_mappings(mappings);
        Ok(())
    }

    /// Run the listener loop with automatic reconnection.
    pub async fn run(&self) -> Result<(), IngestionError> {
        self.load_account_mappings().await?;

        let mut backoff_ms = self.config.initial_backoff_ms;

        loop {
            tracing::info!(ws_url = %self.config.ws_url, "connecting to Solana WebSocket");

            match self.subscribe_and_process().await {
                Ok(()) => {
                    tracing::warn!("subscription ended cleanly, reconnecting...");
                    backoff_ms = self.config.initial_backoff_ms;
                }
                Err(e) => {
                    tracing::error!(error = %e, backoff_ms, "listener error, reconnecting after backoff");
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms * 2).min(self.config.max_backoff_ms);
                }
            }
        }
    }

    /// Subscribe to logs and process incoming transactions.
    async fn subscribe_and_process(&self) -> Result<(), IngestionError> {
        let rpc_client = RpcClient::new(self.config.rpc_url.clone());

        // Get watched wallets to build the subscription filter
        let watched = self
            .pg_store
            .watched_pubkeys()
            .await
            .map_err(|e| IngestionError::Storage(e.to_string()))?;

        if watched.is_empty() {
            tracing::info!("no watched wallets yet, waiting for wallet registration...");
            // Wait until a wallet is added
            let mut rx = self.wallet_changed_rx.clone();
            rx.changed()
                .await
                .map_err(|e| IngestionError::Storage(e.to_string()))?;
            // Return Ok to trigger reconnect loop, which will re-enter with wallets loaded
            return Ok(());
        }

        let wallet_list: Vec<String> = watched.into_iter().collect();
        tracing::info!(wallets = ?wallet_list, "subscribing to logs for watched wallets");

        let pubsub = PubsubClient::new(&self.config.ws_url)
            .await
            .map_err(|e| IngestionError::ConnectionFailed(e.to_string()))?;

        let config = RpcTransactionLogsConfig {
            commitment: Some(CommitmentConfig::confirmed()),
        };

        // Subscribe only to transactions mentioning our watched wallets
        let filter = RpcTransactionLogsFilter::Mentions(vec![wallet_list[0].clone()]);

        let (mut stream, _unsub) = pubsub
            .logs_subscribe(filter, config)
            .await
            .map_err(|e| IngestionError::ConnectionFailed(e.to_string()))?;

        tracing::info!(wallet = %wallet_list[0], "subscribed to logsSubscribe (mentions)");

        // Also listen for wallet changes to trigger re-subscription
        let mut wallet_rx = self.wallet_changed_rx.clone();

        use futures::StreamExt;
        loop {
            tokio::select! {
                msg = stream.next() => {
                    let Some(log_response) = msg else {
                        return Err(IngestionError::SubscriptionEnded);
                    };

                    let signature_str = &log_response.value.signature;
                    let slot = log_response.context.slot;

                    // Skip failed transactions
                    if log_response.value.err.is_some() {
                        continue;
                    }

                    // Fetch full transaction details
                    match self.fetch_and_process_tx(&rpc_client, signature_str, slot).await {
                        Ok(count) => {
                            if count > 0 {
                                tracing::info!(signature = %signature_str, slot, transfers = count, "indexed transfers");
                            }
                            // Update slot cursor
                            if let Err(e) = self.redis_cache.set_last_slot(slot).await {
                                tracing::warn!(error = %e, "failed to update slot cursor");
                            }
                        }
                        Err(e) => {
                            tracing::warn!(signature = %signature_str, error = %e, "failed to process transaction");
                        }
                    }
                }
                _ = wallet_rx.changed() => {
                    tracing::info!("watched wallets changed, re-subscribing...");
                    // Return Ok to trigger reconnect loop with updated wallet list
                    return Ok(());
                }
            }
        }
    }

    /// Fetch a full transaction by signature and process it.
    /// Retries a few times since the TX may not be queryable immediately after log notification.
    async fn fetch_and_process_tx(
        &self,
        rpc_client: &RpcClient,
        signature_str: &str,
        slot: u64,
    ) -> Result<usize, IngestionError> {
        let sig = signature_str
            .parse()
            .map_err(|e| IngestionError::RpcError(format!("invalid signature: {e}")))?;

        let config = solana_client::rpc_config::RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::Base64),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };

        // Retry up to 3 times with delays — TX may not be available immediately
        let mut last_err = String::new();
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            match rpc_client.get_transaction_with_config(&sig, config).await {
                Ok(tx) => {
                    return self.process_fetched_tx(&tx, signature_str, slot).await;
                }
                Err(e) => {
                    last_err = e.to_string();
                    tracing::debug!(
                        signature = %signature_str,
                        attempt = attempt + 1,
                        error = %last_err,
                        "retrying transaction fetch"
                    );
                }
            }
        }

        Err(IngestionError::RpcError(last_err))
    }

    /// Process a successfully fetched transaction.
    async fn process_fetched_tx(
        &self,
        tx: &solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta,
        signature_str: &str,
        slot: u64,
    ) -> Result<usize, IngestionError> {

        let block_time = tx.block_time.map(|ts| {
            DateTime::from_timestamp(ts, 0).unwrap_or_default()
        });

        // Extract transfer events from the transaction
        let events = self
            .decode_transaction(&tx, signature_str, slot, block_time)
            .await;

        if events.is_empty() {
            return Ok(0);
        }

        // Classify against watched wallets
        let watched = self
            .pg_store
            .watched_pubkeys()
            .await
            .map_err(|e| IngestionError::Storage(e.to_string()))?;

        if watched.is_empty() {
            return Ok(0);
        }

        let mapper = self.account_mapper.read().await;
        let classified = classify_transfers(&events, &watched, mapper.all_mappings());
        drop(mapper);

        // Persist
        let count = self
            .pg_store
            .insert_transfers(&classified)
            .await
            .map_err(|e| IngestionError::Storage(e.to_string()))?;

        Ok(count)
    }

    /// Decode all transfer instructions from a transaction (outer + CPI inner).
    async fn decode_transaction(
        &self,
        tx: &solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta,
        signature: &str,
        slot: u64,
        block_time: Option<DateTime<chrono::Utc>>,
    ) -> Vec<TransferEvent> {
        let mut events = Vec::new();
        let mut idx: u32 = 0;

        // Get the decoded transaction
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
            .map(|k: &solana_sdk::pubkey::Pubkey| k.to_string())
            .collect();

        // Process outer instructions
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

            if let Some(event) = self.decode_instruction(
                program_id,
                &ix.data,
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

        // Process CPI inner instructions (critical: ~60-70% of real deposits)
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
                        let data = bs58::decode(&compiled.data)
                            .into_vec()
                            .unwrap_or_default();
                        let ix_accounts: Vec<String> = compiled
                            .accounts
                            .iter()
                            .filter_map(|&i| account_keys.get(i as usize).cloned())
                            .collect();

                        if let Some(event) = self.decode_instruction(
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

    /// Decode a single instruction, returning a `TransferEvent` if it's a known transfer.
    fn decode_instruction(
        &self,
        program_id: &str,
        data: &[u8],
        accounts: &[String],
        signature: &str,
        slot: u64,
        block_time: Option<DateTime<chrono::Utc>>,
        idx: u32,
    ) -> Option<TransferEvent> {
        if program_id == system_program::SYSTEM_PROGRAM_ID {
            match system_program::decode_transfer(data, accounts, signature, slot, block_time, idx) {
                Ok(Some(event)) => return Some(event),
                Ok(None) => {}
                Err(e) => {
                    tracing::trace!(error = %e, "system program decode error");
                }
            }
        }

        if token_program::is_token_program(program_id) {
            match token_program::decode_transfer(
                data, accounts, program_id, signature, slot, block_time, idx,
            ) {
                Ok(Some(event)) => return Some(event),
                Ok(None) => {}
                Err(e) => {
                    tracing::trace!(error = %e, "token program decode error");
                }
            }
        }

        None
    }
}
