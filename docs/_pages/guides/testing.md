---
layout: single
title: Testing Guide
parent: Guides
---

# Testing Guide - Quick Start

## Makefile Commands (Recommended)

### Quick Start

```bash
# Start services (Redis + Memcached)
make up

# Run all tests
make test-all

# Stop services
make down
```

### Common Commands

```bash
make unittest          # Unit tests only
make integration-test  # Integration tests
make redis-test        # Redis integration tests (requires 'make up')
make check            # Format + lint + unit tests
make help             # Show all commands
```

### Infrastructure

```bash
make up      # Start Redis + Memcached
make down    # Stop all services
make ps      # Show running services
make logs    # Show service logs
make clean   # Stop and remove containers
```

---

# Testing Guide

Comprehensive testing strategies for the cache framework and your custom implementations.

---

## Table of Contents

1. [Running Tests](#running-tests)
2. [Unit Testing Strategies](#unit-testing-strategies)
3. [Testing Custom Backends](#testing-custom-backends)
4. [Testing Custom Feeders](#testing-custom-feeders)
5. [Testing Repositories](#testing-repositories)
6. [Integration Testing](#integration-testing)
7. [Performance Testing](#performance-testing)
8. [Mocking & Test Doubles](#mocking--test-doubles)
9. [Common Testing Patterns](#common-testing-patterns)

---

## Running Tests

### Run All Tests

```bash
cargo test --all-features
```

### Run Tests for Specific Feature

```bash
cargo test --features inmemory
cargo test --features redis
cargo test --features memcached
```

### Run Tests with Logging

```bash
RUST_LOG=debug cargo test
RUST_LOG=cache=trace cargo test
```

### Run Specific Test

```bash
cargo test test_cache_hit -- --nocapture
```

### Run Tests in Parallel or Sequentially

```bash
# Parallel (default)
cargo test --all-features

# Sequential (useful for debugging)
cargo test --all-features -- --test-threads=1
```

### View Test Output

```bash
cargo test -- --nocapture
```

---

## Unit Testing Strategies

### Testing CacheEntity

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_cache_key_generation() {
        let entity = Employment {
            id: "emp_123".to_string(),
            employer: "Acme".to_string(),
        };

        let key = entity.cache_key();
        assert_eq!(key.to_string(), "emp_123");
    }

    #[test]
    fn test_cache_prefix() {
        assert_eq!(Employment::cache_prefix(), "employment");
    }

    #[test]
    fn test_serialization() {
        let entity = Employment {
            id: "emp_123".to_string(),
            employer: "Acme".to_string(),
        };

        let bytes = entity.serialize_for_cache().unwrap();
        let deserialized = Employment::deserialize_from_cache(&bytes).unwrap();
        
        assert_eq!(entity.id, deserialized.id);
        assert_eq!(entity.employer, deserialized.employer);
    }

    #[test]
    fn test_validation() {
        let valid = Employment {
            id: "emp_123".to_string(),
            employer: "Acme".to_string(),
        };

        assert!(valid.validate().is_ok());

        let invalid = Employment {
            id: "".to_string(), // Empty ID
            employer: "Acme".to_string(),
        };

        assert!(invalid.validate().is_err());
    }
}
```

### Testing Cache Strategies

```rust
#[test]
fn test_cache_strategy_fresh() {
    // Fresh strategy should only use cache, not DB
    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);

    // Pre-populate cache
    // ... then verify Fresh returns cached data without touching DB
}

#[test]
fn test_cache_strategy_refresh() {
    // Refresh should try cache, fallback to DB
    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);

    // First call: cache miss -> DB fetch
    // Second call: cache hit -> use cached value
}

#[test]
fn test_cache_strategy_invalidate() {
    // Invalidate should clear cache and fetch fresh
    // Verify old data is not returned
}

#[test]
fn test_cache_strategy_bypass() {
    // Bypass should skip cache entirely
    // Verify it goes directly to DB
}
```

---

## Testing Custom Backends

### Backend Interface Testing

```rust
#[cfg(test)]
mod backend_tests {
    use super::*;
    use std::time::Duration;

    // Generic test function for any backend
    fn test_backend_get_set<B: CacheBackend>(mut backend: B) {
        let key = "test_key";
        let value = b"test_value".to_vec();

        // Set
        assert!(backend.set(key, value.clone(), None).is_ok());

        // Get
        let retrieved = backend.get(key).unwrap();
        assert_eq!(retrieved, Some(value));
    }

    fn test_backend_delete<B: CacheBackend>(mut backend: B) {
        let key = "test_key";
        let value = b"test_value".to_vec();

        backend.set(key, value, None).unwrap();
        backend.delete(key).unwrap();

        let result = backend.get(key).unwrap();
        assert_eq!(result, None);
    }

    fn test_backend_exists<B: CacheBackend>(mut backend: B) {
        let key = "test_key";

        // Should not exist initially
        assert!(!backend.exists(key).unwrap());

        // After set
        backend.set(key, b"value".to_vec(), None).unwrap();
        assert!(backend.exists(key).unwrap());

        // After delete
        backend.delete(key).unwrap();
        assert!(!backend.exists(key).unwrap());
    }

    fn test_backend_ttl_expiration<B: CacheBackend>(mut backend: B) {
        let key = "temp_key";
        let short_ttl = Duration::from_millis(100);

        backend.set(key, b"value".to_vec(), Some(short_ttl)).unwrap();
        assert!(backend.exists(key).unwrap());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(150));
        assert!(!backend.exists(key).unwrap());
    }

    fn test_backend_clear_all<B: CacheBackend>(mut backend: B) {
        backend.set("key1", b"value1".to_vec(), None).unwrap();
        backend.set("key2", b"value2".to_vec(), None).unwrap();

        backend.clear_all().unwrap();

        assert!(backend.get("key1").unwrap().is_none());
        assert!(backend.get("key2").unwrap().is_none());
    }

    fn test_backend_health_check<B: CacheBackend>(mut backend: B) {
        let health = backend.health_check().unwrap();
        assert!(health);
    }

    // Batch operation tests
    fn test_backend_mget<B: CacheBackend>(mut backend: B) {
        backend.set("key1", b"value1".to_vec(), None).unwrap();
        backend.set("key2", b"value2".to_vec(), None).unwrap();

        let results = backend.mget(&["key1", "key2"]).unwrap();
        
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], Some(b"value1".to_vec()));
        assert_eq!(results[1], Some(b"value2".to_vec()));
    }

    fn test_backend_mdelete<B: CacheBackend>(mut backend: B) {
        backend.set("key1", b"value1".to_vec(), None).unwrap();
        backend.set("key2", b"value2".to_vec(), None).unwrap();

        backend.mdelete(&["key1", "key2"]).unwrap();

        assert!(backend.get("key1").unwrap().is_none());
        assert!(backend.get("key2").unwrap().is_none());
    }

    // For InMemoryBackend
    #[test]
    fn test_inmemory_backend() {
        let backend = InMemoryBackend::new();
        test_backend_get_set(backend.clone());
        test_backend_delete(backend.clone());
        test_backend_exists(backend.clone());
        test_backend_ttl_expiration(backend.clone());
        test_backend_clear_all(backend.clone());
        test_backend_health_check(backend.clone());
        test_backend_mget(backend.clone());
        test_backend_mdelete(backend);
    }
}
```

### Backend-Specific Tests

```rust
#[test]
#[cfg(feature = "redis")]
fn test_redis_backend_connection() {
    let config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        ..Default::default()
    };

    let backend = RedisBackend::new(config);
    assert!(backend.is_ok());
}

#[test]
#[cfg(feature = "memcached")]
fn test_memcached_backend_multi_server() {
    let config = MemcachedConfig {
        servers: vec![
            "localhost:11211".to_string(),
            "localhost:11212".to_string(),
        ],
        ..Default::default()
    };

    let backend = MemcachedBackend::new(config);
    assert!(backend.is_ok());
}
```

---

## Testing Custom Feeders

```rust
#[cfg(test)]
mod feeder_tests {
    use super::*;

    #[test]
    fn test_feeder_entity_id() {
        let mut feeder = EmploymentFeeder {
            id: "emp_123".to_string(),
            employment: None,
        };

        assert_eq!(feeder.entity_id(), "emp_123");
    }

    #[test]
    fn test_feeder_feed_some() {
        let mut feeder = EmploymentFeeder {
            id: "emp_123".to_string(),
            employment: None,
        };

        let entity = Employment {
            id: "emp_123".to_string(),
            employer: "Acme".to_string(),
        };

        feeder.feed(Some(entity.clone()));
        
        assert!(feeder.employment.is_some());
        assert_eq!(feeder.employment.unwrap().employer, "Acme");
    }

    #[test]
    fn test_feeder_feed_none() {
        let mut feeder = EmploymentFeeder {
            id: "emp_123".to_string(),
            employment: Some(Employment {
                id: "emp_456".to_string(),
                employer: "OldCorp".to_string(),
            }),
        };

        feeder.feed(None);
        assert!(feeder.employment.is_none());
    }

    #[test]
    fn test_generic_feeder() {
        let mut feeder = GenericFeeder::<Employment>::new("emp_123".to_string());
        
        let entity = Employment {
            id: "emp_123".to_string(),
            employer: "Acme".to_string(),
        };

        feeder.feed(Some(entity));
        assert!(feeder.entity.is_some());
    }
}
```

---

## Testing Repositories

### Mock Repository for Testing

```rust
pub struct MockRepository<T> {
    data: std::collections::HashMap<String, T>,
}

impl<T: Clone> MockRepository<T> {
    pub fn new() -> Self {
        MockRepository {
            data: std::collections::HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: String, entity: T) {
        self.data.insert(id, entity);
    }
}

impl<T: Clone + 'static> DataRepository<T> for MockRepository<T> {
    fn fetch_by_id(&self, id: &str) -> Result<Option<T>> {
        Ok(self.data.get(id).cloned())
    }
}

#[test]
fn test_with_mock_repository() {
    let mut repo = MockRepository::new();
    repo.insert("emp_123".to_string(), Employment {
        id: "emp_123".to_string(),
        employer: "Acme".to_string(),
    });

    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);

    let mut feeder = GenericFeeder::<Employment>::new("emp_123".to_string());
    expander.with(&mut feeder, &repo, CacheStrategy::Refresh).unwrap();

    assert!(feeder.entity.is_some());
}
```

### Repository with In-Memory Database

```rust
#[test]
fn test_repository_with_sqlx() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    runtime.block_on(async {
        let database_url = "sqlite::memory:";
        let pool = sqlx::SqlitePool::connect(database_url).await.unwrap();

        // Create table
        sqlx::query("CREATE TABLE employment (id TEXT, employer TEXT)")
            .execute(&pool)
            .await
            .unwrap();

        // Insert test data
        sqlx::query("INSERT INTO employment VALUES (?, ?)")
            .bind("emp_123")
            .bind("Acme")
            .execute(&pool)
            .await
            .unwrap();

        // Test repository
        let repo = EmploymentRepository { pool };
        let result = repo.fetch_by_id("emp_123").unwrap();
        
        assert!(result.is_some());
        assert_eq!(result.unwrap().employer, "Acme");
    });
}
```

---

## Integration Testing

### Full Cache Workflow

```rust
#[test]
fn test_full_cache_workflow() {
    // Setup
    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);
    let mut repo = MockRepository::new();

    // Seed repository
    repo.insert("emp_123".to_string(), Employment {
        id: "emp_123".to_string(),
        employer: "Acme".to_string(),
    });

    // First call: cache miss, DB hit
    let mut feeder = GenericFeeder::<Employment>::new("emp_123".to_string());
    expander.with(&mut feeder, &repo, CacheStrategy::Refresh).unwrap();
    assert!(feeder.entity.is_some());

    // Modify repository (should not affect cache)
    repo.data.remove("emp_123");

    // Second call: cache hit (data still available)
    let mut feeder2 = GenericFeeder::<Employment>::new("emp_123".to_string());
    expander.with(&mut feeder2, &repo, CacheStrategy::Fresh).unwrap();
    assert!(feeder2.entity.is_some());
}
```

### Multi-Entity Integration Test

```rust
#[test]
fn test_multiple_entity_types() {
    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);
    
    let mut employment_repo = MockRepository::new();
    let mut borrower_repo = MockRepository::new();

    // Cache Employment
    let mut emp_feeder = GenericFeeder::<Employment>::new("emp_1".to_string());
    expander.with(&mut emp_feeder, &employment_repo, CacheStrategy::Refresh).ok();

    // Cache Borrower
    let mut bor_feeder = GenericFeeder::<Borrower>::new("bor_1".to_string());
    expander.with(&mut bor_feeder, &borrower_repo, CacheStrategy::Refresh).ok();

    // Both should be cached independently
    assert!(emp_feeder.entity.is_some());
    assert!(bor_feeder.entity.is_some());
}
```

---

## Performance Testing

For comprehensive performance benchmarking using Criterion, including baseline comparison, regression detection, and connection pool optimization, see the **[Performance Guide](performance)**.

The Performance Guide covers:
- Running benchmarks with Criterion (`make perf`)
- Understanding statistical output and confidence intervals
- Baseline comparison and regression detection
- Expected performance metrics by backend
- Connection pool sizing and optimization
- Production monitoring and tuning

### Quick Ad-Hoc Performance Checks

For simple performance checks in unit tests:

```rust
#[test]
fn quick_perf_check_cache_hit() {
    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);
    let repo = MockRepository::new();

    let iterations = 10_000;
    let start = std::time::Instant::now();

    for i in 0..iterations {
        let mut feeder = GenericFeeder::<Employment>::new(format!("emp_{}", i));
        let _ = expander.with(&mut feeder, &repo, CacheStrategy::Fresh);
    }

    let elapsed = start.elapsed();
    let per_op = elapsed.as_nanos() / iterations;

    println!("Cache hit latency: {} ns/op", per_op);

    // Sanity check (not statistically rigorous)
    assert!(per_op < 5000, "Cache hit too slow: {} ns", per_op);
}
```

**Note:** For statistically rigorous benchmarking, use Criterion instead of manual timing. See [Performance Guide](performance).

---

## Mocking & Test Doubles

### Mock CacheMetrics

```rust
pub struct MockMetrics {
    pub hits: std::sync::Mutex<Vec<String>>,
    pub misses: std::sync::Mutex<Vec<String>>,
    pub errors: std::sync::Mutex<Vec<String>>,
}

impl CacheMetrics for MockMetrics {
    fn record_hit(&self, key: &str, _: Duration) {
        self.hits.lock().unwrap().push(key.to_string());
    }

    fn record_miss(&self, key: &str, _: Duration) {
        self.misses.lock().unwrap().push(key.to_string());
    }

    fn record_set(&self, key: &str, _: Duration) {
        // ...
    }

    fn record_error(&self, key: &str, _: &str) {
        self.errors.lock().unwrap().push(key.to_string());
    }
}

#[test]
fn test_metrics_recording() {
    let metrics = MockMetrics {
        hits: std::sync::Mutex::new(vec![]),
        misses: std::sync::Mutex::new(vec![]),
        errors: std::sync::Mutex::new(vec![]),
    };

    metrics.record_hit("emp_123", Duration::from_millis(1));
    assert_eq!(metrics.hits.lock().unwrap().len(), 1);
}
```

### Spy Backend

```rust
pub struct SpyBackend {
    inner: InMemoryBackend,
    pub get_calls: std::sync::Mutex<Vec<String>>,
    pub set_calls: std::sync::Mutex<Vec<String>>,
}

impl SpyBackend {
    pub fn new() -> Self {
        SpyBackend {
            inner: InMemoryBackend::new(),
            get_calls: std::sync::Mutex::new(vec![]),
            set_calls: std::sync::Mutex::new(vec![]),
        }
    }
}

impl CacheBackend for SpyBackend {
    fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>> {
        self.get_calls.lock().unwrap().push(key.to_string());
        self.inner.get(key)
    }

    fn set(&mut self, key: &str, value: Vec<u8>, ttl: Option<Duration>) -> Result<()> {
        self.set_calls.lock().unwrap().push(key.to_string());
        self.inner.set(key, value, ttl)
    }

    // ... other methods ...
}

#[test]
fn test_backend_interactions() {
    let mut backend = SpyBackend::new();
    let mut expander = CacheExpander::new(backend.clone());

    // ... perform cache operations ...

    // Verify interactions
    assert_eq!(backend.get_calls.lock().unwrap().len(), 2);
    assert_eq!(backend.set_calls.lock().unwrap().len(), 1);
}
```

---

## Common Testing Patterns

### Test Error Cases

```rust
#[test]
fn test_cache_error_handling() {
    // Test missing key
    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);
    let repo = MockRepository::new(); // Empty repo

    let mut feeder = GenericFeeder::<Employment>::new("nonexistent".to_string());
    let result = expander.with(&mut feeder, &repo, CacheStrategy::Fresh);

    // Should handle gracefully
    assert!(feeder.entity.is_none());
}

#[test]
fn test_serialization_error() {
    #[derive(Clone)]
    struct NonSerializable;

    // Implementation would fail to serialize
    // Test error handling path
}
```

### Parameterized Tests

```rust
#[test]
fn test_all_strategies() {
    for strategy in [
        CacheStrategy::Fresh,
        CacheStrategy::Refresh,
        CacheStrategy::Invalidate,
        CacheStrategy::Bypass,
    ] {
        // Test each strategy
        assert_strategy_works(&strategy);
    }
}

fn assert_strategy_works(strategy: &CacheStrategy) {
    // Strategy-specific assertions
}
```

### Test State Cleanup

```rust
struct TestFixture {
    backend: InMemoryBackend,
    expander: CacheExpander<InMemoryBackend>,
}

impl TestFixture {
    fn new() -> Self {
        let backend = InMemoryBackend::new();
        let expander = CacheExpander::new(backend.clone());
        TestFixture { backend, expander }
    }

    fn cleanup(&mut self) {
        // Clear state between tests
        self.backend.clear_all().ok();
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[test]
fn test_with_fixture() {
    let mut fixture = TestFixture::new();
    // Use fixture...
    // Cleanup happens automatically
}
```

---

## Best Practices

1. **Test behavior, not implementation** — Test what the cache does, not how it does it
2. **Use descriptive test names** — Name tests to describe what they verify
3. **Keep tests independent** — Each test should be runnable standalone
4. **Use fixtures for setup** — Reduce duplication with test helpers
5. **Test error paths** — Don't just test the happy path
6. **Mock external dependencies** — Use mock repositories, not real databases
7. **Benchmark-critical paths** — Measure cache hit/miss performance
8. **Test thread safety** — Verify `Send + Sync` implementation with concurrent tests

---

## See Also

- [CONTRIBUTING.md](CONTRIBUTING.md) — Testing your extensions
- [EXAMPLES.md](EXAMPLES.md) — Example implementations
- [README.md](README.md) — Quick start
