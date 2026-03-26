//! Request handlers for transfers, balances, wallets, health, backfill, webhooks, and IDLs.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use soltrace_storage::postgres::{NewWebhook, TransferQuery};

use crate::auth::{AuthUser, OptionalAuthUser};
use crate::routes::AppState;

// ── Request/Response types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TransferParams {
    pub wallet: Option<String>,
    pub direction: Option<String>,
    pub mint: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceParams {
    pub wallet: String,
}

#[derive(Debug, Deserialize)]
pub struct AddWalletRequest {
    pub pubkey: String,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub last_slot: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn internal_error(msg: impl ToString) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: msg.to_string(),
        }),
    )
}

// ── Handlers ───────────────────────────────────────────────────────

/// `GET /transfers?wallet=X&direction=deposit&mint=Y&limit=50&offset=0`
pub async fn list_transfers(
    State(state): State<AppState>,
    auth: OptionalAuthUser,
    Query(params): Query<TransferParams>,
) -> impl IntoResponse {
    let query = TransferQuery {
        wallet: params.wallet,
        direction: params.direction,
        mint: params.mint,
        limit: params.limit.unwrap_or(50).min(1000),
        offset: params.offset.unwrap_or(0),
        user_id: auth.0.map(|c| c.sub),
    };

    match state.pg_store.query_transfers(&query).await {
        Ok(rows) => Ok(Json(rows)),
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /balances?wallet=X`
pub async fn get_balances(
    State(state): State<AppState>,
    Query(params): Query<BalanceParams>,
) -> impl IntoResponse {
    match state.pg_store.get_balances(&params.wallet).await {
        Ok(balances) => Ok(Json(balances)),
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /tx/:signature`
pub async fn get_transaction(
    State(state): State<AppState>,
    Path(signature): Path<String>,
) -> impl IntoResponse {
    match state.pg_store.get_transaction(&signature).await {
        Ok(rows) if rows.is_empty() => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "transaction not found".into(),
            }),
        )),
        Ok(rows) => Ok(Json(rows)),
        Err(e) => Err(internal_error(e)),
    }
}

/// `POST /wallets` — Add wallet to watch list (requires auth).
pub async fn add_wallet(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<AddWalletRequest>,
) -> impl IntoResponse {
    match state
        .pg_store
        .add_wallet(&body.pubkey, body.label.as_deref(), auth.0.sub)
        .await
    {
        Ok(wallet) => {
            // Notify ingestion listener to re-subscribe with the new wallet
            let _ = state.wallet_changed_tx.send(());
            Ok((StatusCode::CREATED, Json(wallet)))
        }
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /wallets` — List watched wallets. If authenticated, returns only user's wallets.
pub async fn list_wallets(
    State(state): State<AppState>,
    auth: OptionalAuthUser,
) -> impl IntoResponse {
    let user_id = auth.0.map(|c| c.sub);
    match state.pg_store.list_wallets(user_id).await {
        Ok(wallets) => Ok(Json(wallets)),
        Err(e) => Err(internal_error(e)),
    }
}

/// `DELETE /wallets/:pubkey` — Remove a wallet from the watch list (requires auth).
pub async fn delete_wallet(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(pubkey): Path<String>,
) -> impl IntoResponse {
    match state.pg_store.delete_wallet(&pubkey, auth.0.sub).await {
        Ok(true) => {
            let _ = state.wallet_changed_tx.send(());
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "wallet not found or not owned by you".into(),
            }),
        )),
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /health` — Indexer status, last processed slot, lag.
pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let last_slot = state.redis_cache.get_last_slot().await.unwrap_or(None);

    Json(HealthResponse {
        status: "ok".into(),
        last_slot,
    })
}

// ── Backfill handlers ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BackfillRequest {
    pub wallet: String,
}

#[derive(Debug, Deserialize)]
pub struct BackfillListParams {
    pub wallet: Option<String>,
}

/// `POST /backfill` — Start a backfill job for a wallet (requires auth).
pub async fn start_backfill(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(body): Json<BackfillRequest>,
) -> impl IntoResponse {
    // Verify wallet is being watched
    let watched = match state.pg_store.watched_pubkeys().await {
        Ok(w) => w,
        Err(e) => return Err(internal_error(e)),
    };

    if !watched.contains(&body.wallet) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "wallet is not in the watch list — add it via POST /wallets first".into(),
            }),
        ));
    }

    match state.pg_store.create_backfill_job(&body.wallet).await {
        Ok(job) => {
            let job_id = job.id;
            let wallet = body.wallet.clone();

            // Spawn the backfill in the background
            let pg_store = state.pg_store.clone();
            let backfill_config = state.backfill_config.clone();
            tokio::spawn(async move {
                let engine =
                    soltrace_ingestion::BackfillEngine::new(backfill_config, pg_store.clone());
                match engine.run_job(job_id, &wallet).await {
                    Ok(result) => {
                        tracing::info!(
                            job_id,
                            fetched = result.total_fetched,
                            indexed = result.total_indexed,
                            "backfill job completed"
                        );
                    }
                    Err(e) => {
                        tracing::error!(job_id, error = %e, "backfill job failed");
                        let _ = pg_store
                            .update_backfill_status(job_id, "failed", Some(&e.to_string()))
                            .await;
                    }
                }
            });

            Ok((StatusCode::CREATED, Json(job)))
        }
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /backfill` — List backfill jobs.
pub async fn list_backfill_jobs(
    State(state): State<AppState>,
    Query(params): Query<BackfillListParams>,
) -> impl IntoResponse {
    match state
        .pg_store
        .list_backfill_jobs(params.wallet.as_deref())
        .await
    {
        Ok(jobs) => Ok(Json(jobs)),
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /backfill/:id` — Get a specific backfill job.
pub async fn get_backfill_job(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.pg_store.get_backfill_job(id).await {
        Ok(Some(job)) => Ok(Json(job)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "backfill job not found".into(),
            }),
        )),
        Err(e) => Err(internal_error(e)),
    }
}

// ── Webhook handlers ──────────────────────────────────────────────

/// `POST /webhooks` — Register a webhook (requires auth).
pub async fn create_webhook(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(body): Json<NewWebhook>,
) -> impl IntoResponse {
    match state.pg_store.create_webhook(&body).await {
        Ok(hook) => Ok((StatusCode::CREATED, Json(hook))),
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /webhooks` — List all webhooks.
pub async fn list_webhooks(State(state): State<AppState>) -> impl IntoResponse {
    match state.pg_store.list_webhooks().await {
        Ok(hooks) => Ok(Json(hooks)),
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /webhooks/:id` — Get a specific webhook.
pub async fn get_webhook(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.pg_store.get_webhook(id).await {
        Ok(Some(hook)) => Ok(Json(hook)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "webhook not found".into(),
            }),
        )),
        Err(e) => Err(internal_error(e)),
    }
}

/// `DELETE /webhooks/:id` — Delete a webhook (requires auth).
pub async fn delete_webhook(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.pg_store.delete_webhook(id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "webhook not found".into(),
            }),
        )),
        Err(e) => Err(internal_error(e)),
    }
}

// ── IDL handlers ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UploadIdlRequest {
    pub program_id: String,
    pub idl: serde_json::Value,
}

/// `POST /idls` — Upload an Anchor IDL for dynamic decoding (requires auth).
pub async fn upload_idl(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(body): Json<UploadIdlRequest>,
) -> impl IntoResponse {
    // Extract name and version from the IDL JSON
    let name = body.idl.get("name").and_then(|v| v.as_str());
    let version = body.idl.get("version").and_then(|v| v.as_str());

    // Validate it parses as an Anchor IDL
    if serde_json::from_value::<soltrace_decoder::idl_decoder::AnchorIdl>(body.idl.clone()).is_err()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid Anchor IDL format".into(),
            }),
        ));
    }

    match state
        .pg_store
        .upsert_idl(&body.program_id, &body.idl, name, version)
        .await
    {
        Ok(idl) => {
            // Register in the live IDL registry
            if let Ok(parsed) = serde_json::from_value::<soltrace_decoder::idl_decoder::AnchorIdl>(
                body.idl.clone(),
            ) {
                let mut registry = state.idl_registry.write().await;
                registry.register(body.program_id.clone(), parsed);
            }
            Ok((StatusCode::CREATED, Json(idl)))
        }
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /idls` — List all registered IDLs.
pub async fn list_idls(State(state): State<AppState>) -> impl IntoResponse {
    match state.pg_store.list_idls().await {
        Ok(idls) => Ok(Json(idls)),
        Err(e) => Err(internal_error(e)),
    }
}

/// `GET /idls/:program_id` — Get IDL for a specific program.
pub async fn get_idl(
    State(state): State<AppState>,
    Path(program_id): Path<String>,
) -> impl IntoResponse {
    match state.pg_store.get_idl(&program_id).await {
        Ok(Some(idl)) => Ok(Json(idl)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "IDL not found".into(),
            }),
        )),
        Err(e) => Err(internal_error(e)),
    }
}

/// `DELETE /idls/:program_id` — Remove an IDL (requires auth).
pub async fn delete_idl(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(program_id): Path<String>,
) -> impl IntoResponse {
    match state.pg_store.delete_idl(&program_id).await {
        Ok(true) => {
            let mut registry = state.idl_registry.write().await;
            registry.unregister(&program_id);
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "IDL not found".into(),
            }),
        )),
        Err(e) => Err(internal_error(e)),
    }
}
