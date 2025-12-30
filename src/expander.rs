//! Cache expander - main entry point for cache operations.

use crate::backend::CacheBackend;
use crate::entity::CacheEntity;
use crate::error::{Error, Result};
use crate::feed::CacheFeed;
use crate::key::CacheKeyBuilder;
use crate::observability::{CacheMetrics, NoOpMetrics, TtlPolicy};
use crate::repository::DataRepository;
use crate::strategy::CacheStrategy;
use std::str::FromStr;
use std::time::Instant;

/// Core cache expander - handles cache lookup and fallback logic.
///
/// This is the main entry point for cache operations.
/// Supports multiple access patterns through different methods.
///
/// # Example
///
/// ```ignore
/// use cache_kit::{CacheExpander, backend::InMemoryBackend};
///
/// let backend = InMemoryBackend::new();
/// let mut expander = CacheExpander::new(backend);
/// ```
pub struct CacheExpander<B: CacheBackend> {
    backend: B,
    metrics: Box<dyn CacheMetrics>,
    pub(crate) ttl_policy: TtlPolicy,
}

impl<B: CacheBackend> CacheExpander<B> {
    /// Create new expander with given backend.
    pub fn new(backend: B) -> Self {
        CacheExpander {
            backend,
            metrics: Box::new(NoOpMetrics),
            ttl_policy: TtlPolicy::default(),
        }
    }

    /// Set custom metrics handler.
    pub fn with_metrics(mut self, metrics: Box<dyn CacheMetrics>) -> Self {
        self.metrics = metrics;
        self
    }

    /// Set custom TTL policy.
    pub fn with_ttl_policy(mut self, policy: TtlPolicy) -> Self {
        self.ttl_policy = policy;
        self
    }

    /// Create a builder for complex cache operations.
    ///
    /// The builder pattern provides a fluent interface for configuring
    /// cache operations with custom strategies, TTL overrides, and retry logic.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = expander
    ///     .builder()
    ///     .with_strategy(CacheStrategy::Refresh)
    ///     .with_ttl(Duration::from_secs(300))
    ///     .with_retry(3)
    ///     .execute(&mut feeder, &repo)?;
    /// ```
    pub fn builder(&mut self) -> crate::builder::CacheOperationBuilder<'_, B> {
        crate::builder::CacheOperationBuilder::new(self)
    }

    /// Generic cache operation with strategy.
    ///
    /// This is the primary method used in 80% of cases.
    ///
    /// # Arguments
    /// - `feeder`: Entity feeder (implements `CacheFeed<T>`)
    /// - `repository`: Data repository (implements `DataRepository<T>`)
    /// - `strategy`: Cache strategy (Fresh, Refresh, Invalidate, Bypass)
    ///
    /// # Example
    /// ```ignore
    /// let expander = CacheExpander::new(redis_backend);
    /// let mut feeder = EmploymentFeeder { id: "emp_123", employment: None };
    /// let repo = EmploymentRepository { db: pool };
    ///
    /// expander.with(
    ///     &mut feeder,
    ///     &repo,
    ///     CacheStrategy::Refresh
    /// ).await?;
    ///
    /// let employment = feeder.employment;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err` in these cases:
    /// - `Error::ValidationError`: Feeder or entity validation fails
    /// - `Error::DeserializationError`: Cached data is corrupted or has wrong format
    /// - `Error::InvalidCacheEntry`: Cache magic header mismatch or invalid envelope
    /// - `Error::VersionMismatch`: Schema version mismatch between code and cached data
    /// - `Error::BackendError`: Cache backend is unavailable or network error
    /// - `Error::RepositoryError`: Database access fails
    /// - `Error::Timeout`: Operation exceeds timeout threshold
    /// - `Error::SerializationError`: Entity serialization for caching fails
    pub async fn with<T, F, R>(
        &self,
        feeder: &mut F,
        repository: &R,
        strategy: CacheStrategy,
    ) -> Result<()>
    where
        T: CacheEntity,
        F: CacheFeed<T>,
        R: DataRepository<T>,
        T::Key: FromStr,
    {
        let timer = Instant::now();

        // Step 1: Validate feeder
        feeder.validate()?;

        // Step 2: Get entity ID and build cache key
        let entity_id = feeder.entity_id();
        let cache_key = CacheKeyBuilder::build::<T>(&entity_id);

        debug!(
            "» Cache operation for key: {} (strategy: {})",
            cache_key, strategy
        );

        // Step 3: Execute strategy
        let result = match strategy {
            CacheStrategy::Fresh => self.strategy_fresh::<T, R>(&cache_key, repository).await,
            CacheStrategy::Refresh => self.strategy_refresh::<T, R>(&cache_key, repository).await,
            CacheStrategy::Invalidate => {
                self.strategy_invalidate::<T, R>(&cache_key, repository)
                    .await
            }
            CacheStrategy::Bypass => self.strategy_bypass::<T, R>(&cache_key, repository).await,
        };

        // Step 4: Handle result
        match result {
            Ok(Some(entity)) => {
                entity.validate()?;
                feeder.on_hit(&cache_key)?;
                feeder.on_loaded(&entity)?;
                feeder.feed(Some(entity));
                self.metrics.record_hit(&cache_key, timer.elapsed());
                info!("✓ Cache operation succeeded in {:?}", timer.elapsed());
            }
            Ok(None) => {
                feeder.on_miss(&cache_key)?;
                feeder.feed(None);
                self.metrics.record_miss(&cache_key, timer.elapsed());
                debug!("Entity not found after cache operation for {}", cache_key);
            }
            Err(e) => {
                self.metrics.record_error(&cache_key, &e.to_string());
                return Err(e);
            }
        }

        Ok(())
    }

    /// Fresh strategy: Cache only, no database fallback.
    async fn strategy_fresh<T: CacheEntity, R: DataRepository<T>>(
        &self,
        cache_key: &str,
        _repository: &R,
    ) -> Result<Option<T>> {
        debug!("Executing Fresh strategy for {}", cache_key);

        match self.backend.get(cache_key).await? {
            Some(bytes) => {
                debug!("✓ Cache hit (Fresh strategy)");
                T::deserialize_from_cache(&bytes).map(Some)
            }
            None => {
                debug!("✗ Cache miss (Fresh strategy) - no fallback");
                Ok(None)
            }
        }
    }

    /// Refresh strategy: Try cache, fallback to database on miss.
    async fn strategy_refresh<T: CacheEntity, R: DataRepository<T>>(
        &self,
        cache_key: &str,
        repository: &R,
    ) -> Result<Option<T>>
    where
        T::Key: FromStr,
    {
        debug!("Executing Refresh strategy for {}", cache_key);

        // Try cache first
        if let Some(bytes) = self.backend.get(cache_key).await? {
            debug!("✓ Cache hit (Refresh strategy)");
            return T::deserialize_from_cache(&bytes).map(Some);
        }

        debug!("Cache miss, falling back to database");

        // Cache miss - fetch from database
        let id = self.extract_id_from_key::<T>(cache_key)?;
        match repository.fetch_by_id(&id).await? {
            Some(entity) => {
                // Store in cache for future use
                let ttl = self.ttl_policy.get_ttl(T::cache_prefix());
                let bytes = entity.serialize_for_cache()?;
                let _ = self.backend.set(cache_key, bytes, ttl).await;
                Ok(Some(entity))
            }
            None => Ok(None),
        }
    }

    /// Invalidate strategy: Clear cache and refresh from database.
    async fn strategy_invalidate<T: CacheEntity, R: DataRepository<T>>(
        &self,
        cache_key: &str,
        repository: &R,
    ) -> Result<Option<T>>
    where
        T::Key: FromStr,
    {
        debug!("Executing Invalidate strategy for {}", cache_key);

        // Delete from cache
        self.backend.delete(cache_key).await?;
        debug!("✓ Cache invalidated for {}", cache_key);

        // Fetch fresh from database
        let id = self.extract_id_from_key::<T>(cache_key)?;
        match repository.fetch_by_id(&id).await? {
            Some(entity) => {
                // Re-populate cache
                let ttl = self.ttl_policy.get_ttl(T::cache_prefix());
                let bytes = entity.serialize_for_cache()?;
                let _ = self.backend.set(cache_key, bytes, ttl).await;
                Ok(Some(entity))
            }
            None => Ok(None),
        }
    }

    /// Bypass strategy: Skip cache, always hit database.
    async fn strategy_bypass<T: CacheEntity, R: DataRepository<T>>(
        &self,
        cache_key: &str,
        repository: &R,
    ) -> Result<Option<T>>
    where
        T::Key: FromStr,
    {
        debug!("Executing Bypass strategy for {}", cache_key);
        debug!("Bypassing cache entirely for {}", cache_key);

        // Fetch from database without checking cache
        let id = self.extract_id_from_key::<T>(cache_key)?;
        match repository.fetch_by_id(&id).await? {
            Some(entity) => {
                // Still populate cache for others
                let ttl = self.ttl_policy.get_ttl(T::cache_prefix());
                let bytes = entity.serialize_for_cache()?;
                let _ = self.backend.set(cache_key, bytes, ttl).await;
                Ok(Some(entity))
            }
            None => Ok(None),
        }
    }

    /// Extract the ID portion from a cache key.
    /// Format: "prefix:id" → "id"
    fn extract_id_from_key<T: CacheEntity>(&self, cache_key: &str) -> Result<T::Key>
    where
        T::Key: FromStr,
    {
        let parts: Vec<&str> = cache_key.split(':').collect();
        if parts.len() > 1 {
            let id_str = parts[1..].join(":");
            id_str.parse().ok().ok_or_else(|| {
                Error::ValidationError(format!("Failed to parse ID from cache key: {}", cache_key))
            })
        } else {
            Err(Error::ValidationError(format!(
                "Invalid cache key format: {}",
                cache_key
            )))
        }
    }

    /// Get backend reference (for advanced use).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Get mutable backend reference (for advanced use).
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
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
    async fn test_expander_with_fresh_strategy_hit() {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend.clone());

        // Pre-populate cache
        let entity = TestEntity {
            id: "1".to_string(),
            value: "data".to_string(),
        };
        let bytes = entity.serialize_for_cache().expect("Failed to serialize");
        backend
            .clone()
            .set("test:1", bytes, None)
            .await
            .expect("Failed to set");

        // Create feeder
        let mut feeder = GenericFeeder::new("1".to_string());
        let repo = InMemoryRepository::new();

        // Execute
        expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Fresh)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_some());
    }

    #[tokio::test]
    async fn test_expander_with_fresh_strategy_miss() {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend);

        let mut feeder = GenericFeeder::new("1".to_string());
        let repo = InMemoryRepository::new();

        expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Fresh)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_none());
    }

    #[tokio::test]
    async fn test_expander_refresh_strategy_cache_hit() {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend.clone());

        // Pre-populate cache
        let entity = TestEntity {
            id: "1".to_string(),
            value: "cached_data".to_string(),
        };
        let bytes = entity.serialize_for_cache().expect("Failed to serialize");
        backend
            .clone()
            .set("test:1", bytes, None)
            .await
            .expect("Failed to set");

        let mut feeder = GenericFeeder::new("1".to_string());
        let repo = InMemoryRepository::new();

        expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Refresh)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_some());
        assert_eq!(feeder.data.expect("Data not found").value, "cached_data");
    }

    #[tokio::test]
    async fn test_expander_refresh_strategy_cache_miss_db_hit() {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend.clone());

        // Populate repository
        let mut repo = InMemoryRepository::new();
        repo.insert(
            "1".to_string(),
            TestEntity {
                id: "1".to_string(),
                value: "db_data".to_string(),
            },
        );

        let mut feeder = GenericFeeder::new("1".to_string());

        expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Refresh)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_some());
        assert_eq!(feeder.data.expect("Data not found").value, "db_data");

        // Verify it was cached
        let cached = backend
            .clone()
            .get("test:1")
            .await
            .expect("Failed to get from cache");
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn test_expander_refresh_strategy_complete_miss() {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend);

        let mut feeder = GenericFeeder::new("nonexistent".to_string());
        let repo = InMemoryRepository::new();

        expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Refresh)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_none());
    }

    #[tokio::test]
    async fn test_expander_invalidate_strategy() {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend.clone());

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

        expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Invalidate)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_some());
        assert_eq!(feeder.data.expect("Data not found").value, "fresh_data");

        // Verify cache was updated
        let cached_bytes = backend
            .clone()
            .get("test:1")
            .await
            .expect("Failed to get")
            .expect("Cache is empty");
        let cached_entity =
            TestEntity::deserialize_from_cache(&cached_bytes).expect("Failed to deserialize");
        assert_eq!(cached_entity.value, "fresh_data");
    }

    #[tokio::test]
    async fn test_expander_bypass_strategy() {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend.clone());

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

        expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Bypass)
            .await
            .expect("Failed to execute");

        // Should get database data, not cached data
        assert!(feeder.data.is_some());
        assert_eq!(feeder.data.expect("Data not found").value, "db_data");
    }

    #[tokio::test]
    async fn test_expander_with_ttl_policy() {
        use crate::observability::TtlPolicy;
        use std::time::Duration;

        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend.clone())
            .with_ttl_policy(TtlPolicy::Fixed(Duration::from_secs(300)));

        let mut repo = InMemoryRepository::new();
        repo.insert(
            "1".to_string(),
            TestEntity {
                id: "1".to_string(),
                value: "data".to_string(),
            },
        );

        let mut feeder = GenericFeeder::new("1".to_string());

        expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Refresh)
            .await
            .expect("Failed to execute");

        assert!(feeder.data.is_some());
    }

    #[tokio::test]
    async fn test_expander_with_custom_metrics() {
        use crate::observability::CacheMetrics;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        #[derive(Clone)]
        struct TestMetrics {
            hits: Arc<Mutex<usize>>,
            misses: Arc<Mutex<usize>>,
        }

        impl CacheMetrics for TestMetrics {
            fn record_hit(&self, _key: &str, _duration: Duration) {
                *self.hits.lock().expect("Failed to lock hits") += 1;
            }

            fn record_miss(&self, _key: &str, _duration: Duration) {
                *self.misses.lock().expect("Failed to lock misses") += 1;
            }
        }

        let metrics = TestMetrics {
            hits: Arc::new(Mutex::new(0)),
            misses: Arc::new(Mutex::new(0)),
        };

        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend.clone()).with_metrics(Box::new(metrics.clone()));

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

        // First call: cache miss, database hit
        expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Refresh)
            .await
            .expect("Failed to execute");

        assert_eq!(*metrics.hits.lock().expect("Failed to lock hits"), 1); // Counted as hit after DB fetch

        // Second call: cache hit
        let mut feeder2 = GenericFeeder::new("1".to_string());
        expander
            .with::<TestEntity, _, _>(&mut feeder2, &repo, CacheStrategy::Refresh)
            .await
            .expect("Failed to execute");

        assert_eq!(*metrics.hits.lock().expect("Failed to lock hits"), 2);
    }

    #[tokio::test]
    async fn test_expander_error_on_missing_data() {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend);

        let mut feeder = GenericFeeder::new("nonexistent".to_string());
        let repo = InMemoryRepository::new();

        // Fresh strategy with miss should return None (not error)
        let result = expander
            .with::<TestEntity, _, _>(&mut feeder, &repo, CacheStrategy::Fresh)
            .await;
        assert!(result.is_ok());
        assert!(feeder.data.is_none());
    }

    #[tokio::test]
    async fn test_expander_backend_reference() {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend.clone());

        // Test backend() method
        let _backend_ref = expander.backend();

        // Verify we can access the backend
        assert_eq!(backend.len().await, 0);
    }
}
