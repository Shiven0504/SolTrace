-- Wallets being tracked
CREATE TABLE IF NOT EXISTS watched_wallets (
    wallet_pubkey   TEXT PRIMARY KEY,
    label           TEXT,
    created_at      TIMESTAMPTZ DEFAULT now()
);

-- Token accounts owned by watched wallets
CREATE TABLE IF NOT EXISTS token_accounts (
    token_account   TEXT PRIMARY KEY,
    owner_wallet    TEXT NOT NULL REFERENCES watched_wallets(wallet_pubkey),
    mint            TEXT NOT NULL,
    balance         BIGINT NOT NULL DEFAULT 0,
    last_slot       BIGINT NOT NULL DEFAULT 0,
    UNIQUE(owner_wallet, mint)
);

-- Every transfer event (deposits AND withdrawals)
CREATE TABLE IF NOT EXISTS token_transfers (
    id              BIGSERIAL PRIMARY KEY,
    signature       TEXT NOT NULL,
    slot            BIGINT NOT NULL,
    block_time      TIMESTAMPTZ,
    instruction_idx INT NOT NULL,
    program_id      TEXT NOT NULL,
    source_account  TEXT NOT NULL,
    dest_account    TEXT NOT NULL,
    mint            TEXT,                -- NULL for native SOL
    amount          BIGINT NOT NULL,
    direction       TEXT NOT NULL,       -- 'deposit' or 'withdrawal'
    wallet          TEXT NOT NULL REFERENCES watched_wallets(wallet_pubkey),
    UNIQUE(signature, instruction_idx)   -- dedup replays
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_transfers_wallet_time ON token_transfers(wallet, block_time DESC);
CREATE INDEX IF NOT EXISTS idx_transfers_mint ON token_transfers(mint, block_time DESC);
CREATE INDEX IF NOT EXISTS idx_token_accounts_owner ON token_accounts(owner_wallet);
