//! Redis operations: account owner cache, slot cursor tracking, streams.

use redis::AsyncCommands;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RedisCacheError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
}

/// Redis cache for slot tracking, account owner lookups, and stream operations.
#[derive(Clone)]
pub struct RedisCache {
    client: redis::Client,
    slot_key: String,
}

impl RedisCache {
    /// Create a new Redis cache handle.
    pub fn new(client: redis::Client, slot_key: String) -> Self {
        Self { client, slot_key }
    }

    async fn conn(&self) -> Result<redis::aio::MultiplexedConnection, RedisCacheError> {
        Ok(self.client.get_multiplexed_async_connection().await?)
    }

    // ── Slot cursor ────────────────────────────────────────────────

    /// Get the last processed slot.
    pub async fn get_last_slot(&self) -> Result<Option<u64>, RedisCacheError> {
        let mut conn = self.conn().await?;
        let val: Option<u64> = conn.get(&self.slot_key).await?;
        Ok(val)
    }

    /// Update the last processed slot (only if newer).
    pub async fn set_last_slot(&self, slot: u64) -> Result<(), RedisCacheError> {
        let mut conn = self.conn().await?;
        // Use a Lua script to atomically set only if new slot > current
        let script = redis::Script::new(
            r#"
            local current = redis.call('GET', KEYS[1])
            if current == false or tonumber(ARGV[1]) > tonumber(current) then
                redis.call('SET', KEYS[1], ARGV[1])
                return 1
            end
            return 0
            "#,
        );
        let _: i32 = script
            .key(&self.slot_key)
            .arg(slot)
            .invoke_async(&mut conn)
            .await?;
        Ok(())
    }

    // ── Account owner cache ────────────────────────────────────────

    /// Cache a token account → owner mapping.
    pub async fn cache_account_owner(
        &self,
        token_account: &str,
        owner: &str,
    ) -> Result<(), RedisCacheError> {
        let mut conn = self.conn().await?;
        let key = format!("soltrace:owner:{token_account}");
        conn.set::<_, _, ()>(&key, owner).await?;
        Ok(())
    }

    /// Look up a cached account owner.
    pub async fn get_account_owner(
        &self,
        token_account: &str,
    ) -> Result<Option<String>, RedisCacheError> {
        let mut conn = self.conn().await?;
        let key = format!("soltrace:owner:{token_account}");
        let val: Option<String> = conn.get(&key).await?;
        Ok(val)
    }

    /// Bulk cache account owners.
    pub async fn cache_account_owners(
        &self,
        mappings: &[(String, String)],
    ) -> Result<(), RedisCacheError> {
        if mappings.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn().await?;
        let mut pipe = redis::pipe();
        for (token_account, owner) in mappings {
            pipe.set(format!("soltrace:owner:{token_account}"), owner);
        }
        pipe.atomic().query_async::<()>(&mut conn).await?;
        Ok(())
    }

    // ── Stream operations (for ingestion buffering) ────────────────

    /// Push a raw transaction JSON into the ingestion stream.
    pub async fn push_to_stream(
        &self,
        stream_key: &str,
        data: &str,
    ) -> Result<String, RedisCacheError> {
        let mut conn = self.conn().await?;
        let id: String = redis::cmd("XADD")
            .arg(stream_key)
            .arg("*")
            .arg("data")
            .arg(data)
            .query_async(&mut conn)
            .await?;
        Ok(id)
    }

    /// Read entries from a stream.
    pub async fn read_stream(
        &self,
        stream_key: &str,
        last_id: &str,
        count: usize,
    ) -> Result<Vec<(String, String)>, RedisCacheError> {
        let mut conn = self.conn().await?;
        let results: redis::Value = redis::cmd("XREAD")
            .arg("COUNT")
            .arg(count)
            .arg("BLOCK")
            .arg(1000) // 1 second block
            .arg("STREAMS")
            .arg(stream_key)
            .arg(last_id)
            .query_async(&mut conn)
            .await?;

        // Parse XREAD response
        let entries = parse_xread_response(results);
        Ok(entries)
    }
}

/// Parse the Redis XREAD response into (id, data) pairs.
fn parse_xread_response(value: redis::Value) -> Vec<(String, String)> {
    let mut entries = Vec::new();

    // XREAD returns: [[stream_name, [[id, [field, value, ...]], ...]]]
    if let redis::Value::Array(streams) = value {
        for stream in streams {
            if let redis::Value::Array(parts) = stream {
                if parts.len() >= 2 {
                    if let redis::Value::Array(ref messages) = parts[1] {
                        for msg in messages {
                            if let redis::Value::Array(ref msg_parts) = msg {
                                if msg_parts.len() >= 2 {
                                    let id = redis_value_to_string(&msg_parts[0]);
                                    if let redis::Value::Array(ref fields) = msg_parts[1] {
                                        // fields = [field_name, field_value, ...]
                                        if fields.len() >= 2 {
                                            let data = redis_value_to_string(&fields[1]);
                                            entries.push((id, data));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    entries
}

fn redis_value_to_string(val: &redis::Value) -> String {
    match val {
        redis::Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
        redis::Value::SimpleString(s) => s.clone(),
        _ => String::new(),
    }
}
