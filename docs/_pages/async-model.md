---
layout: single
title: Async Programming Model
description: "Understanding cache-kit's async-first design and tokio integration"
permalink: /async-model/
---




---

## Async-First Philosophy

cache-kit is built from the ground up as an **async-first** library. This design choice reflects the reality of modern Rust services where:

- Database queries are async (SQLx, SeaORM, tokio-postgres)
- HTTP handlers are async (Axum, Actix, warp)
- gRPC services are async (tonic)
- Background workers are async (tokio, async-std)

The cache layer sits between these components and must integrate seamlessly with async workflows.

---

## Tokio Runtime Integration

cache-kit is designed for `tokio`-based applications. The library does not:

- Spawn its own runtime
- Require a specific runtime configuration
- Impose threading models on your application

Instead, cache-kit operates within **your** existing tokio runtime.

### Runtime Requirements

```toml
[dependencies]
tokio = { version = "1.41", features = ["rt", "sync", "macros"] }
cache-kit = "0.9"
```

The minimum required tokio features:
- `rt` — Runtime support
- `sync` — Synchronization primitives (Arc, Mutex, RwLock)
- `macros` — `#[tokio::main]` attribute macro

---

## Interaction Model

The typical interaction flow follows this pattern:

```
Async Database → Async Cache → Async Application
```

### Example: Async Database to Async Cache

```rust
use cache_kit::{CacheEntity, CacheFeed, DataRepository, CacheExpander};
use cache_kit::backend::InMemoryBackend;
use cache_kit::strategy::CacheStrategy;
use sqlx::PgPool;

// Async repository using SQLx
#[derive(Clone)]
struct UserRepository {
    pool: PgPool,
}

impl DataRepository<User> for UserRepository {
    async fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        Ok(user)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect("postgres://localhost/mydb").await?;
    let repo = UserRepository { pool };

    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);

    let mut feeder = UserFeeder {
        id: "user_001".to_string(),
        user: None,
    };

    // Cache operation works seamlessly within async context
    expander.with(&mut feeder, &repo, CacheStrategy::Refresh).await?;

    Ok(())
}
```

---

## Why DataRepository is Async

The `DataRepository` trait uses **async** methods:

```rust
pub trait DataRepository<T: CacheEntity>: Send + Sync {
    async fn fetch_by_id(&self, id: &T::Key) -> Result<Option<T>>;
}
```

This design is intentional and provides several benefits:

### 1. Native Async Support

Async trait methods align with modern Rust practices and integrate seamlessly with async databases.

### 2. Flexibility

You can use both sync and async database layers (see [Handling Sync Code](#handling-sync-code-in-async-repositories) section below).

### 3. Backend Compatibility

Cache backends (Redis, Memcached) are inherently async, and the async trait ensures compatibility across all patterns.

---

## Async Database Integration

The `DataRepository` trait is async, designed for seamless integration with modern async database drivers:

```rust
use sqlx::PgPool;
use cache_kit::DataRepository;

impl DataRepository<Product> for ProductRepository {
    async fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<Product>> {
        // Direct async/await - no bridging needed
        let product = sqlx::query_as!(
            Product,
            "SELECT id, name, price FROM products WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        Ok(product)
    }
}
```

**Recommended async databases:**
- **SQLx** — Async, compile-time checked SQL
- **SeaORM** — Async ORM for Rust
- **tokio-postgres** — Pure async PostgreSQL client

### ⚠️ NEVER use block_in_place + block_on

**NEVER use `block_in_place` + `Handle::current().block_on()`** — this pattern is incorrect. Always use `async fn` with `.await` for async databases.

---

## Async Cache Backends

Cache backends are fully async and follow the same initialization pattern:

```rust
// Redis
use cache_kit::backend::{RedisBackend, RedisConfig};
let config = RedisConfig { url: "redis://localhost:6379".to_string(), ..Default::default() };
let backend = RedisBackend::new(config)?;

// Memcached
use cache_kit::backend::{MemcachedBackend, MemcachedConfig};
let config = MemcachedConfig { servers: vec!["localhost:11211".to_string()], ..Default::default() };
let backend = MemcachedBackend::new(config)?;

// InMemory (lock-free via DashMap)
use cache_kit::backend::InMemoryBackend;
let backend = InMemoryBackend::new();

// All use the same expander API
let mut expander = CacheExpander::new(backend);
```

All backends work seamlessly within your async context — no special handling required.

---

## Runtime Choice is Yours

cache-kit does not:

- Require a specific tokio runtime configuration
- Spawn background tasks (no `tokio::spawn` calls)
- Create thread pools
- Impose executor choices

You control:

- Runtime flavor (multi-thread, current-thread)
- Worker thread count
- Task spawning strategy
- Shutdown behavior



---

## Sync Support (Not Recommended)

While cache-kit is async-first, you can use it in synchronous contexts if absolutely necessary by creating a runtime:

```rust
use tokio::runtime::Runtime;

fn sync_cache_operation() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new()?;

    runtime.block_on(async {
        let backend = InMemoryBackend::new();
        let mut expander = CacheExpander::new(backend);
        
        expander.with(&mut feeder, &repo, CacheStrategy::Refresh).await?;
        Ok(())
    })
}

// Or from within an existing async context:
// let handle = tokio::runtime::Handle::current();
// handle.block_on(async { ... })
```

**Important:** These patterns are provided for compatibility only. Async-first design is strongly recommended for production services.

---

## Best Practices

### DO

✅ Use `tokio::main` for your application entry point
✅ Make `DataRepository::fetch_by_id` an async function
✅ Use async database drivers (SQLx, SeaORM, tokio-postgres)
✅ Let cache-kit operate within your existing runtime
✅ Keep async boundaries explicit and clear

### DON'T

❌ Use `block_in_place` + `block_on` (incorrect pattern)
❌ Call `block_on` inside async contexts
❌ Create multiple tokio runtimes unnecessarily
❌ Assume cache-kit manages runtime lifecycle

---

## Example: Full Async Service

Here's a complete example of a tokio-based service using cache-kit:

```rust
use cache_kit::{CacheEntity, CacheFeed, DataRepository, CacheService};
use cache_kit::backend::RedisBackend;
use cache_kit::strategy::CacheStrategy;
use axum::{Router, routing::get, extract::State};
use sqlx::PgPool;
use std::sync::Arc;

// Your entities, feeders, and repository implementations

struct AppState {
    cache: CacheService<RedisBackend>,
    repo: Arc<UserRepository>,
}

async fn get_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(user_id): axum::extract::Path<String>,
) -> Result<String, String> {
    let mut feeder = UserFeeder {
        id: user_id,
        user: None,
    };

    state.cache
        .execute(&mut feeder, &*state.repo, CacheStrategy::Refresh)
        .await
        .map_err(|e| e.to_string())?;

    Ok(format!("User: {:?}", feeder.user))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Database setup
    let pool = PgPool::connect("postgres://localhost/mydb").await?;
    let repo = Arc::new(UserRepository { pool });

    // Cache setup
    let config = cache_kit::backend::RedisConfig::default();
    let cache = CacheService::new(RedisBackend::new(config)?);

    // Application state
    let state = Arc::new(AppState { cache, repo });

    // HTTP server (async all the way)
    let app = Router::new()
        .route("/users/:id", get(get_user))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

---

## Next Steps

- Learn about [Core Concepts](concepts) in cache-kit
- Explore [Database & ORM Compatibility](database-compatibility)
- Review the [Actix + SQLx reference implementation](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx)
