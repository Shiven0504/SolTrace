-- Phase 2: Backfill jobs, webhooks, and IDL storage

-- Backfill job tracking
CREATE TABLE IF NOT EXISTS backfill_jobs (
    id              BIGSERIAL PRIMARY KEY,
    wallet          TEXT NOT NULL REFERENCES watched_wallets(wallet_pubkey),
    status          TEXT NOT NULL DEFAULT 'pending',   -- pending, running, completed, failed
    before_sig      TEXT,                              -- start scanning before this signature
    until_sig       TEXT,                              -- stop scanning at this signature
    total_fetched   BIGINT NOT NULL DEFAULT 0,
    total_indexed   BIGINT NOT NULL DEFAULT 0,
    error_message   TEXT,
    created_at      TIMESTAMPTZ DEFAULT now(),
    updated_at      TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_backfill_jobs_wallet ON backfill_jobs(wallet);
CREATE INDEX IF NOT EXISTS idx_backfill_jobs_status ON backfill_jobs(status);

-- Webhook registrations
CREATE TABLE IF NOT EXISTS webhooks (
    id              BIGSERIAL PRIMARY KEY,
    url             TEXT NOT NULL,
    secret          TEXT,                              -- HMAC signing secret
    wallet          TEXT REFERENCES watched_wallets(wallet_pubkey),  -- NULL = all wallets
    direction       TEXT,                              -- NULL = both, 'deposit' or 'withdrawal'
    min_amount      BIGINT,                            -- NULL = no minimum
    mint            TEXT,                              -- NULL = all mints
    active          BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_webhooks_active ON webhooks(active);

-- Webhook delivery log (for retry / debugging)
CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id              BIGSERIAL PRIMARY KEY,
    webhook_id      BIGINT NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,
    transfer_id     BIGINT NOT NULL REFERENCES token_transfers(id),
    status_code     INT,
    response_body   TEXT,
    attempt         INT NOT NULL DEFAULT 1,
    delivered_at    TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_webhook ON webhook_deliveries(webhook_id);

-- Uploaded Anchor IDLs for dynamic decoding
CREATE TABLE IF NOT EXISTS program_idls (
    program_id      TEXT PRIMARY KEY,
    idl_json        JSONB NOT NULL,
    name            TEXT,
    version         TEXT,
    uploaded_at     TIMESTAMPTZ DEFAULT now()
);
