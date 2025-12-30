//! Memcached cache backend implementation.

use super::CacheBackend;
use crate::error::{Error, Result};
use async_memcached::AsciiProtocol;
use deadpool_memcached::{Manager, Pool};
use std::time::Duration;

/// Default Memcached connection pool size.
/// Formula: (CPU cores × 2) + 1
/// For 8-core systems: 16 connections is optimal
/// Override with MEMCACHED_POOL_SIZE environment variable
const DEFAULT_POOL_SIZE: u32 = 16;

/// Configuration for Memcached backend.
#[derive(Clone, Debug)]
pub struct MemcachedConfig {
    pub servers: Vec<String>, // e.g., ["localhost:11211", "cache2:11211"]
    pub connection_timeout: Duration,
    pub pool_size: u32,
}

impl Default for MemcachedConfig {
    fn default() -> Self {
        MemcachedConfig {
            servers: vec!["localhost:11211".to_string()],
            connection_timeout: Duration::from_secs(5),
            pool_size: 10,
        }
    }
}

/// Memcached backend with connection pooling and async operations.
///
/// Provides distributed caching using Memcached protocol via async connection pool.
///
/// # Example
///
/// ```no_run
/// # use cache_kit::backend::{MemcachedBackend, MemcachedConfig, CacheBackend};
/// # use cache_kit::error::Result;
/// # async fn example() -> Result<()> {
/// let config = MemcachedConfig {
///     servers: vec!["localhost:11211".to_string()],
///     ..Default::default()
/// };
///
/// let backend = MemcachedBackend::new(config).await?;
/// backend.set("key", b"value".to_vec(), None).await?;
/// let value = backend.get("key").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct MemcachedBackend {
    pool: Pool,
}

impl MemcachedBackend {
    /// Create new Memcached backend from configuration.
    ///
    /// # Errors
    /// Returns `Err` if connection pool creation fails
    pub async fn new(config: MemcachedConfig) -> Result<Self> {
        // deadpool-memcached Manager takes a single server address
        // Use the first server from the list
        let addr = config
            .servers
            .first()
            .ok_or_else(|| Error::ConfigError("No memcached servers specified".to_string()))?
            .clone();

        let manager = Manager::new(addr.clone());

        let pool = Pool::builder(manager)
            .max_size(config.pool_size as usize)
            .build()
            .map_err(|e| Error::ConfigError(format!("Failed to create connection pool: {}", e)))?;

        info!(
            "✓ Memcached backend initialized with server: {} (pool size: {})",
            addr, config.pool_size
        );

        Ok(MemcachedBackend { pool })
    }

    /// Create from server address directly.
    ///
    /// Pool size is determined by:
    /// 1. `MEMCACHED_POOL_SIZE` environment variable (if set)
    /// 2. `DEFAULT_POOL_SIZE` constant (10)
    ///
    /// # Errors
    /// Returns `Err` if connection pool creation fails
    pub async fn from_server(addr: String) -> Result<Self> {
        let pool_size = std::env::var("MEMCACHED_POOL_SIZE")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(DEFAULT_POOL_SIZE);

        let config = MemcachedConfig {
            servers: vec![addr],
            pool_size,
            ..Default::default()
        };
        Self::new(config).await
    }
}

impl CacheBackend for MemcachedBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut conn = self.pool.get().await.map_err(|e| {
            Error::BackendError(format!("Failed to get Memcached connection: {}", e))
        })?;

        match conn.get(key).await {
            Ok(Some(value)) => {
                debug!("✓ Memcached GET {} -> HIT", key);
                Ok(value.data)
            }
            Ok(None) => {
                debug!("✓ Memcached GET {} -> MISS", key);
                Ok(None)
            }
            Err(e) => Err(Error::BackendError(format!(
                "Memcached GET failed for key {}: {}",
                key, e
            ))),
        }
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| {
            Error::BackendError(format!("Failed to get Memcached connection: {}", e))
        })?;

        // Convert Duration to i64 seconds for Memcached TTL
        // Values < 2592000 (30 days) are interpreted as seconds from now
        let expiration = ttl.map(|d| d.as_secs() as i64);

        // Correct parameter order: set(key, value, ttl, flags)
        conn.set(key, value.as_slice(), expiration, None)
            .await
            .map_err(|e| {
                Error::BackendError(format!("Memcached SET failed for key {}: {}", key, e))
            })?;

        if let Some(d) = ttl {
            debug!("✓ Memcached SET {} (TTL: {:?})", key, d);
        } else {
            debug!("✓ Memcached SET {}", key);
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| {
            Error::BackendError(format!("Failed to get Memcached connection: {}", e))
        })?;

        conn.delete(key).await.map_err(|e| {
            Error::BackendError(format!("Memcached DELETE failed for key {}: {}", key, e))
        })?;

        debug!("✓ Memcached DELETE {}", key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        // Memcached doesn't have native EXISTS, use get to check
        let mut conn = self.pool.get().await.map_err(|e| {
            Error::BackendError(format!("Failed to get Memcached connection: {}", e))
        })?;

        match conn.get(key).await {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(Error::BackendError(format!(
                "Memcached EXISTS check failed for key {}: {}",
                key, e
            ))),
        }
    }

    async fn mget(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.pool.get().await.map_err(|e| {
            Error::BackendError(format!("Failed to get Memcached connection: {}", e))
        })?;

        // Use native get_multi for batch retrieval - single round trip
        // Note: get_multi may return "not found" error if no keys exist
        let values = match conn.get_multi(keys).await {
            Ok(vals) => vals,
            Err(e) => {
                let err_msg = e.to_string();
                // Handle "not found" error gracefully - it just means no keys exist
                if err_msg.contains("not found") {
                    debug!("✓ Memcached MGET {} keys (all miss)", keys.len());
                    return Ok(vec![None; keys.len()]);
                }
                return Err(Error::BackendError(format!("Memcached MGET failed: {}", e)));
            }
        };

        // Build a HashMap for O(1) lookup: key -> data
        // Only store values where data is present
        let mut value_map = std::collections::HashMap::with_capacity(values.len());
        for value in values {
            let key_str = String::from_utf8_lossy(&value.key).to_string();
            if let Some(data) = value.data {
                value_map.insert(key_str, data);
            }
        }

        // Preserve input order and handle missing keys
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            match value_map.get(*key) {
                Some(data) => {
                    debug!("MGET key {} -> HIT", key);
                    results.push(Some(data.clone()));
                }
                None => {
                    debug!("MGET key {} -> MISS", key);
                    results.push(None);
                }
            }
        }

        debug!("✓ Memcached MGET {} keys (batch operation)", keys.len());
        Ok(results)
    }

    async fn mdelete(&self, keys: &[&str]) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| {
            Error::BackendError(format!("Failed to get Memcached connection: {}", e))
        })?;

        for key in keys {
            // Ignore errors for individual deletions
            let _ = conn.delete(key).await;
        }

        debug!("✓ Memcached MDELETE {} keys", keys.len());
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        // Try to get a connection and perform a simple operation
        match self.pool.get().await {
            Ok(mut conn) => {
                // Try a simple get operation to verify the connection works
                match conn.get("__health_check__").await {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
            Err(_) => Ok(false),
        }
    }

    async fn clear_all(&self) -> Result<()> {
        let mut conn = self.pool.get().await.map_err(|e| {
            Error::BackendError(format!("Failed to get Memcached connection: {}", e))
        })?;

        conn.flush_all()
            .await
            .map_err(|e| Error::BackendError(format!("Memcached FLUSH_ALL failed: {}", e)))?;

        warn!("⚠ Memcached FLUSH_ALL executed - all cache cleared!");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memcached_config_default() {
        let config = MemcachedConfig::default();
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0], "localhost:11211");
        assert_eq!(config.pool_size, 10);
    }

    #[test]
    fn test_memcached_config_multiple_servers() {
        let config = MemcachedConfig {
            servers: vec![
                "localhost:11211".to_string(),
                "cache1:11211".to_string(),
                "cache2:11211".to_string(),
            ],
            connection_timeout: Duration::from_secs(5),
            pool_size: 20,
        };

        assert_eq!(config.servers.len(), 3);
        assert_eq!(config.pool_size, 20);
    }
}
