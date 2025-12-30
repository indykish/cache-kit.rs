//! Builder pattern for complex cache operations.

use crate::backend::CacheBackend;
use crate::error::Result;
use crate::observability::TtlPolicy;
use crate::strategy::CacheStrategy;
use crate::{CacheEntity, CacheExpander, CacheFeed, DataRepository};
use std::str::FromStr;
use std::time::Duration;

/// Fluent builder for complex cache operations and configuration.
///
/// Provides chainable methods to configure strategies, TTL overrides, and retry logic.
///
/// # Example
///
/// ```ignore
/// use cache_kit::strategy::CacheStrategy;
/// use std::time::Duration;
///
/// // expander is a CacheExpander<B> instance
/// // feeder is a CacheFeed implementation
/// // repo is a DataRepository implementation
/// let result = expander
///     .builder()
///     .with_strategy(CacheStrategy::Refresh)
///     .with_ttl(Duration::from_secs(300))
///     .with_retry(3)
///     .execute(&mut feeder, &repo).await?;
/// ```
pub struct CacheOperationBuilder<'a, B: CacheBackend> {
    expander: &'a mut CacheExpander<B>,
    strategy: CacheStrategy,
    ttl_override: Option<Duration>,
    retry_count: u32,
}

impl<'a, B: CacheBackend> CacheOperationBuilder<'a, B> {
    /// Create a new builder with default settings.
    pub(crate) fn new(expander: &'a mut CacheExpander<B>) -> Self {
        Self {
            expander,
            strategy: CacheStrategy::Refresh,
            ttl_override: None,
            retry_count: 0,
        }
    }

    /// Set the cache strategy.
    ///
    /// # Example
    ///
    /// ```ignore
    /// builder.with_strategy(CacheStrategy::Invalidate)
    /// ```
    pub fn with_strategy(mut self, strategy: CacheStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Override TTL for this operation.
    ///
    /// Temporarily overrides the default TTL policy for this specific operation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// builder.with_ttl(Duration::from_secs(300))  // 5 minutes
    /// ```
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl_override = Some(ttl);
        self
    }

    /// Set retry count for failed operations.
    ///
    /// If the operation fails, it will be retried up to `count` times.
    ///
    /// # Example
    ///
    /// ```ignore
    /// builder.with_retry(3)  // Retry up to 3 times on failure
    /// ```
    pub fn with_retry(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    /// Execute the cache operation.
    ///
    /// Applies the configured strategy, TTL override, and retry logic,
    /// then executes the cache operation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// expander
    ///     .builder()
    ///     .with_strategy(CacheStrategy::Refresh)
    ///     .with_ttl(Duration::from_secs(300))
    ///     .execute(&mut feeder, &repo).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err` if cache operation fails after all retry attempts. Errors include:
    /// - `Error::ValidationError`: Feeder or entity validation fails
    /// - `Error::DeserializationError`: Cached data is corrupted
    /// - `Error::InvalidCacheEntry`: Invalid cache envelope
    /// - `Error::VersionMismatch`: Schema version mismatch
    /// - `Error::BackendError`: Cache backend unavailable
    /// - `Error::RepositoryError`: Database access fails
    /// - `Error::SerializationError`: Entity serialization fails
    ///
    /// Failed operations are retried up to `retry_count` times with exponential backoff
    /// before returning the error. If all retries fail, the final error is returned.
    pub async fn execute<T, F, R>(mut self, feeder: &mut F, repository: &R) -> Result<()>
    where
        T: CacheEntity,
        F: CacheFeed<T>,
        R: DataRepository<T>,
        T::Key: FromStr,
    {
        // Apply TTL override if specified
        if let Some(ttl) = self.ttl_override {
            let original_policy =
                std::mem::replace(&mut self.expander.ttl_policy, TtlPolicy::Fixed(ttl));

            // Execute with override
            let result = self.execute_with_retry(feeder, repository).await;

            // Restore original policy
            self.expander.ttl_policy = original_policy;

            result
        } else {
            // Execute without TTL override
            self.execute_with_retry(feeder, repository).await
        }
    }

    /// Execute operation with retry logic.
    async fn execute_with_retry<T, F, R>(&mut self, feeder: &mut F, repository: &R) -> Result<()>
    where
        T: CacheEntity,
        F: CacheFeed<T>,
        R: DataRepository<T>,
        T::Key: FromStr,
    {
        let mut attempts = 0;
        let max_attempts = self.retry_count + 1; // +1 for initial attempt

        loop {
            attempts += 1;

            match self
                .expander
                .with::<T, F, R>(feeder, repository, self.strategy.clone())
                .await
            {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if attempts >= max_attempts {
                        return Err(e);
                    }

                    debug!(
                        "Cache operation failed (attempt {}/{}), retrying...",
                        attempts, max_attempts
                    );

                    // Small delay before retry (exponential backoff)
                    if self.retry_count > 0 {
                        let delay =
                            tokio::time::Duration::from_millis(100 * 2_u64.pow(attempts - 1));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::InMemoryBackend;
    use crate::feed::GenericFeeder;
    use crate::repository::InMemoryRepository;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Serialize, Deserialize)]
    struct TestEntity {
        id: String,
        value: String,
    }

    impl CacheEntity for TestEntity {
        type Key = String;

        fn cache_key(&self) -> Self::Key {
            self.id.clone()
        }

        fn cache_prefix() -> &'static str {
            "test"
        }
    }

    #[tokio::test]
    async fn test_builder_basic() {
        let backend = InMemoryBackend::new();
        let mut expander = CacheExpander::new(backend.clone());

        // Populate repository
        let mut repo = InMemoryRepository::new();
        repo.insert(
            "1".to_string(),
            TestEntity {
                id: "1".to_string(),
                value: "data".to_string(),
            },
        );

        let mut feeder = GenericFeeder::new("1".to_string());

        // Execute using builder
        expander
            .builder()
            .with_strategy(CacheStrategy::Refresh)
            .execute::<TestEntity, _, _>(&mut feeder, &repo)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_some());
        assert_eq!(feeder.data.expect("Data not found").value, "data");
    }

    #[tokio::test]
    async fn test_builder_with_ttl() {
        let backend = InMemoryBackend::new();
        let mut expander = CacheExpander::new(backend.clone())
            .with_ttl_policy(TtlPolicy::Fixed(Duration::from_secs(60)));

        let mut repo = InMemoryRepository::new();
        repo.insert(
            "1".to_string(),
            TestEntity {
                id: "1".to_string(),
                value: "data".to_string(),
            },
        );

        let mut feeder = GenericFeeder::new("1".to_string());

        // Override TTL to 300 seconds
        expander
            .builder()
            .with_strategy(CacheStrategy::Refresh)
            .with_ttl(Duration::from_secs(300))
            .execute::<TestEntity, _, _>(&mut feeder, &repo)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_some());

        // Verify original TTL policy is restored
        match &expander.ttl_policy {
            TtlPolicy::Fixed(duration) => assert_eq!(*duration, Duration::from_secs(60)),
            _ => panic!("Expected Fixed TTL policy"),
        }
    }

    #[tokio::test]
    async fn test_builder_chaining() {
        let backend = InMemoryBackend::new();
        let mut expander = CacheExpander::new(backend.clone());

        let mut repo = InMemoryRepository::new();
        repo.insert(
            "1".to_string(),
            TestEntity {
                id: "1".to_string(),
                value: "data".to_string(),
            },
        );

        let mut feeder = GenericFeeder::new("1".to_string());

        // Test method chaining
        expander
            .builder()
            .with_strategy(CacheStrategy::Refresh)
            .with_ttl(Duration::from_secs(300))
            .with_retry(2)
            .execute::<TestEntity, _, _>(&mut feeder, &repo)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_some());
    }

    #[tokio::test]
    async fn test_builder_with_invalidate_strategy() {
        let backend = InMemoryBackend::new();
        let mut expander = CacheExpander::new(backend.clone());

        // Pre-populate cache with stale data
        let stale_entity = TestEntity {
            id: "1".to_string(),
            value: "stale_data".to_string(),
        };
        let bytes = stale_entity
            .serialize_for_cache()
            .expect("Failed to serialize");
        backend
            .clone()
            .set("test:1", bytes, None)
            .await
            .expect("Failed to set");

        // Populate repository with fresh data
        let mut repo = InMemoryRepository::new();
        repo.insert(
            "1".to_string(),
            TestEntity {
                id: "1".to_string(),
                value: "fresh_data".to_string(),
            },
        );

        let mut feeder = GenericFeeder::new("1".to_string());

        // Use builder to invalidate cache
        expander
            .builder()
            .with_strategy(CacheStrategy::Invalidate)
            .execute::<TestEntity, _, _>(&mut feeder, &repo)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_some());
        assert_eq!(feeder.data.expect("Data not found").value, "fresh_data");
    }

    #[tokio::test]
    async fn test_builder_with_bypass_strategy() {
        let backend = InMemoryBackend::new();
        let mut expander = CacheExpander::new(backend.clone());

        // Pre-populate cache
        let cached_entity = TestEntity {
            id: "1".to_string(),
            value: "cached_data".to_string(),
        };
        let bytes = cached_entity
            .serialize_for_cache()
            .expect("Failed to serialize");
        backend
            .clone()
            .set("test:1", bytes, None)
            .await
            .expect("Failed to set");

        // Populate repository with different data
        let mut repo = InMemoryRepository::new();
        repo.insert(
            "1".to_string(),
            TestEntity {
                id: "1".to_string(),
                value: "db_data".to_string(),
            },
        );

        let mut feeder = GenericFeeder::new("1".to_string());

        // Use builder to bypass cache
        expander
            .builder()
            .with_strategy(CacheStrategy::Bypass)
            .execute::<TestEntity, _, _>(&mut feeder, &repo)
            .await
            .expect("Failed to execute");

        // Should get database data, not cached data
        assert!(feeder.data.is_some());
        assert_eq!(feeder.data.expect("Data not found").value, "db_data");
    }

    #[tokio::test]
    async fn test_builder_multiple_operations() {
        let backend = InMemoryBackend::new();
        let mut expander = CacheExpander::new(backend.clone());

        let mut repo = InMemoryRepository::new();
        repo.insert(
            "1".to_string(),
            TestEntity {
                id: "1".to_string(),
                value: "data1".to_string(),
            },
        );
        repo.insert(
            "2".to_string(),
            TestEntity {
                id: "2".to_string(),
                value: "data2".to_string(),
            },
        );

        // First operation with different TTL
        let mut feeder1 = GenericFeeder::new("1".to_string());
        expander
            .builder()
            .with_strategy(CacheStrategy::Refresh)
            .with_ttl(Duration::from_secs(100))
            .execute::<TestEntity, _, _>(&mut feeder1, &repo)
            .await
            .expect("Failed to execute");

        // Second operation with different TTL
        let mut feeder2 = GenericFeeder::new("2".to_string());
        expander
            .builder()
            .with_strategy(CacheStrategy::Refresh)
            .with_ttl(Duration::from_secs(200))
            .execute::<TestEntity, _, _>(&mut feeder2, &repo)
            .await
            .expect("Failed to execute");

        assert!(feeder1.data.is_some());
        assert!(feeder2.data.is_some());
        assert_eq!(feeder1.data.expect("Data1 not found").value, "data1");
        assert_eq!(feeder2.data.expect("Data2 not found").value, "data2");
    }
}
