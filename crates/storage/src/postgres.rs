//! PostgreSQL operations: wallet registration, transfer inserts, balance queries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use soltrace_decoder::ClassifiedTransfer;
use sqlx::postgres::PgPool;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PgError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
}

/// PostgreSQL storage handle.
#[derive(Debug, Clone)]
pub struct PgStore {
    pool: PgPool,
}

/// A watched wallet row.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct WatchedWallet {
    pub wallet_pubkey: String,
    pub label: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub user_id: Option<i64>,
}

/// A token account row.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TokenAccount {
    pub token_account: String,
    pub owner_wallet: String,
    pub mint: String,
    pub balance: i64,
    pub last_slot: i64,
}

/// A token transfer row.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TokenTransferRow {
    pub id: i64,
    pub signature: String,
    pub slot: i64,
    pub block_time: Option<DateTime<Utc>>,
    pub instruction_idx: i32,
    pub program_id: String,
    pub source_account: String,
    pub dest_account: String,
    pub mint: Option<String>,
    pub amount: i64,
    pub direction: String,
    pub wallet: String,
}

/// Query filters for listing transfers.
#[derive(Debug, Default)]
pub struct TransferQuery {
    pub wallet: Option<String>,
    pub direction: Option<String>,
    pub mint: Option<String>,
    pub limit: i64,
    pub offset: i64,
    pub user_id: Option<i64>,
}

/// Balance summary for a wallet.
#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceEntry {
    pub mint: Option<String>,
    pub balance: i64,
}

/// A backfill job row.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BackfillJob {
    pub id: i64,
    pub wallet: String,
    pub status: String,
    pub before_sig: Option<String>,
    pub until_sig: Option<String>,
    pub total_fetched: i64,
    pub total_indexed: i64,
    pub error_message: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// A webhook registration row.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WebhookRow {
    pub id: i64,
    pub url: String,
    pub secret: Option<String>,
    pub wallet: Option<String>,
    pub direction: Option<String>,
    pub min_amount: Option<i64>,
    pub mint: Option<String>,
    pub active: bool,
    pub created_at: Option<DateTime<Utc>>,
}

/// Input for creating a new webhook.
#[derive(Debug, Deserialize)]
pub struct NewWebhook {
    pub url: String,
    pub secret: Option<String>,
    pub wallet: Option<String>,
    pub direction: Option<String>,
    pub min_amount: Option<i64>,
    pub mint: Option<String>,
}

/// An uploaded program IDL row.
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProgramIdl {
    pub program_id: String,
    pub idl_json: serde_json::Value,
    pub name: Option<String>,
    pub version: Option<String>,
    pub uploaded_at: Option<DateTime<Utc>>,
}

/// A user row.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub password_hash: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub google_id: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

impl PgStore {
    /// Create a new store from an existing connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the inner pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ── Wallet operations ──────────────────────────────────────────

    /// Add a wallet to the watch list, scoped to a user.
    pub async fn add_wallet(&self, pubkey: &str, label: Option<&str>, user_id: i64) -> Result<WatchedWallet, PgError> {
        let row = sqlx::query_as::<_, WatchedWallet>(
            r#"INSERT INTO watched_wallets (wallet_pubkey, label, user_id)
               VALUES ($1, $2, $3)
               ON CONFLICT (wallet_pubkey) DO UPDATE
               SET label = COALESCE($2, watched_wallets.label),
                   user_id = $3
               RETURNING wallet_pubkey, label, created_at, user_id"#,
        )
        .bind(pubkey)
        .bind(label)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// List watched wallets. If `user_id` is Some, return only that user's wallets.
    /// If None, return all wallets.
    pub async fn list_wallets(&self, user_id: Option<i64>) -> Result<Vec<WatchedWallet>, PgError> {
        let rows = if let Some(uid) = user_id {
            sqlx::query_as::<_, WatchedWallet>(
                "SELECT wallet_pubkey, label, created_at, user_id FROM watched_wallets WHERE user_id = $1 ORDER BY created_at DESC",
            )
            .bind(uid)
            .fetch_all(&self.pool)
            .await?
        } else {
            // Unauthenticated: return empty list (wallets are user-scoped)
            Vec::new()
        };

        Ok(rows)
    }

    /// Delete a watched wallet owned by the given user (or unowned). Returns true if deleted.
    pub async fn delete_wallet(&self, pubkey: &str, user_id: i64) -> Result<bool, PgError> {
        let result = sqlx::query(
            "DELETE FROM watched_wallets WHERE wallet_pubkey = $1 AND (user_id = $2 OR user_id IS NULL)",
        )
        .bind(pubkey)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get all watched wallet pubkeys as a set.
    pub async fn watched_pubkeys(&self) -> Result<std::collections::HashSet<String>, PgError> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT wallet_pubkey FROM watched_wallets")
                .fetch_all(&self.pool)
                .await?;

        Ok(rows.into_iter().map(|(pk,)| pk).collect())
    }

    // ── Token account operations ───────────────────────────────────

    /// Upsert a token account mapping.
    pub async fn upsert_token_account(
        &self,
        token_account: &str,
        owner: &str,
        mint: &str,
        balance: i64,
        slot: i64,
    ) -> Result<(), PgError> {
        sqlx::query(
            r#"INSERT INTO token_accounts (token_account, owner_wallet, mint, balance, last_slot)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (token_account) DO UPDATE
               SET balance = $4, last_slot = $5
               WHERE token_accounts.last_slot <= $5"#,
        )
        .bind(token_account)
        .bind(owner)
        .bind(mint)
        .bind(balance)
        .bind(slot)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get all token accounts for a wallet.
    pub async fn get_token_accounts(&self, owner: &str) -> Result<Vec<TokenAccount>, PgError> {
        let rows = sqlx::query_as::<_, TokenAccount>(
            "SELECT token_account, owner_wallet, mint, balance, last_slot FROM token_accounts WHERE owner_wallet = $1",
        )
        .bind(owner)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get all token account → owner mappings for watched wallets.
    pub async fn all_token_account_owners(&self) -> Result<std::collections::HashMap<String, String>, PgError> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT token_account, owner_wallet FROM token_accounts")
                .fetch_all(&self.pool)
                .await?;

        Ok(rows.into_iter().collect())
    }

    // ── Transfer operations ────────────────────────────────────────

    /// Insert a classified transfer. Uses ON CONFLICT to deduplicate.
    pub async fn insert_transfer(&self, ct: &ClassifiedTransfer) -> Result<(), PgError> {
        sqlx::query(
            r#"INSERT INTO token_transfers
               (signature, slot, block_time, instruction_idx, program_id,
                source_account, dest_account, mint, amount, direction, wallet)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
               ON CONFLICT (signature, instruction_idx) DO NOTHING"#,
        )
        .bind(&ct.event.signature)
        .bind(ct.event.slot as i64)
        .bind(ct.event.block_time)
        .bind(ct.event.instruction_idx as i32)
        .bind(&ct.event.program_id)
        .bind(&ct.event.source_account)
        .bind(&ct.event.dest_account)
        .bind(&ct.event.mint)
        .bind(ct.event.amount as i64)
        .bind(ct.direction.to_string())
        .bind(&ct.wallet)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert multiple classified transfers in a batch.
    pub async fn insert_transfers(&self, transfers: &[ClassifiedTransfer]) -> Result<usize, PgError> {
        let mut count = 0;
        for ct in transfers {
            self.insert_transfer(ct).await?;
            count += 1;
        }
        Ok(count)
    }

    /// Query transfers with optional filters.
    pub async fn query_transfers(&self, q: &TransferQuery) -> Result<Vec<TokenTransferRow>, PgError> {
        // Unauthenticated: return empty list (transfers are user-scoped)
        let user_id = match q.user_id {
            Some(uid) => uid,
            None => return Ok(Vec::new()),
        };

        // Build dynamic query scoped to user's wallets
        let mut sql = String::from(
            "SELECT id, signature, slot, block_time, instruction_idx, program_id, \
             source_account, dest_account, mint, amount, direction, wallet \
             FROM token_transfers WHERE wallet IN \
             (SELECT wallet_pubkey FROM watched_wallets WHERE user_id = $1)",
        );
        let mut bind_idx = 2u32;
        let mut binds: Vec<String> = Vec::new();

        if let Some(ref wallet) = q.wallet {
            sql.push_str(&format!(" AND wallet = ${bind_idx}"));
            bind_idx += 1;
            binds.push(wallet.clone());
        }
        if let Some(ref direction) = q.direction {
            sql.push_str(&format!(" AND direction = ${bind_idx}"));
            bind_idx += 1;
            binds.push(direction.clone());
        }
        if let Some(ref mint) = q.mint {
            sql.push_str(&format!(" AND mint = ${bind_idx}"));
            bind_idx += 1;
            binds.push(mint.clone());
        }

        sql.push_str(" ORDER BY block_time DESC NULLS LAST, id DESC");
        sql.push_str(&format!(" LIMIT ${bind_idx}"));
        bind_idx += 1;
        sql.push_str(&format!(" OFFSET ${bind_idx}"));

        let mut query = sqlx::query_as::<_, TokenTransferRow>(&sql);
        query = query.bind(user_id);
        for b in &binds {
            query = query.bind(b);
        }
        query = query.bind(q.limit).bind(q.offset);

        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Look up a single transaction by signature.
    pub async fn get_transaction(&self, signature: &str) -> Result<Vec<TokenTransferRow>, PgError> {
        let rows = sqlx::query_as::<_, TokenTransferRow>(
            "SELECT id, signature, slot, block_time, instruction_idx, program_id, \
             source_account, dest_account, mint, amount, direction, wallet \
             FROM token_transfers WHERE signature = $1 ORDER BY instruction_idx",
        )
        .bind(signature)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get balances for a wallet (aggregated from token_accounts table).
    pub async fn get_balances(&self, wallet: &str) -> Result<Vec<BalanceEntry>, PgError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT mint, balance FROM token_accounts WHERE owner_wallet = $1 AND balance > 0",
        )
        .bind(wallet)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(mint, balance)| BalanceEntry { mint: Some(mint), balance })
            .collect())
    }

    /// Run the SQL migration file (supports multiple statements separated by `;`).
    pub async fn run_migrations(&self, sql: &str) -> Result<(), PgError> {
        for statement in sql.split(';') {
            // Strip leading SQL comment lines before checking if the chunk is empty
            let stripped: String = statement
                .lines()
                .filter(|line| !line.trim().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n");
            let trimmed = stripped.trim();
            if trimmed.is_empty() {
                continue;
            }
            sqlx::query(trimmed).execute(&self.pool).await?;
        }
        Ok(())
    }

    // ── Backfill job operations ────────────────────────────────────

    /// Create a new backfill job for a wallet.
    pub async fn create_backfill_job(&self, wallet: &str) -> Result<BackfillJob, PgError> {
        let row = sqlx::query_as::<_, BackfillJob>(
            r#"INSERT INTO backfill_jobs (wallet, status)
               VALUES ($1, 'pending')
               RETURNING id, wallet, status, before_sig, until_sig,
                         total_fetched, total_indexed, error_message, created_at, updated_at"#,
        )
        .bind(wallet)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get a backfill job by ID.
    pub async fn get_backfill_job(&self, job_id: i64) -> Result<Option<BackfillJob>, PgError> {
        let row = sqlx::query_as::<_, BackfillJob>(
            r#"SELECT id, wallet, status, before_sig, until_sig,
                      total_fetched, total_indexed, error_message, created_at, updated_at
               FROM backfill_jobs WHERE id = $1"#,
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// List backfill jobs, optionally filtered by wallet.
    pub async fn list_backfill_jobs(&self, wallet: Option<&str>) -> Result<Vec<BackfillJob>, PgError> {
        let rows = if let Some(w) = wallet {
            sqlx::query_as::<_, BackfillJob>(
                r#"SELECT id, wallet, status, before_sig, until_sig,
                          total_fetched, total_indexed, error_message, created_at, updated_at
                   FROM backfill_jobs WHERE wallet = $1 ORDER BY created_at DESC"#,
            )
            .bind(w)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, BackfillJob>(
                r#"SELECT id, wallet, status, before_sig, until_sig,
                          total_fetched, total_indexed, error_message, created_at, updated_at
                   FROM backfill_jobs ORDER BY created_at DESC"#,
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows)
    }

    /// Update a backfill job's status.
    pub async fn update_backfill_status(
        &self,
        job_id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<(), PgError> {
        sqlx::query(
            r#"UPDATE backfill_jobs
               SET status = $2, error_message = $3, updated_at = now()
               WHERE id = $1"#,
        )
        .bind(job_id)
        .bind(status)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update backfill job progress counters.
    pub async fn update_backfill_progress(
        &self,
        job_id: i64,
        total_fetched: i64,
        total_indexed: i64,
    ) -> Result<(), PgError> {
        sqlx::query(
            r#"UPDATE backfill_jobs
               SET total_fetched = $2, total_indexed = $3, updated_at = now()
               WHERE id = $1"#,
        )
        .bind(job_id)
        .bind(total_fetched)
        .bind(total_indexed)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ── Webhook operations ─────────────────────────────────────────

    /// Register a new webhook.
    pub async fn create_webhook(&self, hook: &NewWebhook) -> Result<WebhookRow, PgError> {
        let row = sqlx::query_as::<_, WebhookRow>(
            r#"INSERT INTO webhooks (url, secret, wallet, direction, min_amount, mint)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, url, secret, wallet, direction, min_amount, mint, active, created_at"#,
        )
        .bind(&hook.url)
        .bind(&hook.secret)
        .bind(&hook.wallet)
        .bind(&hook.direction)
        .bind(hook.min_amount)
        .bind(&hook.mint)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// List all active webhooks.
    pub async fn list_webhooks(&self) -> Result<Vec<WebhookRow>, PgError> {
        let rows = sqlx::query_as::<_, WebhookRow>(
            r#"SELECT id, url, secret, wallet, direction, min_amount, mint, active, created_at
               FROM webhooks ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get a specific webhook by ID.
    pub async fn get_webhook(&self, webhook_id: i64) -> Result<Option<WebhookRow>, PgError> {
        let row = sqlx::query_as::<_, WebhookRow>(
            r#"SELECT id, url, secret, wallet, direction, min_amount, mint, active, created_at
               FROM webhooks WHERE id = $1"#,
        )
        .bind(webhook_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Delete a webhook by ID.
    pub async fn delete_webhook(&self, webhook_id: i64) -> Result<bool, PgError> {
        let result = sqlx::query("DELETE FROM webhooks WHERE id = $1")
            .bind(webhook_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get all active webhooks that match a classified transfer.
    pub async fn matching_webhooks(
        &self,
        wallet: &str,
        direction: &str,
        amount: i64,
        mint: Option<&str>,
    ) -> Result<Vec<WebhookRow>, PgError> {
        let rows = sqlx::query_as::<_, WebhookRow>(
            r#"SELECT id, url, secret, wallet, direction, min_amount, mint, active, created_at
               FROM webhooks
               WHERE active = TRUE
                 AND (wallet IS NULL OR wallet = $1)
                 AND (direction IS NULL OR direction = $2)
                 AND (min_amount IS NULL OR $3 >= min_amount)
                 AND (mint IS NULL OR mint = $4)"#,
        )
        .bind(wallet)
        .bind(direction)
        .bind(amount)
        .bind(mint)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Log a webhook delivery attempt.
    pub async fn log_webhook_delivery(
        &self,
        webhook_id: i64,
        transfer_id: i64,
        status_code: Option<i32>,
        response_body: Option<&str>,
        attempt: i32,
    ) -> Result<(), PgError> {
        sqlx::query(
            r#"INSERT INTO webhook_deliveries (webhook_id, transfer_id, status_code, response_body, attempt)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(webhook_id)
        .bind(transfer_id)
        .bind(status_code)
        .bind(response_body)
        .bind(attempt)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ── IDL operations ─────────────────────────────────────────────

    /// Store or update an Anchor IDL for a program.
    pub async fn upsert_idl(
        &self,
        program_id: &str,
        idl_json: &serde_json::Value,
        name: Option<&str>,
        version: Option<&str>,
    ) -> Result<ProgramIdl, PgError> {
        let row = sqlx::query_as::<_, ProgramIdl>(
            r#"INSERT INTO program_idls (program_id, idl_json, name, version)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (program_id) DO UPDATE
               SET idl_json = $2, name = COALESCE($3, program_idls.name),
                   version = COALESCE($4, program_idls.version), uploaded_at = now()
               RETURNING program_id, idl_json, name, version, uploaded_at"#,
        )
        .bind(program_id)
        .bind(idl_json)
        .bind(name)
        .bind(version)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get the IDL for a program.
    pub async fn get_idl(&self, program_id: &str) -> Result<Option<ProgramIdl>, PgError> {
        let row = sqlx::query_as::<_, ProgramIdl>(
            r#"SELECT program_id, idl_json, name, version, uploaded_at
               FROM program_idls WHERE program_id = $1"#,
        )
        .bind(program_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// List all registered IDLs.
    pub async fn list_idls(&self) -> Result<Vec<ProgramIdl>, PgError> {
        let rows = sqlx::query_as::<_, ProgramIdl>(
            r#"SELECT program_id, idl_json, name, version, uploaded_at
               FROM program_idls ORDER BY uploaded_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Delete an IDL by program ID.
    pub async fn delete_idl(&self, program_id: &str) -> Result<bool, PgError> {
        let result = sqlx::query("DELETE FROM program_idls WHERE program_id = $1")
            .bind(program_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    // ── User operations ─────────────────────────────────────────────

    /// Create a new user. Caller is responsible for hashing the password.
    pub async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
    ) -> Result<UserRow, PgError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"INSERT INTO users (username, password_hash)
               VALUES ($1, $2)
               RETURNING id, username, password_hash, created_at, google_id, email, avatar_url"#,
        )
        .bind(username)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Find a user by username.
    pub async fn get_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserRow>, PgError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"SELECT id, username, password_hash, created_at, google_id, email, avatar_url
               FROM users WHERE username = $1"#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Find a user by ID.
    pub async fn get_user_by_id(&self, user_id: i64) -> Result<Option<UserRow>, PgError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"SELECT id, username, password_hash, created_at, google_id, email, avatar_url
               FROM users WHERE id = $1"#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Find or create a user by Google ID. Used for Google OAuth sign-in.
    /// Returns `(UserRow, is_new)` — `is_new` is true when the user was just created.
    pub async fn upsert_google_user(
        &self,
        google_id: &str,
        email: &str,
        username: &str,
        avatar_url: Option<&str>,
    ) -> Result<(UserRow, bool), PgError> {
        // Try to find existing user first
        let existing = sqlx::query_as::<_, UserRow>(
            r#"SELECT id, username, password_hash, created_at, google_id, email, avatar_url
               FROM users WHERE google_id = $1"#,
        )
        .bind(google_id)
        .fetch_optional(&self.pool)
        .await?;

        if existing.is_some() {
            // Update email and avatar (but NOT username — user may have customized it)
            let row = sqlx::query_as::<_, UserRow>(
                r#"UPDATE users SET email = $2, avatar_url = $3
                   WHERE google_id = $1
                   RETURNING id, username, password_hash, created_at, google_id, email, avatar_url"#,
            )
            .bind(google_id)
            .bind(email)
            .bind(avatar_url)
            .fetch_one(&self.pool)
            .await?;
            Ok((row, false))
        } else {
            // New user
            let row = sqlx::query_as::<_, UserRow>(
                r#"INSERT INTO users (google_id, email, username, avatar_url)
                   VALUES ($1, $2, $3, $4)
                   RETURNING id, username, password_hash, created_at, google_id, email, avatar_url"#,
            )
            .bind(google_id)
            .bind(email)
            .bind(username)
            .bind(avatar_url)
            .fetch_one(&self.pool)
            .await?;
            Ok((row, true))
        }
    }

    /// Update a user's username.
    pub async fn update_username(&self, user_id: i64, username: &str) -> Result<UserRow, PgError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"UPDATE users SET username = $2
               WHERE id = $1
               RETURNING id, username, password_hash, created_at, google_id, email, avatar_url"#,
        )
        .bind(user_id)
        .bind(username)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }
}
