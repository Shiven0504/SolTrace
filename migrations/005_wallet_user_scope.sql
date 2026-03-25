-- Add user scoping to watched_wallets (Option B: public read, auth for write)
ALTER TABLE watched_wallets ADD COLUMN user_id BIGINT REFERENCES users(id);

-- Index for fast per-user wallet lookups
CREATE INDEX IF NOT EXISTS idx_watched_wallets_user ON watched_wallets(user_id);
