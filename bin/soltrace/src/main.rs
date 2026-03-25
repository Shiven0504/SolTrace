use std::sync::Arc;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::sync::{watch, RwLock};
use tracing_subscriber::EnvFilter;

use soltrace_api::{build_router, JwtConfig};
use soltrace_decoder::idl_decoder::{AnchorIdl, IdlRegistry};
use soltrace_ingestion::backfill::BackfillConfig;
use soltrace_ingestion::rpc_listener::{ListenerConfig, RpcListener};
use soltrace_storage::{PgStore, RedisCache};

/// Application configuration loaded from config/default.toml + env vars.
#[derive(Debug, Deserialize)]
struct AppConfig {
    solana: SolanaConfig,
    database: DatabaseConfig,
    redis: RedisConfig,
    ingestion: IngestionConfig,
    api: ApiConfig,
    #[serde(default)]
    backfill: BackfillAppConfig,
    #[serde(default)]
    auth: AuthConfig,
}

#[derive(Debug, Deserialize)]
struct AuthConfig {
    #[serde(default = "default_jwt_secret")]
    jwt_secret: String,
    #[serde(default = "default_jwt_expiry_hours")]
    jwt_expiry_hours: u64,
    /// Google OAuth client ID (optional — enables Google sign-in when set).
    google_client_id: Option<String>,
}

fn default_jwt_secret() -> String {
    "soltrace-dev-secret-change-in-production".to_string()
}

fn default_jwt_expiry_hours() -> u64 {
    72
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            jwt_expiry_hours: default_jwt_expiry_hours(),
            google_client_id: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SolanaConfig {
    rpc_url: String,
    ws_url: String,
}

#[derive(Debug, Deserialize)]
struct DatabaseConfig {
    url: String,
    max_connections: u32,
}

#[derive(Debug, Deserialize)]
struct RedisConfig {
    url: String,
}

#[derive(Debug, Deserialize)]
struct IngestionConfig {
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
    slot_cursor_key: String,
}

#[derive(Debug, Deserialize)]
struct ApiConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct BackfillAppConfig {
    #[serde(default = "default_rate_limit_ms")]
    rate_limit_ms: u64,
    #[serde(default = "default_batch_size")]
    batch_size: usize,
}

fn default_rate_limit_ms() -> u64 {
    200
}
fn default_batch_size() -> usize {
    100
}

impl Default for BackfillAppConfig {
    fn default() -> Self {
        Self {
            rate_limit_ms: default_rate_limit_ms(),
            batch_size: default_batch_size(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("SolTrace starting...");

    // Load configuration
    let config: AppConfig = config::Config::builder()
        .add_source(config::File::with_name("config/default"))
        .add_source(config::Environment::with_prefix("SOLTRACE").separator("__"))
        .build()
        .context("failed to load configuration")?
        .try_deserialize()
        .context("failed to deserialize configuration")?;

    tracing::info!(rpc = %config.solana.rpc_url, "loaded configuration");

    // Connect to PostgreSQL
    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect(&config.database.url)
        .await
        .context("failed to connect to PostgreSQL")?;

    tracing::info!("connected to PostgreSQL");

    let pg_store = PgStore::new(pg_pool);

    // Run Phase 2 migration (idempotent with IF NOT EXISTS)
    let migration_sql = include_str!("../../../migrations/002_phase2_backfill_webhooks_idls.sql");
    pg_store
        .run_migrations(migration_sql)
        .await
        .context("failed to run Phase 2 migrations")?;
    tracing::info!("Phase 2 migrations applied");

    // Run Phase 3 auth migration
    let auth_migration_sql = include_str!("../../../migrations/003_users_auth.sql");
    pg_store
        .run_migrations(auth_migration_sql)
        .await
        .context("failed to run auth migrations")?;
    tracing::info!("auth migrations applied");

    // Run Google OAuth migration
    let google_oauth_sql = include_str!("../../../migrations/004_google_oauth.sql");
    pg_store
        .run_migrations(google_oauth_sql)
        .await
        .context("failed to run Google OAuth migrations")?;
    tracing::info!("Google OAuth migrations applied");

    // Connect to Redis
    let redis_client = redis::Client::open(config.redis.url.as_str())
        .context("failed to create Redis client")?;

    // Verify Redis connection
    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .context("failed to connect to Redis")?;
    redis::cmd("PING")
        .query_async::<String>(&mut conn)
        .await
        .context("Redis PING failed")?;
    drop(conn);

    tracing::info!("connected to Redis");

    let redis_cache = RedisCache::new(redis_client, config.ingestion.slot_cursor_key);

    // Load IDL registry from DB
    let idl_registry = Arc::new(RwLock::new(IdlRegistry::new()));
    {
        let idls = pg_store
            .list_idls()
            .await
            .context("failed to load IDLs from database")?;

        let mut registry = idl_registry.write().await;
        for idl_row in &idls {
            if let Ok(parsed) =
                serde_json::from_value::<AnchorIdl>(idl_row.idl_json.clone())
            {
                registry.register(idl_row.program_id.clone(), parsed);
                tracing::info!(program = %idl_row.program_id, "loaded IDL");
            }
        }
        tracing::info!(count = idls.len(), "IDL registry initialized");
    }

    // Channel to notify ingestion when wallets change
    let (wallet_changed_tx, wallet_changed_rx) = watch::channel(());
    let wallet_changed_tx = Arc::new(wallet_changed_tx);

    // Build backfill config
    let backfill_config = BackfillConfig {
        rpc_url: config.solana.rpc_url.clone(),
        rate_limit_ms: config.backfill.rate_limit_ms,
        batch_size: config.backfill.batch_size,
    };

    // JWT config for auth (with optional Google OAuth)
    let jwt_config = JwtConfig::new(config.auth.jwt_secret, config.auth.jwt_expiry_hours)
        .with_google(config.auth.google_client_id.clone());

    if jwt_config.google_client_id.is_some() {
        tracing::info!("Google OAuth sign-in enabled");
    }

    // Start API server
    let api_addr = format!("{}:{}", config.api.host, config.api.port);
    let router = build_router(
        pg_store.clone(),
        redis_cache.clone(),
        wallet_changed_tx,
        backfill_config,
        idl_registry,
        Some(jwt_config),
    );

    let listener = tokio::net::TcpListener::bind(&api_addr)
        .await
        .context("failed to bind API server")?;

    tracing::info!(addr = %api_addr, "API server listening");

    // Start ingestion listener in a separate task
    let listener_config = ListenerConfig {
        rpc_url: config.solana.rpc_url,
        ws_url: config.solana.ws_url,
        initial_backoff_ms: config.ingestion.initial_backoff_ms,
        max_backoff_ms: config.ingestion.max_backoff_ms,
    };

    let rpc_listener = RpcListener::new(listener_config, pg_store, redis_cache, wallet_changed_rx);

    let ingestion_handle = tokio::spawn(async move {
        if let Err(e) = rpc_listener.run().await {
            tracing::error!(error = %e, "ingestion listener exited with error");
        }
    });

    // Run API server (blocks until shutdown)
    let api_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            tracing::error!(error = %e, "API server exited with error");
        }
    });

    // Wait for either task to complete (shouldn't happen normally)
    tokio::select! {
        _ = ingestion_handle => tracing::error!("ingestion task exited unexpectedly"),
        _ = api_handle => tracing::error!("API server exited unexpectedly"),
    }

    tracing::info!("SolTrace shutting down");
    Ok(())
}
