//! Webhook dispatcher: matches indexed transfers against registered webhooks
//! and sends HTTP POST callbacks with optional HMAC signing.

use std::time::Duration;

use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use soltrace_decoder::ClassifiedTransfer;

use crate::postgres::{PgStore, WebhookRow};

type HmacSha256 = Hmac<Sha256>;

/// Payload sent to webhook URLs.
#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    pub event: String,
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<String>,
    pub wallet: String,
    pub direction: String,
    pub source_account: String,
    pub dest_account: String,
    pub mint: Option<String>,
    pub amount: u64,
    pub program_id: String,
}

impl WebhookPayload {
    pub fn from_transfer(ct: &ClassifiedTransfer) -> Self {
        Self {
            event: "transfer".into(),
            signature: ct.event.signature.clone(),
            slot: ct.event.slot,
            block_time: ct.event.block_time.map(|t| t.to_rfc3339()),
            wallet: ct.wallet.clone(),
            direction: ct.direction.to_string(),
            source_account: ct.event.source_account.clone(),
            dest_account: ct.event.dest_account.clone(),
            mint: ct.event.mint.clone(),
            amount: ct.event.amount,
            program_id: ct.event.program_id.clone(),
        }
    }
}

/// The webhook dispatcher. Call `dispatch` after indexing transfers.
pub struct WebhookDispatcher {
    pg_store: PgStore,
    http_client: reqwest::Client,
}

impl WebhookDispatcher {
    pub fn new(pg_store: PgStore) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");

        Self {
            pg_store,
            http_client,
        }
    }

    /// Dispatch webhooks for a list of newly indexed transfers.
    /// This is fire-and-forget; failures are logged but don't block indexing.
    pub async fn dispatch(&self, transfers: &[ClassifiedTransfer]) {
        for ct in transfers {
            if let Err(e) = self.dispatch_single(ct).await {
                tracing::warn!(
                    signature = %ct.event.signature,
                    error = %e,
                    "webhook dispatch error"
                );
            }
        }
    }

    async fn dispatch_single(&self, ct: &ClassifiedTransfer) -> Result<(), String> {
        let hooks = self
            .pg_store
            .matching_webhooks(
                &ct.wallet,
                &ct.direction.to_string(),
                ct.event.amount as i64,
                ct.event.mint.as_deref(),
            )
            .await
            .map_err(|e| e.to_string())?;

        if hooks.is_empty() {
            return Ok(());
        }

        let payload = WebhookPayload::from_transfer(ct);
        let payload_json =
            serde_json::to_string(&payload).map_err(|e| e.to_string())?;

        for hook in &hooks {
            let result = self.send_webhook(hook, &payload_json).await;

            // Log delivery (best-effort, don't fail on logging errors)
            let (status_code, response_body) = match &result {
                Ok((code, body)) => (Some(*code), Some(body.as_str())),
                Err(e) => {
                    tracing::warn!(
                        webhook_id = hook.id,
                        url = %hook.url,
                        error = %e,
                        "webhook delivery failed"
                    );
                    (None, None)
                }
            };

            // We don't have transfer_id here easily, so we log with 0
            // In production, you'd pass the DB-assigned transfer ID
            let _ = self
                .pg_store
                .log_webhook_delivery(hook.id, 0, status_code, response_body, 1)
                .await;
        }

        Ok(())
    }

    async fn send_webhook(
        &self,
        hook: &WebhookRow,
        payload_json: &str,
    ) -> Result<(i32, String), String> {
        let mut request = self
            .http_client
            .post(&hook.url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "SolTrace-Webhook/0.1");

        // Add HMAC signature if secret is configured
        if let Some(ref secret) = hook.secret {
            let mut mac =
                HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| e.to_string())?;
            mac.update(payload_json.as_bytes());
            let signature = hex::encode(mac.finalize().into_bytes());
            request = request.header("X-SolTrace-Signature", signature);
        }

        let response = request
            .body(payload_json.to_string())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = response.status().as_u16() as i32;
        let body = response.text().await.unwrap_or_default();

        Ok((status, body))
    }
}
