---
layout: single
title: Production Troubleshooting
parent: Guides
---

# Production Troubleshooting Guide

Diagnose and resolve cache-kit issues in production environments.



---

## Overview

This guide covers the most common cache-kit issues, how to diagnose them, and how to fix them.

### Diagnostic Workflow

```
Issue Observed
    ↓
Identify Category (Connection? Performance? Data?)
    ↓
Gather Logs & Metrics
    ↓
Check Backend Health
    ↓
Apply Fix
    ↓
Verify Resolution
```

### Tools You'll Need

```bash
# Redis diagnosis
redis-cli
redis-cli --latency
redis-cli --stat

# Memcached diagnosis
echo "stats" | nc localhost 11211
memcached-tool localhost:11211

# Application logs
grep "cache" app.log | grep "error"

# System metrics
top
vmstat
netstat
```

---

## Common Issues & Solutions

### Issue 1: Low Cache Hit Rate (< 30%)

**Symptoms:**
- Hit rate stuck at 10-20%
- Cache size not growing
- High database load despite caching

**Possible Causes:**
1. Keys are not being reused (different key each time)
2. TTL is too short (entries expire quickly)
3. Cache keys are non-deterministic
4. Cache is being cleared unexpectedly
5. New users/data not being cached

#### Diagnosis Steps

**Step 1: Check hit/miss rates**
```rust
pub struct CacheMetrics {
    hits: AtomicU64,
    misses: AtomicU64,
}

impl CacheMetrics {
    pub fn hit_rate(&self) -> f64 {
        let h = self.hits.load(Ordering::Relaxed) as f64;
        let m = self.misses.load(Ordering::Relaxed) as f64;
        h / (h + m)
    }
}

// In your metrics endpoint
println!("Cache hit rate: {:.1}%", metrics.hit_rate() * 0.9.0);
```

**Step 2: Verify cache keys are deterministic**
```rust
// ✅ Good: Same input always produces same key
impl CacheEntity for User {
    type Key = String;
    fn cache_key(&self) -> Self::Key {
        self.id.clone()  // Deterministic
    }
}

// ❌ Bad: Different keys for same entity
impl CacheEntity for User {
    type Key = String;
    fn cache_key(&self) -> Self::Key {
        // WRONG: Creates new key each time!
        format!("{}:{}", self.id, SystemTime::now().timestamp())
    }
}
```

**Step 3: Check TTL configuration**
```bash
# Redis: Check expiration times
redis-cli TTL "user:123"
# Output: 3600 (seconds remaining)
# Output: -1 (no expiration set - cache forever!)
# Output: -2 (key doesn't exist)

# If TTL is -1 or very short, that's your problem
```

**Step 4: Verify cache strategy**
```rust
// If using CacheStrategy::Fresh instead of Refresh, you miss database writes
// ❌ Wrong: Always cache-only
expander.with(&mut feeder, &repo, CacheStrategy::Fresh)?;

// ✅ Correct: Cache with database fallback
expander.with(&mut feeder, &repo, CacheStrategy::Refresh)?;
```

#### Solutions

**Solution 1A: Set appropriate TTL**
```rust
// Before: No TTL
let expander = CacheExpander::new(backend);

// After: 1-hour TTL for user data
let expander = CacheExpander::builder()
    .with_backend(backend)
    .with_ttl(Duration::from_secs(3600))
    .build();
```

**Solution 1B: Use Refresh strategy**
```rust
// Ensure using Refresh, not Fresh
expander.with(&mut feeder, &repo, CacheStrategy::Refresh)?;
//              ↑ Try cache first, fallback to DB if miss
```

**Solution 1C: Verify key uniqueness**
```rust
// Log all cache keys to find issues
impl CacheEntity for User {
    type Key = String;
    fn cache_key(&self) -> Self::Key {
        let key = self.id.clone();
        debug!("Cache key for user: {}", key);  // Add this
        key
    }
}

// Check logs for duplicate/varying keys
grep "Cache key for user" app.log | sort | uniq -c
```

**Solution 1D: Check for cache invalidation logic**
```rust
// Verify you're not over-invalidating
// This is correct: Invalidate on write
pub fn update_user(&self, user: &User) -> Result<()> {
    // 1. Update database
    self.repo.update(user)?;
    
    // 2. Invalidate OLD version
    let mut feeder = UserFeeder { id: user.id.clone(), user: None };
    self.cache.with(&mut feeder, &self.repo, CacheStrategy::Invalidate)?;
    
    // 3. Cache NEW version
    let mut feeder = UserFeeder { id: user.id.clone(), user: Some(user.clone()) };
    self.cache.with(&mut feeder, &self.repo, CacheStrategy::Refresh)?;
    
    Ok(())
}
```

### Issue 2: Backend Connection Timeouts

**Symptoms:**
- Request timeouts after N milliseconds
- "Connection refused" errors
- Pool exhaustion errors
- p99 latency spikes

**Possible Causes:**
1. Backend (Redis/Memcached) is down
2. Network connectivity issue
3. Connection pool size is too small
4. Timeout is set too aggressively

#### Diagnosis Steps

**Step 1: Verify backend is running**
```bash
# Redis
redis-cli ping
# Response: PONG (good)
# Response: (error) ERR... (bad - Redis down)

# Memcached
echo "stats" | nc -w 1 localhost 11211
# Response: STAT... (good)
# No response or error (bad - Memcached down)
```

**Step 2: Check network connectivity**
```bash
# Test network path
nc -zv localhost 6379
# Connected to localhost port 6379 (good)
# Connection refused (network issue)

# Check latency
redis-cli --latency
# Typical latency: < 1ms (good)
# Typical latency: > 10ms (slow network)
```

**Step 3: Examine connection pool status**
```rust
// Monitor pool metrics
pub struct PoolMetrics {
    active_connections: AtomicU64,
    waiting_requests: AtomicU64,
}

// Log pool status on errors
match cache.with(&mut feeder, &repo, CacheStrategy::Refresh) {
    Ok(_) => {},
    Err(e) if e.to_string().contains("pool") => {
        error!(
            "Pool exhausted: {} active, {} waiting",
            metrics.active_connections.load(Ordering::Relaxed),
            metrics.waiting_requests.load(Ordering::Relaxed)
        );
    }
    Err(e) => error!("Cache error: {}", e),
}
```

**Step 4: Check timeout configuration**
```rust
// Current timeout
let config = RedisConfig {
    connection_timeout: Duration::from_secs(5),
    // Is this too short for your network?
};
```

#### Solutions

**Solution 2A: Restart backend**
```bash
# Redis
redis-cli shutdown
redis-server

# Or with Docker
docker restart redis_container
```

**Solution 2B: Increase pool size**
```rust
// Before: Small pool
let config = RedisConfig {
    pool_size: 10,
    ..Default::default()
};

// After: Formula (CPU_cores × 2) + 1
let cores = num_cpus::get();
let config = RedisConfig {
    pool_size: (cores * 2 + 1) as u32,
    ..Default::default()
};
```

**Solution 2C: Increase timeout**
```rust
// Before: Very strict timeout
let config = RedisConfig {
    connection_timeout: Duration::from_secs(1),
    ..Default::default()
};

// After: More realistic
let config = RedisConfig {
    connection_timeout: Duration::from_secs(10),
    ..Default::default()
};
```

**Solution 2D: Add circuit breaker**
```rust
// See Error Handling guide for circuit breaker implementation
// This prevents cascading failures when backend is slow
let breaker = CircuitBreaker::new(5, Duration::from_secs(30));
```

---

### Issue 3: High Memory Usage

**Symptoms:**
- Cache backend consuming GB of RAM
- OOM killer triggering
- Eviction errors from backend
- Request latency increasing

**Possible Causes:**
1. Entries are too large (whole objects)
2. TTL not set (entries never expire)
3. Too many unique keys (unbounded growth)
4. No eviction policy configured
5. Memory leak in application

#### Diagnosis Steps

**Step 1: Check Redis memory usage**
```bash
redis-cli INFO memory
# Output:
# used_memory_human:2.5G  ← How much is being used
# maxmemory:3G            ← Maximum allowed
# evicted_keys:1000       ← Keys removed due to eviction
```

**Step 2: Estimate entry size**
```bash
redis-cli --bigkeys
# Output:
# Scanning database...
# [Hash] "employment:123" -> 512 bytes
# [String] "user:456" -> 128 bytes

# If entries are 512+ bytes, consider smaller DTOs
```

**Step 3: Analyze key count**
```bash
redis-cli DBSIZE
# Output: 5000000
# With 1KB entries: 5GB of data
# Check if all keys are needed
```

**Step 4: Check eviction policy**
```bash
redis-cli CONFIG GET maxmemory-policy
# Output: "allkeys-lru" (good - evicts least recently used)
# Output: "no-eviction" (bad - rejects new writes when full)
```

#### Solutions

**Solution 3A: Set appropriate TTL**
```rust
// Before: Cache forever
let expander = CacheExpander::new(backend);

// After: 1-hour TTL for users, 1-day for products
let user_cache = CacheExpander::builder()
    .with_backend(redis_backend)
    .with_ttl(Duration::from_secs(3600))  // 1 hour
    .build();

let product_cache = CacheExpander::builder()
    .with_backend(redis_backend)
    .with_ttl(Duration::from_secs(86400))  // 1 day
    .build();
```

**Solution 3B: Reduce entry size**
```rust
// Before: Cache entire User with all fields
#[derive(Serialize, Deserialize)]
struct CachedUser {
    id: String,
    name: String,
    email: String,
    password_hash: String,    // Unnecessary in cache
    profile_picture: Vec<u8>, // Too large
    bio: String,
    preferences: Vec<String>, // Heavy
}

// After: Cache only needed fields
#[derive(Serialize, Deserialize)]
struct CachedUser {
    id: String,
    name: String,
    email: String,
    // Skip: password, picture, preferences
}
```

**Solution 3C: Limit cache size**
```bash
# Redis: Set maximum memory
redis-cli CONFIG SET maxmemory 2gb
redis-cli CONFIG SET maxmemory-policy allkeys-lru

# Memcached: Set maximum memory at startup
memcached -m 2048  # 2GB
```

**Solution 3D: Monitor key count**
```rust
// Alert if key count grows unbounded
pub fn check_cache_health(metrics: &Metrics) {
    let key_count = get_redis_dbsize();
    
    if key_count > ALERT_THRESHOLD {
        alert!("Cache size growing: {} keys", key_count);
        // Investigate: Are keys not expiring?
    }
}
```

---

### Issue 4: Serialization Errors

**Symptoms:**
- "Serialization failed" errors
- "Version mismatch" errors
- "Invalid magic header" errors
- Some requests fail, others work

**Possible Causes:**
1. Entity type changed (schema mismatch)
2. Corrupted cache entry
3. Type contains unsupported fields (e.g., Decimal)
4. Different serialization formats

#### Diagnosis Steps

**Step 1: Check error logs**
```bash
grep -i "serialization" app.log | head -20
# Output:
# 2024-01-15 10:23:45 WARN: Serialization failed for user:123: Unsupported type
```

**Step 2: Identify affected entities**
```bash
grep -i "serialization" app.log | grep -o "user:[0-9]*" | sort | uniq
# Output: user:123, user:456, user:789
# All from same key? Different ones? Pattern?
```

**Step 3: Check entity definition**
```rust
// Look for unsupported types
#[derive(Serialize, Deserialize)]
struct User {
    id: String,
    name: String,
    balance: rust_decimal::Decimal,  // ❌ Not supported by Postcard!
}

// Or type changed
// Version 1:
// struct User { id: String, name: String }
// Version 2:
// struct User { id: String, name: String, email: String }  // Added field!
```

#### Solutions

**Solution 4A: Clear affected entries**
```bash
# Redis: Delete specific key
redis-cli DEL "user:123"

# Or delete all of a type
redis-cli KEYS "user:*" | xargs redis-cli DEL

# Memcached
echo "delete user:123" | nc localhost 11211
```

**Solution 4B: Replace Decimal with i64**
```rust
// Before: Uses Decimal
#[derive(Serialize, Deserialize)]
struct CachedProduct {
    id: String,
    price: rust_decimal::Decimal,  // ❌ Not serializable
}

// After: Use integer cents
#[derive(Serialize, Deserialize)]
struct CachedProduct {
    id: String,
    price_cents: i64,  // ✅ Serializable
}

impl CachedProduct {
    pub fn price(&self) -> f64 {
        self.price_cents as f64 / 0.9.0
    }
}
```

**Solution 4C: Use cache-specific DTO**
```rust
// Database type (with Decimal)
#[derive(sqlx::FromRow)]
struct ProductRow {
    id: String,
    price: rust_decimal::Decimal,
}

// Cache type (with i64)
#[derive(Serialize, Deserialize)]
struct CachedProduct {
    id: String,
    price_cents: i64,
}

impl From<ProductRow> for CachedProduct {
    fn from(row: ProductRow) -> Self {
        CachedProduct {
            id: row.id,
            price_cents: (row.price * 100).to_i64().unwrap_or(0),
        }
    }
}
```

---

## Logging Setup

### Configure Log Levels

```rust
// In your main.rs or lib.rs
use tracing_subscriber;

fn main() {
    // Development: DEBUG level
    #[cfg(debug_assertions)]
    {
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .init();
    }

    // Production: INFO level (less verbose)
    #[cfg(not(debug_assertions))]
    {
        tracing_subscriber::fmt()
            .with_max_level(Level::INFO)
            .init();
    }
}
```

### Environment Variable Configuration

```bash
# Enable debug logging
RUST_LOG=cache=debug cargo run

# Cache-kit only
RUST_LOG=cache_kit=trace cargo run

# Everything
RUST_LOG=debug cargo run
```

### Structured Logging

```rust
use tracing::{info, warn, error, debug};

// ✅ Good: Structured, searchable logs
info!(
    user_id = %user_id,
    cache_hit = hit,
    latency_ms = latency.as_millis(),
    "Cache operation completed"
);

// ❌ Bad: Unstructured, hard to parse
info!("Cache operation for user {} completed in {:?}ms", user_id, latency);
```

### Common Log Patterns

```
✅ Cache hit (expected):
    info!("Cache hit for user:{}", user_id);

✅ Cache miss (expected):
    debug!("Cache miss for user:{}, fetching from DB", user_id);

✅ Backend error (expected but needs handling):
    warn!("Cache backend unavailable: {}, using fallback", error);

❌ Serialization error (unexpected):
    error!("Serialization error for user:{}: {}", user_id, error);

❌ Pool exhausted (capacity issue):
    error!("Connection pool exhausted: {} active, {} waiting", active, waiting);
```

---

## Health Checks

### Backend Health Check Implementation

```rust
pub async fn check_cache_health(
    cache: &mut CacheExpander<RedisBackend>,
) -> Result<HealthStatus> {
    let start = Instant::now();

    match cache.health_check().await {
        Ok(true) => {
            let latency = start.elapsed();
            info!("Cache healthy, latency: {:?}", latency);
            
            if latency > Duration::from_millis(100) {
                warn!("Cache is slow: {:?}", latency);
                Ok(HealthStatus::Degraded)
            } else {
                Ok(HealthStatus::Healthy)
            }
        }
        Ok(false) => {
            error!("Cache health check failed");
            Ok(HealthStatus::Unhealthy)
        }
        Err(e) => {
            error!("Health check error: {}", e);
            Err(e)
        }
    }
}
```

### Periodic Health Monitoring

```rust
pub async fn monitor_cache_health(
    cache: Arc<Mutex<CacheExpander<RedisBackend>>>,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;

            let mut cache = cache.lock().await;
            match check_cache_health(&mut cache).await {
                Ok(HealthStatus::Healthy) => {
                    debug!("Cache health: OK");
                }
                Ok(HealthStatus::Degraded) => {
                    warn!("Cache health: DEGRADED (slow responses)");
                }
                Ok(HealthStatus::Unhealthy) => {
                    error!("Cache health: DOWN");
                }
                Err(e) => {
                    error!("Health check failed: {}", e);
                }
            }
        }
    });
}
```

### Expose Health Endpoint

```rust
// Axum example
async fn health_check(
    State(cache): State<Arc<Mutex<CacheExpander<RedisBackend>>>>,
) -> impl IntoResponse {
    let mut cache = cache.lock().await;

    match cache.health_check().await {
        Ok(true) => Json(serde_json::json!({
            "status": "healthy",
            "cache": "ready"
        })),
        _ => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "unhealthy",
                "cache": "unavailable"
            })),
        ),
    }
}

// In router
app.route("/health", get(health_check))
```

---

## Production Troubleshooting Checklist

Use this checklist when issues occur:

### Cache Issues

- [ ] Is Redis/Memcached running? (`redis-cli ping`)
- [ ] Is network connectivity OK? (`nc -zv localhost 6379`)
- [ ] Are connection pool metrics available?
- [ ] What's the hit rate? (< 20% = investigate TTL/keys)
- [ ] Are there serialization errors? (check entity types)
- [ ] Is memory usage growing unbounded? (check TTL)
- [ ] Are cache keys deterministic? (check key generation)

### Network Issues

- [ ] Is backend reachable? (`netstat` or `ss`)
- [ ] What's the latency? (`redis-cli --latency`)
- [ ] Are there packet drops? (`netstat -s`)
- [ ] Is there network congestion?
- [ ] Did firewall rules change?

### Application Issues

- [ ] Are error logs being generated? (`grep error app.log`)
- [ ] Is the cache fallback code working?
- [ ] Are metrics being exported?
- [ ] Is the database under load?
- [ ] Did schema change recently?

### System Issues

- [ ] CPU usage normal? (`top`)
- [ ] Memory usage normal? (`free -h`)
- [ ] Disk space available? (`df -h`)
- [ ] System load average? (`uptime`)
- [ ] Are there OOM killings? (`dmesg | tail`)

---

## Getting Help

If you can't resolve the issue:

1. **Gather diagnostics:**
   ```bash
   redis-cli INFO server > redis_info.txt
   redis-cli DBSIZE > redis_size.txt
   top -b -n 1 > system_info.txt
   grep -i cache app.log > cache_logs.txt
   ```

2. **Check recent changes:**
   - Code deploy?
   - Config change?
   - Infrastructure change?
   - Data volume increase?

3. **Enable debug logging:**
   ```bash
   RUST_LOG=cache=debug,cache_kit=trace cargo run
   ```

4. **Open an issue:** https://github.com/megamsys/cache-kit.rs/issues

---

---

## Error Handling Best Practices

### DO ✅

**1. Handle cache errors gracefully**
```rust
// Good: Don't crash on cache errors
match cache.with(&mut feeder, &repo, CacheStrategy::Refresh) {
    Ok(_) => {
        // Use cached data from feeder
        Some(feeder.user)
    }
    Err(e) => {
        // Cache failed - fallback to database
        eprintln!("Cache error: {}, using fallback", e);
        repo.fetch_by_id(&user_id).ok().flatten()
    }
}
```

**2. Log cache errors separately**
```rust
// Good: Log with context
match cache.with(&mut feeder, &repo, CacheStrategy::Refresh) {
    Ok(_) => info!("Cache operation successful"),
    Err(e) => error!("Cache error: {}", e),  // Separate from app logic
}
```

**3. Use Result types properly**
```rust
// Good: Propagate errors with context
pub fn get_user(id: String) -> Result<User> {
    // Use ? operator
    cache.with(&mut feeder, &repo, CacheStrategy::Refresh)?;
    Ok(feeder.user.ok_or(Error::NotFound)?)
}

// Bad: Swallowing errors silently
pub fn get_user(id: String) -> Option<User> {
    cache.with(&mut feeder, &repo, CacheStrategy::Refresh).ok();
    feeder.user
}
```

**4. Distinguish cache errors from application errors**
```rust
// Good: Different handling for different error types
match cache_result {
    Err(Error::BackendError(_)) => fallback_to_database(),
    Err(Error::SerializationError(_)) => invalidate_and_refetch(),
    Err(Error::TimeoutError(_)) => use_stale_data(),
    Ok(_) => use_fresh_data(),
}

// Bad: Treat all errors the same
if cache_result.is_err() {
    panic!("Cache failed!");
}
```

**5. Test error paths thoroughly**
```rust
#[test]
fn test_cache_error_fallback() {
    let backend = FailingBackend::new();  // Always fails
    let mut cache = CacheExpander::new(backend);
    
    let result = get_user_with_fallback(&mut cache, &repo, "123".to_string());
    
    assert!(result.is_ok());  // Still works despite backend failure
    assert_eq!(result.unwrap(), Some(expected_user));
}
```

### DON'T ❌

**1. Panic on cache errors**
```rust
// Bad: Crashes the service
let result = cache.with(&mut feeder, &repo, CacheStrategy::Refresh)
    .expect("Cache must work!");  // 💥 Production incident

// Good: Handle gracefully
let _ = cache.with(&mut feeder, &repo, CacheStrategy::Refresh)
    .map_err(|e| eprintln!("Cache error: {}", e));
```

**2. Expose cache errors to users**
```rust
// Bad: User sees internal error
async fn get_user(id: String) -> Result<User, String> {
    let result = cache.with(&mut feeder, &repo, CacheStrategy::Refresh)?;
    Ok(feeder.user.ok_or_else(|| "Cache error: serialization failed".to_string())?)
}

// Good: Return user-friendly error
async fn get_user(id: String) -> Result<User> {
    let result = cache.with(&mut feeder, &repo, CacheStrategy::Refresh)
        .map_err(|e| {
            error!("Cache error: {}", e);  // Log internally
            StatusCode::INTERNAL_SERVER_ERROR  // Return generic error
        })?;
    Ok(feeder.user.ok_or(Error::NotFound)?)
}
```

**3. Ignore all errors equally**
```rust
// Bad: All errors treated the same
if cache.with(&mut feeder, &repo, CacheStrategy::Refresh).is_ok() {
    return feeder.user;
}
return database_fallback();

// Good: Different handling per error
match cache.with(&mut feeder, &repo, CacheStrategy::Refresh) {
    Ok(_) => feeder.user,
    Err(Error::BackendError(_)) => database_fallback(),  // Try DB
    Err(Error::SerializationError(_)) => {  // Clear and refetch
        cache.with(&mut feeder, &repo, CacheStrategy::Invalidate).ok();
        database_fallback()
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
        None
    }
}
```

**4. Cascade failures across services**
```rust
// Bad: Cache failure blocks all dependent services
service1 depends on cache ❌
service2 depends on service1 ❌
service3 depends on service2 ❌
// If cache fails, everything fails

// Good: Each service has its own fallback
service1 has fallback to DB ✅
service2 has fallback to API ✅
service3 has fallback to queue ✅
// If any fails, others continue
```

**5. Log sensitive data in error messages**
```rust
// Bad: Logs contain user data
Err(e) => {
    error!("Failed to cache user: {:?}", user);  // 🔓 Sensitive data
}

// Good: Only log IDs
Err(e) => {
    error!("Failed to cache user {}: {}", user.id, e);  // ✅ Safe
}
```

---

## Next Steps

- Set up [Monitoring and metrics](monitoring) to detect issues early
- Check [Performance tuning](performance) for optimization

---

## See Also

- [Monitoring Guide](monitoring) — Set up metrics and alerting
- [Cache Backends](../backends) — Backend-specific configuration
