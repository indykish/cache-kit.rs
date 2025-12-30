# Cache-Kit Actix Web Example (Service Layer Pattern)

This example demonstrates the **Service Layer architecture pattern** for integrating cache-kit with Actix Web, providing clean separation of concerns and maintainable code structure.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    HTTP Layer (Routes)                   │
│  • Handles HTTP requests/responses only                 │
│  • Delegates to services                                │
│  • Clean, minimal boilerplate                           │
└────────────────┬────────────────────────────────────────┘
                 │
┌────────────────▼────────────────────────────────────────┐
│              Service Layer (Business Logic)              │
│  • Coordinates between repository and cache             │
│  • Implements business logic                            │
│  • Manages cache strategies                             │
└────────────┬──────────────┬─────────────────────────────┘
             │              │
             │              │
   ┌─────────▼────────┐    │
   │   Cache Layer    │    │
   │  (InMemoryBackend│    │
   │      or Redis)   │    │
   └──────────────────┘    │
                           │
                  ┌────────▼───────────┐
                  │  Repository Layer  │
                  │  (Database Access) │
                  │  • Pure DB logic   │
                  │  • No cache aware  │
                  └────────────────────┘
```

## Why Service Layer?

### ✅ Advantages

1. **Clear Separation of Concerns**
   - Routes: HTTP only
   - Services: Business logic + caching
   - Repositories: DB access only

2. **Intuitive for Developers**
   - Cache logic lives where it makes sense (service layer)
   - Routes are clean: `data.user_service.get(&id)`
   - Models stay pure

3. **Testable**
   - Test repositories without cache
   - Test services with mock repositories
   - Test routes with mock services

4. **Scalable**
   - Easy to add transactions, validation, authorization
   - Services can orchestrate multiple repositories
   - Reusable in CLI, tests, other frameworks

## Project Structure

```
examples/actixsqlx/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs                    # Server setup + DI
    ├── models.rs                  # User, Product entities
    ├── repository.rs              # DB access (no cache)
    ├── routes.rs                  # HTTP handlers
    └── services/
        ├── mod.rs
        ├── user_service.rs        # User business logic + cache
        └── product_service.rs     # Product business logic + cache
```

## Code Examples

### Repository Layer (Pure DB Access)

```rust
// repository.rs - No cache knowledge
impl DataRepository<User> for UserRepository {
    fn fetch_by_id(&self, id: &String) -> Result<Option<User>> {
        log::info!("[DB] Fetching user: {}", id);
        Ok(self.users.lock().unwrap().get(id).cloned())
    }
}
```

### Service Layer (Cache Integration)

```rust
// services/user_service.rs
impl UserService {
    pub fn get(&self, id: &str) -> Result<Option<User>> {
        let mut feeder = UserFeeder { id: id.to_string(), user: None };

        self.cache.lock().unwrap()
            .with(&mut feeder, &*self.repo, CacheStrategy::Refresh)?;

        Ok(feeder.user)
    }

    pub fn create(&self, user: User) -> Result<User> {
        let created = self.repo.create(user)?;

        // Cache the new user
        let mut feeder = UserFeeder {
            id: created.id.clone(),
            user: Some(created.clone())
        };

        self.cache.lock().unwrap()
            .with(&mut feeder, &*self.repo, CacheStrategy::Refresh);

        Ok(created)
    }
}
```

### Route Layer (Clean HTTP)

```rust
// routes.rs - Just HTTP, delegates to service
pub async fn get_user(
    path: web::Path<String>,
    data: web::Data<AppState>
) -> impl Responder {
    let user_id = path.into_inner();

    match data.user_service.get(&user_id) {
        Ok(Some(user)) => HttpResponse::Ok().json(user),
        Ok(None) => HttpResponse::NotFound().json(...),
        Err(e) => HttpResponse::InternalServerError().json(...)
    }
}
```

### Dependency Injection (main.rs)

```rust
// Build layers bottom-up
let cache_expander = Arc::new(Mutex::new(CacheExpander::new(backend)));
let user_repo = Arc::new(UserRepository::new());

let user_service = Arc::new(UserService::new(
    user_repo.clone(),
    cache_expander.clone(),
));

let app_state = web::Data::new(AppState { user_service });
```

## Running the Example

```bash
# Build the example
cd examples/actixsqlx
cargo build

# Run the server
cargo run

# Test the API
curl http://localhost:8080/health
curl http://localhost:8080/users/user_001
curl -X POST http://localhost:8080/users \
  -H "Content-Type: application/json" \
  -d '{"id":"user_003","username":"charlie","email":"charlie@example.com","created_at":"2025-12-26T00:00:00Z"}'
```

## API Endpoints

| Method | Endpoint | Description | Cache Strategy |
|--------|----------|-------------|----------------|
| GET | `/health` | Health check | None |
| GET | `/users/:id` | Get user | Refresh (cache + DB) |
| POST | `/users` | Create user | Refresh (cache after create) |
| PUT | `/users/:id` | Update user | Invalidate + Refresh |
| DELETE | `/users/:id` | Delete user | Invalidate |
| GET | `/products/:id` | Get product | Refresh (cache + DB) |
| POST | `/products` | Create product | Refresh (cache after create) |
| PUT | `/products/:id` | Update product | Invalidate + Refresh |
| DELETE | `/products/:id` | Delete product | Invalidate |

## Cache Strategies Demonstrated

1. **Refresh** (GET operations)
   - Check cache first
   - On miss, fetch from DB and store in cache
   - On hit, return from cache

2. **Invalidate** (PUT/DELETE operations)
   - Remove from cache
   - Force fresh fetch on next read

## Comparison with Other Patterns

### Service Layer (This Example) ⭐
```rust
// Routes
match data.user_service.get(&id) {
    Ok(Some(user)) => HttpResponse::Ok().json(user),
    ...
}
```

**Pros**: Clean routes, centralized cache logic, testable
**Cons**: Extra layer

### Routes Handle Cache (Alternative)
```rust
// Routes
let mut feeder = UserFeeder { id: user_id.clone(), user: None };
data.cache_expander.lock().unwrap()
    .with(&mut feeder, &*data.user_repo, CacheStrategy::Refresh)?;
```

**Pros**: No extra layer
**Cons**: Verbose, routes too complex

### Tightly Coupled (Legacy)
```rust
// Repository
pub fn get(&self, id: &str, cached: bool) -> Result<Option<User>> {
    if cached {
        return self.cache.get(id);
    }
    self.db.query(id)
}
```

**Pros**: Simple API
**Cons**: Repository coupled to cache, hard to test

## Testing

This example includes comprehensive test suites inspired by [Exonum's test patterns](https://github.com/exonum/exonum/tree/master/test-suite):

### Test Files

1. **`tests/api_integration_tests.rs`** - Basic API integration tests
   - CRUD operations (Create, Read, Update, Delete)
   - Cache hit/miss verification
   - Cache invalidation on updates
   - HTTP status code validation

2. **`tests/advanced_integration_tests.rs`** - Advanced patterns ✨
   - **Concurrency Tests** (inspired by `exonum/soak-tests/send_txs.rs`)
     - Concurrent reads of same entity (cache safety)
     - Read-after-write consistency
   - **Error Handling Tests** (inspired by `exonum/testkit/tests/api.rs`)
     - Invalid UUID format handling
     - Empty/null input validation
     - Oversized payload protection
   - **Performance Tests** (inspired by `exonum/soak-tests` timing patterns)
     - Cache hit latency benchmarking
     - TimingStats (avg/min/max tracking)

### Running Tests

```bash
# Start database
make up

# Run all tests
cargo test --test api_integration_tests -- --test-threads=1
cargo test --test advanced_integration_tests -- --test-threads=1

# Run specific test
cargo test test_concurrent_reads_same_user -- --test-threads=1
```

### Test Patterns from Exonum

The advanced tests demonstrate patterns learned from Exonum's test suite:

| Pattern | Exonum Source | Our Implementation |
|---------|---------------|-------------------|
| Concurrent operations | `soak-tests/send_txs.rs` | `test_concurrent_reads_same_user` |
| Read-after-write | `testkit/tests/counter/main.rs:90-103` | `test_read_after_write_consistency` |
| Error responses | `testkit/tests/api.rs:120-149` | `test_invalid_uuid_format` |
| Timing statistics | `soak-tests/send_txs.rs:81-112` | `TimingStats` struct + `test_cache_performance_timing` |

### Performance Benchmarks

Expected results from `test_cache_performance_timing`:
- **Cache Hits**: < 10ms average (in-memory backend)
- **Cache Misses**: < 50ms average (includes DB query)
- **Concurrent Load**: 100 requests in < 5 seconds

## Next Steps

- [x] Add SQLX for real PostgreSQL integration ✅
- [x] Add database migrations ✅
- [x] Add integration tests ✅
- [x] Add advanced concurrency tests ✅
- [ ] Add Redis backend option
- [ ] Add metrics/observability
- [ ] Add request tracing
- [ ] Add remaining cache strategy tests (Fresh, Bypass)
- [ ] Add thundering herd test

## References

- [Cache-Kit Documentation](../../README.md)
- [Actix Web Documentation](https://actix.rs/)
- [Service Layer Pattern](https://martinfowler.com/eaaCatalog/serviceLayer.html)
