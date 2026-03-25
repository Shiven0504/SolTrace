//! Axum router definition wiring all API endpoints.

use std::sync::Arc;

use axum::routing::{delete, get, post, put};
use axum::Router;
use soltrace_decoder::idl_decoder::IdlRegistry;
use soltrace_ingestion::backfill::BackfillConfig;
use soltrace_storage::{PgStore, RedisCache};
use tokio::sync::{watch, RwLock};

use crate::auth::{self, JwtConfig};
use crate::handlers;

/// Shared application state available to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub pg_store: PgStore,
    pub redis_cache: RedisCache,
    /// Notify ingestion listener when wallets change.
    pub wallet_changed_tx: Arc<watch::Sender<()>>,
    /// Backfill configuration for spawning backfill jobs.
    pub backfill_config: BackfillConfig,
    /// Live IDL registry shared between API and decoder.
    pub idl_registry: Arc<RwLock<IdlRegistry>>,
    /// JWT config for authentication (None = auth disabled).
    pub jwt_config: Option<JwtConfig>,
}

/// Build the full Axum router with all API routes.
#[allow(clippy::too_many_arguments)]
pub fn build_router(
    pg_store: PgStore,
    redis_cache: RedisCache,
    wallet_changed_tx: Arc<watch::Sender<()>>,
    backfill_config: BackfillConfig,
    idl_registry: Arc<RwLock<IdlRegistry>>,
    jwt_config: Option<JwtConfig>,
) -> Router {
    let state = AppState {
        pg_store,
        redis_cache,
        wallet_changed_tx,
        backfill_config,
        idl_registry,
        jwt_config,
    };

    Router::new()
        // Phase 1 endpoints
        .route("/transfers", get(handlers::list_transfers))
        .route("/balances", get(handlers::get_balances))
        .route("/tx/{signature}", get(handlers::get_transaction))
        .route("/wallets", post(handlers::add_wallet))
        .route("/wallets", get(handlers::list_wallets))
        .route("/health", get(handlers::health_check))
        // Phase 2: Backfill
        .route("/backfill", post(handlers::start_backfill))
        .route("/backfill", get(handlers::list_backfill_jobs))
        .route("/backfill/{id}", get(handlers::get_backfill_job))
        // Phase 2: Webhooks
        .route("/webhooks", post(handlers::create_webhook))
        .route("/webhooks", get(handlers::list_webhooks))
        .route("/webhooks/{id}", get(handlers::get_webhook))
        .route("/webhooks/{id}", delete(handlers::delete_webhook))
        // Phase 2: IDLs
        .route("/idls", post(handlers::upload_idl))
        .route("/idls", get(handlers::list_idls))
        .route("/idls/{program_id}", get(handlers::get_idl))
        .route("/idls/{program_id}", delete(handlers::delete_idl))
        // Auth
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/google", post(auth::google_login))
        .route("/auth/google-client-id", get(auth::google_client_id))
        .route("/auth/username", put(auth::update_username))
        .route("/auth/me", get(auth::me))
        .with_state(state)
}
