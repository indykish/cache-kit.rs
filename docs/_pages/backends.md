---
layout: single
title: Cache Backend Support
description: "Understanding cache backends and choosing the right one for your use case"
permalink: /backends/
---




---

## Backend Tiers

cache-kit supports multiple cache backends with explicit tiering based on production readiness and use case.

| Backend | Tier | Use Case | Persistence | Distribution |
|---------|------|----------|-------------|--------------|
| **Redis** | Tier-0 (Production) | High-performance distributed cache | Optional | ✅ Yes |
| **Memcached** | Tier-0 (Production) | Ultra-fast memory cache | ❌ No | ✅ Yes |
| **InMemory** | Tier-1 (Dev/Test) | Local development, testing | ❌ No | ❌ No |

---

## Tier-0: Production-Grade Backends

### Redis

Redis is a high-performance, feature-rich in-memory database with optional persistence.

#### Why Choose Redis?

- ✅ **Persistence** — Data survives restarts (optional)
- ✅ **Rich data structures** — Beyond key-value
- ✅ **Pub/Sub** — Event notifications
- ✅ **Clustering** — Horizontal scaling
- ✅ **Managed services** — AWS ElastiCache, DigitalOcean, etc.

#### Installation

```toml
[dependencies]
cache-kit = { version = "0.9", features = ["redis"] }
```

#### Configuration

```rust
use cache_kit::backend::{RedisBackend, RedisConfig};
use cache_kit::CacheExpander;
use std::time::Duration;

let config = RedisConfig {
    host: "localhost".to_string(),
    port: 6379,
    pool_size: 10,
    connection_timeout: Duration::from_secs(5),
    username: None,
    password: None,
    database: 0,
};

let backend = RedisBackend::new(config)?;
let expander = CacheExpander::new(backend);
```

#### Redis Configuration Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `host` | `String` | `"localhost"` | Redis server hostname or IP |
| `port` | `u16` | `6379` | Redis server port |
| `username` | `Option<String>` | `None` | Redis username (Redis 6+) |
| `password` | `Option<String>` | `None` | Redis password |
| `database` | `u32` | `0` | Redis database number (0-15) |
| `pool_size` | `u32` | `16` | Connection pool size |
| `connection_timeout` | `Duration` | `5s` | Connection timeout |

#### Configuration Examples

```rust
use std::time::Duration;

// Basic configuration
let config = RedisConfig {
    host: "localhost".to_string(),
    port: 6379,
    ..Default::default()
};

// With authentication
let config = RedisConfig {
    host: "example.com".to_string(),
    port: 6379,
    password: Some("secret".to_string()),
    database: 1,
    ..Default::default()
};

// With custom pool size
let config = RedisConfig {
    host: "localhost".to_string(),
    port: 6379,
    pool_size: 32,
    connection_timeout: Duration::from_secs(10),
    ..Default::default()
};
```

#### Managed Redis Services

cache-kit works with Redis-compatible managed services:

- **AWS ElastiCache for Redis**
  ```rust
  let config = RedisConfig {
      host: "my-cluster.cache.amazonaws.com".to_string(),
      port: 6379,
      pool_size: 20,
      ..Default::default()
  };
  ```

- **DigitalOcean Managed Redis**
  ```rust
  let config = RedisConfig {
      host: "my-redis-cluster".to_string(),
      port: 25061,
      pool_size: 15,
      // Note: For TLS, you may need additional configuration
      ..Default::default()
  };
  ```

- **Redis Cloud**
  ```rust
  let config = RedisConfig {
      host: "redis-12345.cloud.redislabs.com".to_string(),
      port: 12345,
      password: Some("your-password".to_string()),
      ..Default::default()
  };
  ```

#### Redis Best Practices

✅ **DO:**
- Use connection pooling (`pool_size` >= expected concurrent requests)
- Enable persistence for production (AOF or RDB)
- Set appropriate `maxmemory` and eviction policies
- Monitor memory usage and hit rates
- Use TLS for network traffic

❌ **DON'T:**
- Use a single connection for high concurrency
- Ignore Redis memory limits
- Store unbounded data without TTLs
- Skip authentication in production

---

### Memcached

Memcached is an ultra-fast, distributed memory object caching system.

#### Why Choose Memcached?

- ✅ **Extremely fast** — Optimized for speed
- ✅ **Distributed** — Multi-server deployment
- ✅ **Simple** — Minimal configuration
- ✅ **Mature** — Battle-tested in production

⚠️ **Caveats:**
- ❌ **No persistence** — Data lost on restart
- ❌ **No wildcard deletes** — Cannot delete by pattern
- ❌ **No pub/sub** — No event notifications

#### Installation

```toml
[dependencies]
cache-kit = { version = "0.9", features = ["memcached"] }
```

#### Configuration

```rust
use cache_kit::backend::{MemcachedBackend, MemcachedConfig};
use cache_kit::CacheExpander;

let config = MemcachedConfig {
    servers: vec!["localhost:11211".to_string()],
    max_connections: 10,
    min_connections: 2,
};

let backend = MemcachedBackend::new(config)?;
let expander = CacheExpander::new(backend);
```

#### Memcached Configuration Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `servers` | `Vec<String>` | Required | List of Memcached server addresses |
| `max_connections` | `usize` | `10` | Maximum connections per server |
| `min_connections` | `usize` | `2` | Minimum idle connections per server |

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

**Key distribution:** Keys are automatically distributed across servers using consistent hashing.

#### Memcached Best Practices

✅ **DO:**
- Use multiple servers for redundancy
- Set appropriate TTLs (no persistence)
- Monitor memory usage per server
- Plan for cache misses (no persistence)

❌ **DON'T:**
- Rely on wildcard delete operations (not supported)
- Expect data to survive restarts
- Use for long-term storage
- Ignore server failures (no automatic failover)

---

## Tier-1: Development & Testing

### InMemory Backend

The InMemory backend uses an in-process concurrent HashMap (DashMap).

#### Why Choose InMemory?

- ✅ **Zero dependencies** — No external services needed
- ✅ **Fast setup** — Perfect for local development
- ✅ **Deterministic** — Same process, predictable behavior
- ✅ **Thread-safe** — Lock-free concurrent access

⚠️ **Limitations:**
- ❌ **Single instance** — Not distributed
- ❌ **Memory-only** — Data lost on process restart
- ❌ **Not scalable** — Limited to single machine

#### Installation

InMemory backend is included by default:

```toml
[dependencies]
cache-kit = "0.9"
```

#### Configuration

```rust
use cache_kit::backend::InMemoryBackend;
use cache_kit::CacheExpander;

let backend = InMemoryBackend::new();
let expander = CacheExpander::new(backend);
```

No configuration needed! Perfect for:
- Unit tests
- Integration tests
- Local development
- Proof-of-concept projects

#### InMemory Best Practices

✅ **DO:**
- Use for all unit tests
- Use for local development
- Create fresh instances per test
- Clear cache between tests if needed

❌ **DON'T:**
- Use in production
- Share instances across tests (isolation)
- Expect data to survive process restarts
- Use for distributed services

---

## Backend Comparison

| Feature | Redis | Memcached | InMemory |
|---------|-------|-----------|----------|
| **Performance** | ⚡⚡ Very Fast | ⚡⚡⚡ Ultra Fast | ⚡⚡⚡ Ultra Fast |
| **Persistence** | ✅ Optional | ❌ No | ❌ No |
| **Distribution** | ✅ Clustering | ✅ Multi-server | ❌ Single process |
| **Complexity** | Medium | Low | Very Low |
| **Setup Time** | Minutes | Minutes | Seconds |
| **Production Ready** | ✅ Yes | ✅ Yes | ❌ No |
| **Data Structures** | ✅ Rich | ❌ Key-Value only | ❌ Key-Value only |
| **Memory Management** | ✅ Eviction policies | ✅ LRU | ⚠️ Manual |
| **Pub/Sub** | ✅ Yes | ❌ No | ❌ No |
| **Transactions** | ✅ Yes | ❌ No | ❌ No |

---

## Choosing the Right Backend

### Decision Tree

```
Are you in production?
├─ Yes → Need persistence?
│   ├─ Yes → Redis
│   └─ No → Need extreme speed?
│       ├─ Yes → Memcached
│       └─ No → Redis
└─ No → Local development / testing?
    └─ Yes → InMemory
```

### Use Case Recommendations

| Use Case | Recommended Backend | Rationale |
|----------|-------------------|-----------|
| **Production web app** | Redis | Persistence, rich features, managed services |
| **High-traffic API** | Memcached | Ultra-fast, distributed |
| **Session storage** | Redis | Persistence, expiry, pub/sub |
| **Read-heavy workload** | Memcached | Optimized for reads |
| **Local development** | InMemory | Zero setup, fast iterations |
| **Unit tests** | InMemory | Deterministic, isolated |
| **Multi-region deployment** | Redis | Replication, clustering |

---

## Switching Backends

Switching backends requires **no code changes** in your application logic:

```rust
// Development (InMemory)
#[cfg(debug_assertions)]
let backend = InMemoryBackend::new();

// Production (Redis)
#[cfg(not(debug_assertions))]
let backend = RedisBackend::new(RedisConfig {
    host: std::env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
    port: std::env::var("REDIS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(6379),
    ..Default::default()
})?;

// Same expander interface
let expander = CacheExpander::new(backend);
```

Or use environment variables:

```rust
fn create_backend() -> Box<dyn cache_kit::backend::CacheBackend> {
    match std::env::var("CACHE_BACKEND").as_deref() {
        Ok("redis") => {
            let host = std::env::var("REDIS_HOST")
                .unwrap_or_else(|_| "localhost".to_string());
            let port = std::env::var("REDIS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(6379);
            Box::new(RedisBackend::new(RedisConfig {
                host,
                port,
                ..Default::default()
            }).expect("Failed to connect to Redis"))
        }
        Ok("memcached") => {
            let servers = std::env::var("MEMCACHED_SERVERS")
                .expect("MEMCACHED_SERVERS required")
                .split(',')
                .map(String::from)
                .collect();
            Box::new(MemcachedBackend::new(MemcachedConfig {
                servers,
                ..Default::default()
            }).expect("Failed to connect to Memcached"))
        }
        _ => Box::new(InMemoryBackend::new()),
    }
}
```

---

## Connection Pooling

Redis and Memcached backends use connection pooling for optimal performance.

### Pool Configuration

```rust
use std::time::Duration;

let config = RedisConfig {
    host: "localhost".to_string(),
    port: 6379,
    pool_size: 16,    // Optimized default (8 cores × 2 + 1 ≈ 16)
    connection_timeout: Duration::from_secs(10),
    username: None,
    password: None,
    database: 0,
};
```

### Sizing Guidelines

**Recommended formula:** `(CPU cores × 2) + 1`

| System | Formula | Recommended Pool Size |
|--------|---------|----------------------|
| **4-core system** | (4 × 2) + 1 = 9 | 8-10 |
| **8-core system** | (8 × 2) + 1 = 17 | **16** (default) |
| **16-core system** | (16 × 2) + 1 = 33 | 32 |
| **32-core system** | (32 × 2) + 1 = 65 | 64 |

**Research findings:** Pool size of 16 provides 49-53% latency reduction compared to pool size of 10 on 8-core systems, with 2.75x reduction in contention outliers (22% → 8%). See [Performance Guide](guides/performance) for detailed benchmarks.

**Default:** cache-kit uses `max_connections: 16` and `min_connections: 4` as optimized defaults for typical 8-core systems.

---

## Docker Compose Setup

For local development:

```yaml
version: '3.8'

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
    command: memcached -m 64  # 64MB memory limit

volumes:
  redis_data:
```

Start services:

```bash
docker-compose up -d
```

Test connectivity:

```bash
# Redis
redis-cli ping  # Should return: PONG

# Memcached
echo "stats" | nc localhost 11211
```

---

## Monitoring and Observability

### Redis Monitoring

```bash
# Connection count
redis-cli CLIENT LIST | wc -l

# Memory usage
redis-cli INFO memory | grep used_memory_human

# Hit rate
redis-cli INFO stats | grep keyspace
```

### Memcached Monitoring

```bash
# Stats
echo "stats" | nc localhost 11211

# Hit rate
echo "stats" | nc localhost 11211 | grep -E "cmd_get|get_hits|get_misses"
```

### Application Metrics

Implement cache metrics in your application:

```rust
struct CacheMetrics {
    hits: AtomicU64,
    misses: AtomicU64,
    errors: AtomicU64,
}

impl CacheMetrics {
    fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        if hits + misses == 0 {
            return 0.0;
        }
        hits as f64 / (hits + misses) as f64
    }
}
```

---

## Production Checklist

Before deploying to production:

### Redis

- [ ] Enable persistence (AOF or RDB)
- [ ] Set `maxmemory` policy
- [ ] Enable authentication (`requirepass`)
- [ ] Use TLS for network traffic
- [ ] Configure backup strategy
- [ ] Monitor memory usage
- [ ] Set up replication or clustering

### Memcached

- [ ] Deploy multiple servers
- [ ] Configure memory limits
- [ ] Monitor server health
- [ ] Plan for cache misses (no persistence)
- [ ] Set up monitoring alerts

### InMemory

- [ ] ⚠️ **Not recommended for production**
- [ ] Use only for proof-of-concept or single-instance services

---

## Next Steps

- Review [Design principles](design-principles)
- Explore the [Actix + SQLx reference implementation](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx)
- Read about [Serialization formats](serialization)
- Check the [Installation guide](installation)
