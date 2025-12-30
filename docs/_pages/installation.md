---
layout: single
title: Installation & Configuration
description: "Getting started with cache-kit in your Rust project"
permalink: /installation/
---

---

## Prerequisites

- **Rust:** 1.75 or later
- **Tokio:** 1.41 or later (async runtime)

---

## Installation

Add cache-kit to your `Cargo.toml`:

```toml
[dependencies]
cache-kit = { version = "0.9" }
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.41", features = ["rt", "sync", "macros"] }
```

### Feature Flags

cache-kit uses feature flags to enable optional backends:

| Feature     | Description                           | Default     |
| ----------- | ------------------------------------- | ----------- |
| `inmemory`  | In-memory cache backend               | ✅ Enabled  |
| `redis`     | Redis backend with connection pooling | ❌ Optional |
| `memcached` | Memcached backend                     | ❌ Optional |
| `all`       | Enable all backends                   | ❌ Optional |

### Basic Installation (InMemory Only)

```toml
[dependencies]
cache-kit = { version = "0.9" }
```

This provides the InMemory backend, perfect for:

- Development
- Testing
- Single-instance services

### Redis Backend

```toml
[dependencies]
cache-kit = { version = "0.9", features = ["redis"] }
```

Enables production-grade Redis caching with connection pooling.

### Memcached Backend

```toml
[dependencies]
cache-kit = { version = "0.9", features = ["memcached"] }
```

Enables Memcached backend for high-performance distributed caching.

### All Backends

```toml
[dependencies]
cache-kit = { version = "0.9", features = ["all"] }
```

Enables all available backends. Useful for:

- Testing multiple backends
- Switching backends based on environment
- Benchmarking comparisons

---

## Minimal Configuration

cache-kit requires minimal configuration. Here's a complete working example:

```rust
use cache_kit::{
    CacheEntity, CacheFeed, DataRepository, CacheService,
    backend::InMemoryBackend,
    strategy::CacheStrategy,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct User {
    id: String,
    name: String,
}

impl CacheEntity for User {
    type Key = String;
    fn cache_key(&self) -> Self::Key { self.id.clone() }
    fn cache_prefix() -> &'static str { "user" }
}

struct UserFeeder {
    id: String,
    user: Option<User>,
}

impl CacheFeed<User> for UserFeeder {
    fn entity_id(&mut self) -> String { self.id.clone() }
    fn feed(&mut self, entity: Option<User>) { self.user = entity; }
}

struct UserRepository;

impl DataRepository<User> for UserRepository {
    async fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<User>> {
        Ok(Some(User {
            id: id.clone(),
            name: "Alice".to_string(),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = InMemoryBackend::new();
    let cache = CacheService::new(backend);
    let repository = UserRepository;

    let mut feeder = UserFeeder {
        id: "user_001".to_string(),
        user: None,
    };

    cache.execute(&mut feeder, &repository, CacheStrategy::Refresh).await?;

    println!("User: {:?}", feeder.user);
    Ok(())
}
```

---

## Backend Configuration

### ⚠️ Production Backend Requirement

**InMemory backend is for development/testing only.** Use Redis or Memcached for production deployments.

---

### InMemory Backend

No configuration required:

```rust
use cache_kit::{CacheService, backend::InMemoryBackend};

let cache = CacheService::new(InMemoryBackend::new());
```

The InMemory backend uses `DashMap` internally, providing:

- Lock-free concurrent HashMap
- Thread-safe operations
- Zero external dependencies

### Redis Backend

```rust
use cache_kit::{CacheService, backend::{RedisBackend, RedisConfig}};

let config = RedisConfig {
    url: "redis://localhost:6379".to_string(),
    max_connections: 10,
    min_connections: 2,
    connection_timeout_secs: 5,
};

let cache = CacheService::new(RedisBackend::new(config)?);
```

#### Redis Configuration Options

| Field                     | Type     | Default  | Description                   |
| ------------------------- | -------- | -------- | ----------------------------- |
| `url`                     | `String` | Required | Redis connection URL          |
| `max_connections`         | `usize`  | `10`     | Maximum connection pool size  |
| `min_connections`         | `usize`  | `2`      | Minimum idle connections      |
| `connection_timeout_secs` | `u64`    | `5`      | Connection timeout in seconds |

#### Redis URL Formats

```rust
// Local Redis
"redis://localhost:6379"

// Remote Redis with password
"redis://:password@example.com:6379"

// Redis with specific database
"redis://localhost:6379/1"

// TLS connection
"rediss://example.com:6379"
```

#### Environment-Based Configuration

```rust
use std::env;

let redis_url = env::var("REDIS_URL")
    .unwrap_or_else(|_| "redis://localhost:6379".to_string());

let config = RedisConfig {
    url: redis_url,
    ..Default::default()
};

let backend = RedisBackend::new(config)?;
```

### Memcached Backend

```rust
use cache_kit::{CacheService, backend::{MemcachedBackend, MemcachedConfig}};

let config = MemcachedConfig {
    servers: vec!["localhost:11211".to_string()],
    max_connections: 10,
    min_connections: 2,
};

let cache = CacheService::new(MemcachedBackend::new(config)?);
```

#### Memcached Configuration Options

| Field             | Type          | Default  | Description                  |
| ----------------- | ------------- | -------- | ---------------------------- |
| `servers`         | `Vec<String>` | Required | Memcached server addresses   |
| `max_connections` | `usize`       | `10`     | Maximum connection pool size |
| `min_connections` | `usize`       | `2`      | Minimum idle connections     |

#### Multiple Memcached Servers

```rust
let config = MemcachedConfig {
    servers: vec![
        "memcached-01:11211".to_string(),
        "memcached-02:11211".to_string(),
        "memcached-03:11211".to_string(),
    ],
    max_connections: 20,
    min_connections: 5,
};

let backend = MemcachedBackend::new(config)?;
```

---

## TTL Configuration

Configure time-to-live (TTL) for cached entries:

### Global TTL

```rust
use std::time::Duration;
use cache_kit::{CacheService, observability::TtlPolicy, backend::InMemoryBackend};

let cache = CacheService::new(InMemoryBackend::new());
// Note: TTL configuration via CacheService is set through backend configuration
```

### No TTL (Cache Forever)

```rust
use cache_kit::{CacheService, backend::InMemoryBackend};

// Don't set TTL - cached entries never expire
let cache = CacheService::new(InMemoryBackend::new());
```

**Note:** "Cache forever" is not recommended for production. Always set appropriate TTLs based on your data freshness requirements.

---

## Environment-Based Configuration

Create a configuration module for your application:

```rust
use cache_kit::{CacheService, backend::{InMemoryBackend, RedisBackend, RedisConfig}};
use std::env;

pub enum Environment {
    Development,
    Production,
}

impl Environment {
    pub fn from_env() -> Self {
        match env::var("ENV").as_deref() {
            Ok("production") => Environment::Production,
            _ => Environment::Development,
        }
    }
}

pub fn create_cache_service() -> CacheService<impl cache_kit::backend::CacheBackend> {
    match Environment::from_env() {
        Environment::Development => {
            CacheService::new(InMemoryBackend::new())
        }
        Environment::Production => {
            let redis_url = env::var("REDIS_URL")
                .expect("REDIS_URL must be set in production");

            let config = RedisConfig {
                url: redis_url,
                max_connections: 20,
                min_connections: 5,
                connection_timeout_secs: 10,
            };

            CacheService::new(RedisBackend::new(config).expect("Failed to connect to Redis"))
        }
    }
}
```

Usage:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = create_cache_service();

    // Your application logic
    Ok(())
}
```

---

## Docker Compose for Development

Use Docker Compose to run Redis and Memcached locally:

```yaml
version: "3.8"

services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes

  memcached:
    image: memcached:1.6-alpine
    ports:
      - "11211:11211"
    command: memcached -m 64

volumes:
  redis_data:
```

Start services:

```bash
docker-compose up -d
```

Test connections:

```bash
# Redis
redis-cli ping  # Should return: PONG

# Memcached
echo "stats" | nc localhost 11211  # Should return stats
```

---

## Testing Configuration

For unit and integration tests, use the InMemory backend:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cache_kit::backend::InMemoryBackend;

    #[tokio::test]
    async fn test_user_caching() {
        // Use InMemory backend for tests (no external dependencies)
        let backend = InMemoryBackend::new();
        let mut expander = CacheExpander::new(backend);

        // Your test logic
    }
}
```

### Test Isolation

Each test should create its own backend instance to avoid interference:

```rust
#[tokio::test]
async fn test_cache_hit() {
    let backend = InMemoryBackend::new();  // Fresh instance
    let mut expander = CacheExpander::new(backend);
    // Test logic
}

#[tokio::test]
async fn test_cache_miss() {
    let backend = InMemoryBackend::new();  // Separate instance
    let mut expander = CacheExpander::new(backend);
    // Test logic
}
```

---

## Production Checklist

Before deploying cache-kit to production:

- [ ] **Backend selected:** Redis or Memcached for production
- [ ] **Connection pooling configured:** Set appropriate `max_connections`
- [ ] **TTL policies defined:** Set TTLs based on data freshness requirements
- [ ] **Error handling implemented:** Handle cache failures gracefully
- [ ] **Monitoring enabled:** Track cache hit/miss rates
- [ ] **Environment variables set:** `REDIS_URL` or `MEMCACHED_SERVERS`
- [ ] **Fallback strategy:** Application works if cache is unavailable
- [ ] **Load tested:** Verify performance under expected load

---

## Common Configuration Patterns

### Pattern 1: Shared Cache Across Services

```rust
use cache_kit::{CacheService, backend::{RedisBackend, RedisConfig}};

let cache = CacheService::new(RedisBackend::new(config)?);

// CacheService is Clone - easily share across services
let user_service = UserService::new(cache.clone());
let product_service = ProductService::new(cache.clone());
```

### Pattern 2: Multiple Cache Backends

```rust
use cache_kit::{CacheService, backend::{RedisBackend, RedisConfig}};

// User cache
let user_backend = RedisBackend::new(user_config)?;
let user_cache = CacheService::new(user_backend);

// Product cache
let product_backend = RedisBackend::new(product_config)?;
let product_cache = CacheService::new(product_backend);
```

### Pattern 3: Read-Through Cache

```rust
use cache_kit::{CacheService, DataRepository, CacheFeed, strategy::CacheStrategy, backend::RedisBackend};

pub struct CachedRepository<R> {
    repository: R,
    cache: CacheService<RedisBackend>,
}

impl<R: DataRepository<User>> CachedRepository<R> {
    pub async fn get_user(&self, id: &str) -> cache_kit::Result<Option<User>> {
        let mut feeder = UserFeeder {
            id: id.to_string(),
            user: None,
        };

        // Always use Refresh strategy for read-through
        self.cache.execute(&mut feeder, &self.repository, CacheStrategy::Refresh).await?;

        Ok(feeder.user)
    }
}
```

---

## Next Steps

- Learn about [Database & ORM compatibility](database-compatibility)
- Explore [Cache backend options](backends) in detail
- Review [Serialization formats](serialization)
- See the [Actix + SQLx example](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx)
