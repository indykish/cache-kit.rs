---
layout: single
title: Positioning & Design Principles
description: "Understanding cache-kit's philosophy and design decisions"
permalink: /design-principles/
---




---

## Core Philosophy

cache-kit is designed around three fundamental principles:

1. **Boundaries, not ownership**
2. **Explicit behavior, not hidden magic**
3. **Integration, not lock-in**

These principles guide every design decision in cache-kit.

---

## Boundaries, Not Ownership

cache-kit does not try to own your application stack.

### What This Means

cache-kit integrates **around** your existing choices:

```
┌─────────────────────────────────────────┐
│           Your Choices                  │
│  • Framework (Axum, Actix, Tonic)       │
│  • ORM (SQLx, SeaORM, Diesel)           │
│  • Transport (HTTP, gRPC, Workers)      │
│  • Runtime (tokio)                      │
└──────────────┬──────────────────────────┘
               │
               ↓ Cache operations
┌─────────────────────────────────────────┐
│          cache-kit                      │
│  Places clear boundaries               │
│  Does NOT dictate architecture         │
└─────────────────────────────────────────┘
```

### Design Decisions

| What cache-kit Does | What cache-kit Does NOT Do |
|---------------------|----------------------------|
| ✅ Provide cache operations | ❌ Replace your ORM |
| ✅ Define cache boundaries | ❌ Manage HTTP routing |
| ✅ Handle serialization | ❌ Impose web frameworks |
| ✅ Support multiple backends | ❌ Require specific databases |
| ✅ Integrate with async | ❌ Create runtimes |

### Benefits

- **Freedom of choice** — Use any framework, ORM, transport
- **Evolutionary architecture** — Swap components independently
- **Library-safe** — Use inside SDKs and libraries
- **No vendor lock-in** — cache-kit is just one piece

---

## Explicit Behavior, Not Hidden Magic

cache-kit makes cache behavior **visible and predictable**.

### No Implicit Caching

```rust
// ❌ WRONG: Hidden caching (magic)
fn get_user(id: &str) -> User {
    // Automatically cached somewhere?
    // How? When? For how long?
    database.query(id)
}

// ✅ RIGHT: Explicit caching (cache-kit)
fn get_user(id: &str) -> Result<Option<User>> {
    let mut feeder = UserFeeder { id: id.to_string(), user: None };

    // Explicit: I know this uses cache
    // Explicit: I chose the strategy
    // Explicit: I control the result
    cache.with(&mut feeder, &repository, CacheStrategy::Refresh)?;

    Ok(feeder.user)
}
```

### Explicit Invalidation

cache-kit does NOT:
- Automatically invalidate on writes
- Guess when data is stale
- Track entity relationships
- Provide "magic" cache eviction

**You decide** when to invalidate:

```rust
impl UserService {
    pub async fn update_user(&self, user: User) -> Result<User> {
        // 1. Update database
        let updated = self.repo.update(&user).await?;

        // 2. Explicitly invalidate cache
        let mut feeder = UserFeeder {
            id: updated.id.clone(),
            user: None,
        };
        self.cache.with(&mut feeder, &self.repo, CacheStrategy::Invalidate)?;

        Ok(updated)
    }
}
```

### Explicit Strategies

Four cache strategies, each with clear semantics:

| Strategy | Behavior | Use When |
|----------|----------|----------|
| `Fresh` | Cache-only | You ONLY want cached data |
| `Refresh` | Cache + DB fallback | Normal reads (default) |
| `Invalidate` | Clear + refresh | After writes |
| `Bypass` | Skip cache | Debugging, auditing |

No guessing. No surprises.

---

## Integration, Not Lock-In

cache-kit is designed to **play well with others**.

### Framework Agnostic

Works with any framework:

```rust
// Axum
async fn axum_handler(State(cache): State<Arc<CacheExpander<_>>>) -> Result<Json<User>> {
    // Same cache operations
}

// Actix
async fn actix_handler(data: web::Data<AppState>) -> HttpResponse {
    // Same cache operations
}

// Tonic (gRPC)
async fn grpc_method(&self, request: Request<UserRequest>) -> Result<Response<UserResponse>> {
    // Same cache operations
}
```

The **same cache logic** works across all frameworks.

### ORM Agnostic

Works with any database layer:

```rust
// SQLx
impl DataRepository<User> for SqlxRepository {
    async fn fetch_by_id(&self, id: &String) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        Ok(user)
    }
}

// SeaORM
impl DataRepository<User> for SeaOrmRepository {
    async fn fetch_by_id(&self, id: &String) -> Result<Option<User>> {
        let user = Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        Ok(user)
    }
}
```

### Backend Agnostic

Swap backends with **zero code changes**:

```rust
// Development
let backend = InMemoryBackend::new();

// Production
let backend = RedisBackend::new(config)?;

// Same interface
let expander = CacheExpander::new(backend);
```

---

## Guarantees and Non-Guarantees

cache-kit is explicit about what it **guarantees** and what it **does not**.

### What cache-kit Guarantees

✅ **Type safety** — Compiler-verified cache operations
✅ **Thread safety** — `Send + Sync` everywhere
✅ **Deterministic keys** — Same entity → same key
✅ **No silent failures** — All errors are propagated
✅ **Backend abstraction** — Swap backends without code changes
✅ **Async-first** — Built for tokio-based apps

### What cache-kit Does NOT Guarantee

❌ **Strong consistency** — Distributed caches are eventually consistent
❌ **Automatic invalidation** — You control when data is invalidated
❌ **Distributed coordination** — No locks, no consensus
❌ **Eviction policies** — Depends on backend (Redis, Memcached)
❌ **Persistence** — Depends on backend (Redis has persistence, Memcached doesn't)
❌ **Cross-language compatibility** — Postcard is Rust-only

---

## Design Patterns

### Service Layer Pattern (Recommended)

```
HTTP Handler → Service Layer → Cache → Repository → Database
```

**Benefits:**
- Clean separation of concerns
- Reusable across transports
- Testable in isolation

**Example:**

```rust
pub struct UserService {
    cache: Arc<CacheExpander<RedisBackend>>,
    repo: Arc<UserRepository>,
}

impl UserService {
    // Business logic + caching
    pub fn get_user(&self, id: &str) -> Result<Option<User>> {
        let mut feeder = UserFeeder { id: id.to_string(), user: None };
        self.cache.with(&mut feeder, &*self.repo, CacheStrategy::Refresh)?;
        Ok(feeder.user)
    }
}

// Use in HTTP handler
async fn handler(service: Arc<UserService>) -> Result<Json<User>> {
    service.get_user("user_001")  // Clean and simple
}

// Use in gRPC handler
async fn grpc_handler(service: Arc<UserService>) -> Result<Response<UserResponse>> {
    service.get_user("user_001")  // Same logic!
}
```

### Repository Pattern

```rust
// Repository: Only data access
impl DataRepository<User> for UserRepository {
    fn fetch_by_id(&self, id: &String) -> Result<Option<User>> {
        // Pure database logic
    }
}

// Service: Business logic + caching
impl UserService {
    pub fn get_user(&self, id: &str) -> Result<Option<User>> {
        // Cache coordination
    }

    pub async fn update_user(&self, user: User) -> Result<User> {
        // Write + cache invalidation
    }
}
```

### Feeder Pattern

```rust
// Feeder: Explicit data flow
struct UserFeeder {
    id: String,
    user: Option<User>,
}

impl CacheFeed<User> for UserFeeder {
    fn entity_id(&mut self) -> String { self.id.clone() }
    fn feed(&mut self, entity: Option<User>) { self.user = entity; }
}

// Usage: Clear and traceable
let mut feeder = UserFeeder { id: "user_001".to_string(), user: None };
cache.with(&mut feeder, &repo, CacheStrategy::Refresh)?;
println!("Result: {:?}", feeder.user);  // Explicit data flow
```

---

## Trade-Offs and Honesty

cache-kit makes intentional trade-offs and is honest about them.

### Trade-Off 1: Postcard vs JSON

| Aspect | Postcard (Chosen) | JSON (Alternative) |
|--------|-------------------|--------------------|
| **Performance** | ⚡ 10-15x faster | ❌ Baseline |
| **Size** | 📦 40-50% smaller | ❌ Baseline |
| **Decimal support** | ❌ No | ✅ Yes |
| **Language support** | ❌ Rust-only | ✅ Many languages |

**Decision:** Prioritize performance for Rust-to-Rust caching. Decimal limitation is documented and workarounds are provided.

### Trade-Off 2: Async DataRepository

| Aspect | Async (Chosen) |
|--------|----------------|
| **Native async support** | ✅ Direct `.await` |
| **Modern Rust practices** | ✅ Idiomatic async/await |
| **Compatibility** | ✅ SQLx, SeaORM, tokio-postgres |
| **Ecosystem alignment** | ✅ Works with modern async frameworks |

**Decision:** Use async trait for modern async databases. This is the recommended pattern for Rust services.

### Trade-Off 3: Explicit Invalidation vs Automatic

| Aspect | Explicit (Chosen) | Automatic (Alternative) |
|--------|-------------------|-------------------------|
| **Control** | ✅ Full control | ❌ Hidden behavior |
| **Predictability** | ✅ Predictable | ⚠️ Can surprise you |
| **Complexity** | ✅ Simple | ❌ Complex dependency tracking |

**Decision:** Make invalidation explicit. No magic, no surprises.

---

## Safety and Reliability

### Thread Safety

All cache-kit types are `Send + Sync`:

```rust
// Safe to share across threads
let cache = Arc::new(CacheExpander::new(backend));

// Safe to use in async tasks
tokio::spawn(async move {
    let mut feeder = UserFeeder { ... };
    cache.with(&mut feeder, &repo, CacheStrategy::Refresh)?;
});
```

### Error Handling

cache-kit **never panics** in normal operation:

```rust
// All operations return Result
match cache.with(&mut feeder, &repo, CacheStrategy::Refresh) {
    Ok(_) => println!("Success"),
    Err(e) => eprintln!("Cache error: {}", e),
}
```

### Memory Safety

- No unsafe code in cache-kit core
- All backends use safe Rust
- DashMap (InMemory) is lock-free and safe

---

## Library and SDK Use

cache-kit is **safe to use inside libraries**:

```rust
// Inside a library crate
pub struct MyLibrary {
    cache: CacheExpander<InMemoryBackend>,
    // or bring-your-own-backend pattern
}

impl MyLibrary {
    pub fn new() -> Self {
        Self {
            cache: CacheExpander::new(InMemoryBackend::new()),
        }
    }

    // Your library methods
    pub fn fetch_data(&mut self, id: &str) -> Result<Data> {
        let mut feeder = DataFeeder { ... };
        self.cache.with(&mut feeder, &self.repo, CacheStrategy::Refresh)?;
        // ...
    }
}
```

**Benefits:**
- No framework dependencies
- No global state
- No runtime assumptions
- Safe to embed

---

## When NOT to Use cache-kit

cache-kit is **not** the right choice if you need:

❌ **Distributed locks** — Use a coordination service (etcd, ZooKeeper)
❌ **Strong consistency** — Use a distributed database (Spanner, CockroachDB)
❌ **Cross-language caching** — Use JSON or MessagePack (when available)
❌ **Automatic schema migration** — cache-kit uses explicit versioning
❌ **All-in-one framework** — cache-kit is just a caching library

---

## Design Goals Summary

| Goal | How cache-kit Achieves It |
|------|---------------------------|
| **Simple** | Four cache strategies, clear semantics |
| **Fast** | Postcard serialization, async-first |
| **Type-safe** | Compile-time verified operations |
| **Flexible** | Works with any ORM, framework, backend |
| **Honest** | Explicit about trade-offs and limitations |
| **Predictable** | No magic, explicit behavior |
| **Safe** | Send + Sync, no panics, safe Rust |
| **Integrable** | Fits into existing architectures |

---

## Contributing to cache-kit

cache-kit follows these principles in all contributions:

### Code Contributions

✅ **Preferred:**
- Backend implementations (MessagePack, new cache backends)
- ORM examples (SeaORM, Diesel)
- Documentation improvements
- Bug fixes with tests

⚠️ **Discouraged:**
- Breaking API changes without strong justification
- Framework-specific features
- Magic or implicit behavior
- Features that increase complexity significantly

### Documentation Contributions

✅ **Encouraged:**
- Real-world examples
- Integration guides
- Performance comparisons
- Best practice documentation

See [CONTRIBUTING.md](https://github.com/megamsys/cache-kit.rs/blob/main/CONTRIBUTING.md) for details.

---

## References and Inspiration

cache-kit is inspired by:

- **SeaORM** — Clean, composable Rust ORM
- **Exonum** — Type-safe service boundaries
- **Rust ecosystem** — async-first, zero-cost abstractions

---

## Next Steps

- Start with [Installation](installation)
- Review [Core Concepts](concepts)
- Explore the [Actix + SQLx reference implementation](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx)
- Join the community on [GitHub](https://github.com/megamsys/cache-kit.rs)
