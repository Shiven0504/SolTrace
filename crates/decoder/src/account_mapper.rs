//! Maintains wallet → token account mapping.
//!
//! Combines eager prefetch (`getTokenAccountsByOwner` on wallet registration)
//! with lazy resolution (`getAccountInfo` on cache miss).

use std::collections::HashMap;

/// In-memory account mapping, backed by Redis cache.
///
/// Maps token account addresses to their owner wallet pubkey.
/// Token account → owner mapping is stable (doesn't change), so caching is safe.
#[derive(Debug, Default)]
pub struct AccountMapper {
    /// token_account_address → owner_wallet_pubkey
    owners: HashMap<String, String>,
}

impl AccountMapper {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a known token account → owner mapping.
    pub fn insert(&mut self, token_account: &str, owner: &str) {
        self.owners
            .insert(token_account.to_string(), owner.to_string());
    }

    /// Look up the owner of a token account.
    pub fn get_owner(&self, token_account: &str) -> Option<&str> {
        self.owners.get(token_account).map(String::as_str)
    }

    /// Get all mappings (for classifier).
    pub fn all_mappings(&self) -> &HashMap<String, String> {
        &self.owners
    }

    /// Bulk insert mappings (e.g., from Redis cache on startup).
    pub fn load_mappings(&mut self, mappings: HashMap<String, String>) {
        self.owners.extend(mappings);
    }
}
