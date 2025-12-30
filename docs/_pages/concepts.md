---
layout: single
title: Core Concepts
description: "Understanding the fundamental concepts behind cache-kit"
permalink: /concepts/
---




---

## Overview

cache-kit is built around four core concepts that work together to provide clean, explicit caching boundaries:

1. **Serializable Entities** — Type-safe data models
2. **Deterministic Cache Keys** — Consistent, predictable addressing
3. **Explicit Cache Boundaries** — Clear ownership and behavior
4. **Cache Invalidation Control** — You decide when data becomes stale

These concepts are **intentionally simple** and avoid framework-specific abstractions.

---

## Serializable Entities

An entity in cache-kit is any Rust type that can be:

1. **Serialized** to bytes (for storage in cache)
2. **Deserialized** from bytes (for retrieval from cache)
3. **Cloned** (for internal cache operations)
4. **Identified** by a unique key

### The CacheEntity Trait

```rust
use cache_kit::CacheEntity;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct User {
    id: String,
    name: String,
    email: String,
}

impl CacheEntity for User {
    type Key = String;

    fn cache_key(&self) -> Self::Key {
        self.id.clone()
    }

    fn cache_prefix() -> &'static str {
        "user"
    }
}
```

### What Makes an Entity Cacheable?

| Requirement | Purpose |
|------------|---------|
| `Clone` | Cache operations need to duplicate entities |
| `Serialize` | Convert to bytes for storage |
| `Deserialize` | Convert from bytes for retrieval |
| `Send + Sync` | Safe to share across threads |
| `cache_key()` | Unique identifier for this entity |
| `cache_prefix()` | Namespace for entity type |

### Cache Key Construction

The final cache key is constructed as:

```
{prefix}:{key}
```

For the User example above:

```rust
let user = User {
    id: "user_001".to_string(),
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};

// Final cache key: "user:user_001"
```

This pattern ensures:
- **No collisions** between different entity types
- **Predictable keys** for debugging and monitoring
- **Type safety** at compile time

---

## Deterministic Cache Keys

Cache keys must be **deterministic** — given the same entity, you always get the same key.

### Good Key Examples

```rust
// ✅ Simple ID
impl CacheEntity for User {
    type Key = String;
    fn cache_key(&self) -> Self::Key {
        self.id.clone()
    }
}

// ✅ Composite key
impl CacheEntity for OrderItem {
    type Key = String;
    fn cache_key(&self) -> Self::Key {
        format!("{}:{}", self.order_id, self.item_id)
    }
}

// ✅ Numeric ID
impl CacheEntity for Product {
    type Key = u64;
    fn cache_key(&self) -> Self::Key {
        self.product_id
    }
}
```

### Anti-Patterns to Avoid

```rust
// ❌ Non-deterministic (timestamp)
fn cache_key(&self) -> String {
    format!("{}:{}", self.id, SystemTime::now().timestamp())
}

// ❌ Non-deterministic (random)
fn cache_key(&self) -> String {
    format!("{}:{}", self.id, rand::random::<u64>())
}

// ❌ Overly complex (hash collisions possible)
fn cache_key(&self) -> String {
    format!("{:x}", calculate_hash(&self))
}
```

**Rule:** Cache keys should depend **only** on stable entity attributes.

---

## Explicit Cache Boundaries

cache-kit uses a **feeder pattern** to define explicit cache boundaries.

### The CacheFeed Trait

A feeder acts as a bridge between cache-kit and your application:

```rust
use cache_kit::CacheFeed;

struct UserFeeder {
    id: String,
    user: Option<User>,
}

impl CacheFeed<User> for UserFeeder {
    fn entity_id(&mut self) -> String {
        self.id.clone()
    }

    fn feed(&mut self, entity: Option<User>) {
        self.user = entity;
    }
}
```

### Why Feeders?

Feeders provide several benefits:

1. **Explicit data flow** — You control where cached data goes
2. **Type safety** — Compiler enforces correct usage
3. **No hidden state** — No implicit global caches
4. **Testability** — Easy to mock and verify

### Feeder Lifecycle

```
1. Create feeder with entity ID
        ↓
2. Pass feeder to cache expander
        ↓
3. Cache expander calls entity_id()
        ↓
4. Cache hit → feed() called with entity
   Cache miss → fetch from repository → feed() called
        ↓
5. Application reads entity from feeder
```

### Example: Using a Feeder

```rust
// 1. Create feeder with the ID you want to fetch
let mut feeder = UserFeeder {
    id: "user_001".to_string(),
    user: None,
};

// 2. Execute cache operation
expander.with(&mut feeder, &repository, CacheStrategy::Refresh)?;

// 3. Access the result
if let Some(user) = feeder.user {
    println!("Found user: {}", user.name);
} else {
    println!("User not found");
}
```

---

## Cache Strategies

cache-kit provides four explicit cache strategies:

### 1. Fresh (Cache-Only)

```rust
CacheStrategy::Fresh
```

- **Behavior:** Return entity from cache, or `None` if not cached
- **Use case:** When you ONLY want cached data, never database
- **Example:** Real-time dashboards showing last known state

```rust
cache.execute(&mut feeder, &repository, CacheStrategy::Fresh).await?;

match feeder.user {
    Some(user) => println!("Cached user: {}", user.name),
    None => println!("Not in cache"),
}
```

### 2. Refresh (Cache + Database Fallback)

```rust
CacheStrategy::Refresh
```

- **Behavior:** Try cache first, fallback to database on miss, then cache the result
- **Use case:** **Default and recommended** for most operations
- **Example:** User profile lookups, product details

```rust
cache.execute(&mut feeder, &repository, CacheStrategy::Refresh).await?;

// Will always have data (if it exists in DB)
if let Some(user) = feeder.user {
    println!("User: {}", user.name);
}
```

### 3. Invalidate (Clear + Refresh)

```rust
CacheStrategy::Invalidate
```

- **Behavior:** Remove from cache, fetch from database, cache the fresh result
- **Use case:** After updates/writes to ensure fresh data
- **Example:** After user updates profile

```rust
// User updated their profile
repository.update_user(&updated_user)?;

// Invalidate cache and fetch fresh data
expander.with(&mut feeder, &repository, CacheStrategy::Invalidate)?;
```

### 4. Bypass (Database-Only)

```rust
CacheStrategy::Bypass
```

- **Behavior:** Skip cache entirely, always fetch from database
- **Use case:** One-off queries, debugging, auditing
- **Example:** Admin operations that need guaranteed fresh data

```rust
// Always fetch from database, ignore cache
cache.execute(&mut feeder, &repository, CacheStrategy::Bypass).await?;
```

### Strategy Decision Tree

```
Need data?
  ├─ Only cached? → Fresh
  ├─ Fresh from DB required? → Invalidate or Bypass
  ├─ Normal read? → Refresh (default)
  └─ Debugging? → Bypass
```

---

## Data Repository Pattern

cache-kit is agnostic to your data source. You define how to fetch entities:

### The DataRepository Trait

```rust
use cache_kit::DataRepository;

pub trait DataRepository<T: CacheEntity>: Send + Sync {
    async fn fetch_by_id(&self, id: &T::Key) -> cache_kit::Result<Option<T>>;
}
```

### Example: SQLx Repository

```rust
use sqlx::PgPool;

struct UserRepository {
    pool: PgPool,
}

impl DataRepository<User> for UserRepository {
    async fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<User>> {
        let user = sqlx::query_as!(
            User,
            "SELECT id, name, email FROM users WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        Ok(user)
    }
}
```

### Example: In-Memory Repository (for Testing)

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct InMemoryRepository {
    data: Arc<Mutex<HashMap<String, User>>>,
}

impl DataRepository<User> for InMemoryRepository {
    async fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<User>> {
        let data = self.data.lock().unwrap();
        Ok(data.get(id).cloned())
    }
}
```

### Repository Best Practices

✅ **DO:**
- Keep repositories focused on data fetching only
- Return `Option<T>` to distinguish "not found" from errors
- Use proper error types (convert DB errors to cache-kit errors)
- Make repositories cloneable (`Arc` wrapper)

❌ **DON'T:**
- Put cache logic inside repositories
- Mix business logic with data access
- Assume entities exist (always return Option)
- Panic on database errors

---

## Cache Ownership and Invalidation

You own cache invalidation. cache-kit does not:

- Automatically invalidate on writes
- Track entity relationships
- Provide distributed invalidation
- Guess when data is stale

### Invalidation Patterns

#### Pattern 1: Invalidate After Write

```rust
use cache_kit::{CacheService, backend::InMemoryBackend, CacheStrategy};

pub struct UserService {
    cache: CacheService<InMemoryBackend>,
    repository: UserRepository,
}

impl UserService {
    pub async fn update_user(&self, user: &User) -> cache_kit::Result<()> {
        // 1. Update database
        self.repository.update(user).await?;

        // 2. Invalidate cache
        let mut feeder = UserFeeder {
            id: user.id.clone(),
            user: None,
        };
        self.cache.execute(
            &mut feeder,
            &self.repository,
            CacheStrategy::Invalidate
        ).await?;

        Ok(())
    }
}
```

#### Pattern 2: TTL-Based Expiry

```rust
use std::time::Duration;
use cache_kit::{CacheService, backend::InMemoryBackend};

// Create cache (TTL managed by backend configuration)
let cache = CacheService::new(InMemoryBackend::new());

// Data expiry is configured at backend level
cache.execute(&mut feeder, &repository, CacheStrategy::Refresh).await?;
```

#### Pattern 3: Event-Driven Invalidation

```rust
use cache_kit::{CacheService, backend::InMemoryBackend, CacheStrategy};

// When user updates profile via event
async fn on_user_updated(
    cache: &CacheService<InMemoryBackend>,
    repository: &UserRepository,
    event: UserUpdatedEvent
) -> cache_kit::Result<()> {
    let mut feeder = UserFeeder {
        id: event.user_id,
        user: None,
    };

    // Clear cache for this user
    cache.execute(&mut feeder, repository, CacheStrategy::Invalidate).await?;
    Ok(())
}
```

---

## Putting It All Together

Here's how all concepts work together:

```rust
use cache_kit::{
    CacheEntity, CacheFeed, DataRepository, CacheService,
    backend::InMemoryBackend,
    strategy::CacheStrategy,
};
use serde::{Deserialize, Serialize};

// 1. Entity (Serializable)
#[derive(Clone, Serialize, Deserialize)]
struct Product {
    id: u64,
    name: String,
    price: f64,
}

// 2. Deterministic cache key
impl CacheEntity for Product {
    type Key = u64;
    fn cache_key(&self) -> Self::Key { self.id }
    fn cache_prefix() -> &'static str { "product" }
}

// 3. Explicit cache boundary (Feeder)
struct ProductFeeder {
    id: u64,
    product: Option<Product>,
}

impl CacheFeed<Product> for ProductFeeder {
    fn entity_id(&mut self) -> u64 { self.id }
    fn feed(&mut self, entity: Option<Product>) { self.product = entity; }
}

// 4. Data repository
struct ProductRepository;

impl DataRepository<Product> for ProductRepository {
    async fn fetch_by_id(&self, id: &u64) -> cache_kit::Result<Option<Product>> {
        // Your database logic
        Ok(Some(Product {
            id: *id,
            name: "Example Product".to_string(),
            price: 99.99,
        }))
    }
}

// Usage
#[tokio::main]
async fn main() -> cache_kit::Result<()> {
    let cache = CacheService::new(InMemoryBackend::new());
    let repository = ProductRepository;

    let mut feeder = ProductFeeder {
        id: 123,
        product: None,
    };

    // Cache operation with explicit strategy
    cache.execute(&mut feeder, &repository, CacheStrategy::Refresh).await?;

    if let Some(product) = feeder.product {
        println!("Product: {} - ${}", product.name, product.price);
    }

    Ok(())
}
```

---

## Next Steps

- [Install and configure](installation) cache-kit in your project
- Learn about [Database & ORM compatibility](database-compatibility)
- Explore [Serialization options](serialization)
- Review [Cache backend choices](backends)
