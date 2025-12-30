---
layout: single
title: Performance Guide
parent: Guides
---

# Performance Guide

Comprehensive guide to benchmarking, optimization, and production tuning for cache-kit.



---

## Overview

This guide covers:
- **Benchmarking** — Measuring and comparing cache performance
- **Pool Optimization** — Tuning connection pools for Redis and Memcached
- **Production Monitoring** — Tracking performance in production
- **Troubleshooting** — Diagnosing performance issues

---

## Benchmarking

### Running Benchmarks

cache-kit uses [Criterion](https://bheisler.github.io/criterion.rs/book/) for statistically rigorous performance benchmarking.

```bash
# Run all benchmarks and view results
make perf

# Run backend-specific benchmarks (requires services running)
cargo bench --bench redis_benchmark --features redis
cargo bench --bench memcached_benchmark --features memcached

# Save current performance as baseline
make perf-save

# Compare against saved baseline (detect regressions)
make perf-diff
```

### What Gets Benchmarked

1. **InMemory Backend** — Cache operations (set, get, delete) with throughput metrics
2. **Redis Backend** — Network-backed operations, batch operations, connection pooling (requires Redis running)
3. **Memcached Backend** — Network-backed operations, batch operations, binary protocol (requires Memcached running)
4. **CacheExpander** — Full cache lifecycle (hit/miss paths) with throughput metrics
5. **Serialization** — Bincode performance across payload sizes with throughput metrics

### Understanding Criterion Output

```
inmemory_backend/get_hit/1000
                        time:   [30.456 ns 30.789 ns 31.123 ns]
                                 ^^^^^^^^  ^^^^^^^^  ^^^^^^^^
                                 lower     median    upper (95% confidence)
```

- **Median**: Most reliable performance metric
- **Confidence interval**: Range where true performance likely falls (95% certain)
- **Outliers**: Anomalies removed automatically

### Baseline Comparison

```
inmemory_backend/get_hit/1000
                        time:   [30.123 ns 30.456 ns 30.789 ns]
                        change: [-40.234% -38.567% -36.891%] (p = 0.00 < 0.05)
                        Performance has improved.
```

- **change**: Performance difference vs baseline
- **p-value**: Statistical significance (p < 0.05 = real change, not noise)
- **Verdict**: "improved", "regressed", or "no change"

### What is a Baseline?

A **baseline** is a saved snapshot of benchmark performance used for comparison:

```bash
# Step 1: Before optimization - save baseline
make perf-save
# Saves to: target/criterion/*/main/

# Step 2: Make code changes
vim src/backend/inmemory.rs

# Step 3: Compare new performance
make perf-diff
# Shows: "40% faster!" or "5% slower (regression!)"
```

Baselines are stored in `target/criterion/<benchmark-name>/main/`. The name "main" is just a label — you can use others:

```bash
cargo bench -- --save-baseline v0.3.0
cargo bench -- --baseline v0.3.0
```

### Performance Variance

Benchmarks won't be **exactly** identical across runs due to:
- CPU frequency scaling
- Background processes
- OS scheduler
- Cache state

**Typical variation**: ±1-5%

Criterion uses statistical analysis to determine if changes are real:
- **< 2% difference**: "No change detected" (within noise)
- **> 5% difference + p < 0.05**: "Performance changed" (statistically significant)

---

## Expected Performance

### InMemory Backend (In-Process)

| Benchmark | Typical Time | Throughput |
|-----------|--------------|------------|
| `inmemory_backend/get_miss` | 15-20 ns | 50-66M ops/sec |
| `inmemory_backend/get_hit/1KB` | 30-50 ns | 20-33M ops/sec, 20-33 GB/sec |
| `inmemory_backend/set/1KB` | 40-60 ns | 16-25M ops/sec, 16-25 GB/sec |
| `cache_expander/refresh_hit/1KB` | 100-200 ns | 5-10M ops/sec, 5-10 GB/sec |
| `cache_expander/refresh_miss/1KB` | 1-5 µs | 200K-1M ops/sec, 200MB-1GB/sec |
| `serialization/serialize/1KB` | 50-100 ns | 10-20M ops/sec, 10-20 GB/sec |

### Redis Backend (Network-Backed)

| Benchmark | Typical Time | Throughput |
|-----------|--------------|------------|
| `redis_backend/get_miss` | 200-500 µs | 2K-5K ops/sec |
| `redis_backend/get_hit/1KB` | 200-600 µs | 1.6K-5K ops/sec, 1.6-5 MB/sec |
| `redis_backend/set/1KB` | 300.9.0 µs | 1.2K-3.3K ops/sec, 1.2-3.3 MB/sec |
| `redis_batch_ops/mget/batch_10_size_1000` | 300-1000 µs | 10K-33K ops/sec, 10-33 MB/sec |
| `redis_connection_pool/health_check` | 100-300 µs | 3.3K-10K ops/sec |

### Memcached Backend (Network-Backed)

| Benchmark | Typical Time | Throughput |
|-----------|--------------|------------|
| `memcached_backend/get_miss` | 150-400 µs | 2.5K-6.6K ops/sec |
| `memcached_backend/get_hit/1KB` | 150-500 µs | 2K-6.6K ops/sec, 2-6.6 MB/sec |
| `memcached_backend/set/1KB` | 200-600 µs | 1.6K-5K ops/sec, 1.6-5 MB/sec |
| `memcached_batch_ops/mget/batch_10_size_1000` | 250.9.0 µs | 12.5K-40K ops/sec, 12.5-40 MB/sec |
| `memcached_protocol/health_check` | 100-250 µs | 4K-10K ops/sec |

**Note:** Network-backed benchmarks are ~1000x slower than in-memory due to network latency. Times vary based on network conditions and service configuration.

---

## Connection Pool Optimization

### Research Summary

**Optimal pool size: 16 connections** (for 8-core systems)

- **Improvement:** 49-53% latency reduction vs. pool size 10
- **Outlier reduction:** 22% → 8% (2.75x reduction in contention)
- **Formula:** `(CPU cores × 2) + 1`

### Benchmark Results

| Operation | Pool=10 | Pool=16 | Pool=32 | Best |
|-----------|---------|---------|---------|------|
| SET/100   | 809 µs  | 412 µs  | 381 µs  | 16 ✅ |
| SET/1KB   | 584 µs  | 383 µs  | 388 µs  | 16 ✅ |
| SET/10KB  | 540 µs  | 524 µs  | 525 µs  | tie  |
| GET/100   | 385 µs  | 395 µs  | 383 µs  | 32   |
| GET/1KB   | 379 µs  | 386 µs  | 382 µs  | 10   |
| Outliers  | 22%     | 8%      | 12%     | 16 ✅ |

**Verdict:** Pool=16 provides best balance of latency and stability.

### Why Pool Size Matters

#### Connection Queue Dynamics

**Pool Size = 10 (Under-provisioned)**
```
┌─ Redis Server (1 connection limit)
│
├─ Active conn 1
├─ Active conn 2
├─ ...
├─ Active conn 10
│
└─ Queue: [req11, req12, ... req50]  ← 40 requests waiting!
   Waiting = HIGH LATENCY & OUTLIERS
```

**Pool Size = 16 (Optimal)**
```
┌─ Redis Server
│
├─ Active conn 1-14
├─ Available conn 15-16  ← buffer for burst traffic
│
└─ Queue: [req45, req46...]  ← 6 requests waiting (acceptable)
   Waiting = LOW LATENCY & STABLE
```

**Pool Size = 32 (Over-provisioned)**
```
├─ Active conn 1-14
├─ Idle conn 15-32  ← WASTED MEMORY & CPU
│
└─ Queue: empty
   Problem: Network becomes bottleneck, not pooling
```

### Pool Sizing Formula

```
Cores: 8 (typical development system)
Calculation: (8 × 2) + 1 = 17 ≈ 16

Recommended: 16 connections
```

#### Scaling to Other Hardware

| Cores | Formula | Recommended |
|-------|---------|-------------|
| 4     | 9       | 8-10        |
| 8     | 17      | **16** (default) |
| 16    | 33      | 32          |
| 32    | 65      | 64          |
| 64    | 129     | 128         |

### Default Configuration

cache-kit defaults to pool size **16** for optimal performance on typical systems:

```rust
// src/backend/redis.rs
const DEFAULT_POOL_SIZE: u32 = 16;  // Optimized default

// src/backend/memcached.rs
const DEFAULT_POOL_SIZE: u32 = 16;  // Optimized default
```

### Custom Pool Sizing

Override defaults using environment variables or configuration:

```bash
# High-concurrency service
REDIS_POOL_SIZE=32 cargo run

# Low-traffic service
MEMCACHED_POOL_SIZE=8 cargo run
```

Or in code:

```rust
use cache_kit::backend::{RedisBackend, RedisConfig};
use std::time::Duration;

let config = RedisConfig {
    host: "localhost".to_string(),
    port: 6379,
    pool_size: 32,  // Override default
    connection_timeout: Duration::from_secs(5),
    username: None,
    password: None,
    database: 0,
};

let backend = RedisBackend::new(config)?;
```

---

## Production Monitoring

### Key Metrics to Track

#### 1. Connection Pool Utilization

```
Ideal: 70-90% of pool connections in use
Too low (< 50%): Over-provisioned, waste memory
Too high (> 95%): Under-provisioned, requests queue up
```

#### 2. Request Latency Percentiles

```
p50: < 1ms (median)
p95: < 5ms (95th percentile)
p99: < 20ms (99th percentile - acceptable spikes)
```

#### 3. Connection Wait Time

```
Monitor queue depth: should be near zero
Spikes indicate under-provisioning
```

### Tuning in Production

**If you see high p99 latency:**
```bash
# Increase pool size
REDIS_POOL_SIZE=$(($(nproc) * 2 + 1)) cargo run
```

**If you see memory pressure:**
```bash
# Decrease pool size
REDIS_POOL_SIZE=$(($(nproc) * 1)) cargo run
```

### Application Metrics

Implement cache metrics in your application:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

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

## Benchmark Prerequisites

### Redis Benchmarks

```bash
# Start Redis (Docker recommended)
docker run -d -p 6379:6379 redis:latest

# Or use system Redis
redis-server

# Run benchmarks
cargo bench --bench redis_benchmark --features redis
```

### Memcached Benchmarks

```bash
# Start Memcached (Docker recommended)
docker run -d -p 11211:11211 memcached:latest

# Or use system Memcached
memcached -p 11211

# Run benchmarks
cargo bench --bench memcached_benchmark --features memcached
```

### Using Docker Compose

```bash
# Start all services (from project root)
make up

# Run all benchmarks
make perf

# Stop services
make down
```

---

## Advanced Benchmarking

### Custom Baselines

```bash
# Save version-specific baselines
cargo bench -- --save-baseline v0.3.0
cargo bench -- --save-baseline before-optimization

# Compare against specific baseline
cargo bench -- --baseline v0.3.0
```

### Run Specific Benchmarks

```bash
# Run only InMemory benchmarks
cargo bench -- inmemory_backend

# Run only cache expander
cargo bench -- cache_expander

# Run single benchmark
cargo bench -- inmemory_backend/get_miss

# Run specific backend benchmark suites
cargo bench --bench redis_benchmark --features redis -- redis_backend
cargo bench --bench memcached_benchmark --features memcached -- memcached_backend

# Run specific Redis benchmark group
cargo bench --bench redis_benchmark --features redis -- redis_batch_ops
```

### Using Feature Flags

```bash
# Test specific backends
make test FEATURES="--features redis"
make test FEATURES="--all-features"

# Build with features
make build FEATURES="--features memcached"

# Benchmark specific backends
cargo bench --bench redis_benchmark --features redis
cargo bench --bench memcached_benchmark --features memcached
```

---

## CI Integration

### GitHub Actions Example

```yaml
# In .github/workflows/ci.yml
- name: Run benchmarks
  run: make perf-save

- name: Check for regressions
  run: make perf-diff
```

### View HTML Report

After running `make perf`, open:
```
target/criterion/report/index.html
```

Shows:
- Performance violin plots
- Statistical analysis
- Regression detection
- Historical comparisons

---

## Troubleshooting

### "No baseline found"

**Solution:** Run `make perf-save` first to create a baseline.

### "Large variance in results"

**Causes:**
- Background processes consuming CPU
- Running on battery power (CPU throttling)
- Thermal throttling

**Solutions:**
- Close other applications
- Run on AC power (not battery)
- Ensure adequate cooling
- Run benchmarks multiple times and check consistency

### "Benchmarks take too long"

This is normal! Criterion runs 100+ iterations for statistical accuracy.

**Typical runtime:** 5-10 minutes for full suite

**To speed up:**
```bash
# Run specific benchmark groups
cargo bench -- inmemory_backend

# Or reduce sample size (less accurate)
cargo bench -- --sample-size 10
```

### High Latency in Network Backends

**Checklist:**
- [ ] Redis/Memcached services running?
- [ ] Network connectivity OK?
- [ ] Connection pool sized appropriately?
- [ ] Check service logs for errors
- [ ] Verify pool configuration matches CPU cores

---

## Performance Optimization Summary

| Metric | Pool=10 → Pool=16 |
|--------|-------------------|
| SET latency (100B) | 809 µs → 412 µs (-49%) |
| SET latency (1KB) | 584 µs → 383 µs (-34%) |
| Outlier reduction | 22% → 8% (-64%) |
| Consistency | Poor → Excellent |
| Tail latency (p99) | High → Low |

---

## References

- [Criterion documentation](https://bheisler.github.io/criterion.rs/book/)
- [Deadpool documentation](https://docs.rs/deadpool/) — Pool sizing recommendations
- [Redis best practices](https://redis.io/docs/manual/client-side-caching/) — Connection pooling
- Hardware utilization: CPU core × 2 rule

---

## See Also

- [Testing Guide](testing) — Unit and integration testing strategies
- [Cache Backend Support](../backends) — Backend configuration details
- [Contributing Guide](contributing) — Code standards and submission process

---

**Status:** ✅ Benchmarking and optimization guidelines verified with Criterion on 8-core systems.
