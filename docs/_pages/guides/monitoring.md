---
layout: single
title: Monitoring & Metrics
parent: Guides
---

# Monitoring & Metrics Guide

Set up production-grade monitoring, metrics, and observability for cache-kit.



---

## Overview

Monitoring cache-kit allows you to:
- **Detect problems early** — Catch issues before users notice
- **Understand performance** — Hit rates, latency, throughput
- **Optimize configuration** — Data-driven tuning decisions
- **Alert on degradation** — Automated on-call notifications
- **Troubleshoot quickly** — Historical data for diagnosis

### The 4 Golden Signals for Caching

1. **Latency** — How fast are cache operations? (p50, p99)
2. **Traffic** — How much are we using the cache? (ops/sec)
3. **Errors** — What percentage of operations fail? (error rate)
4. **Hit rate** — What percentage of requests hit cache? (cache efficiency)

---

## Metrics Implementation

### Simple Metrics Struct

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct CacheMetrics {
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    latency_total_us: Arc<AtomicU64>,
    latency_count: Arc<AtomicU64>,
}

impl CacheMetrics {
    pub fn new() -> Self {
        CacheMetrics {
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            errors: Arc::new(AtomicU64::new(0)),
            latency_total_us: Arc::new(AtomicU64::new(0)),
            latency_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record a cache hit
    pub fn record_hit(&self, latency_us: u64) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.latency_total_us.fetch_add(latency_us, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss
    pub fn record_miss(&self, latency_us: u64) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        self.latency_total_us.fetch_add(latency_us, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache error
    pub fn record_error(&self, latency_us: u64) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        self.latency_total_us.fetch_add(latency_us, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        if (hits + misses) == 0.0 {
            return 0.0;
        }
        hits / (hits + misses)
    }

    /// Get error rate (0.0 to 1.0)
    pub fn error_rate(&self) -> f64 {
        let errors = self.errors.load(Ordering::Relaxed) as f64;
        let total = self.hits.load(Ordering::Relaxed) as f64
                  + self.misses.load(Ordering::Relaxed) as f64
                  + errors;
        if total == 0.0 {
            return 0.0;
        }
        errors / total
    }

    /// Get average latency in microseconds
    pub fn avg_latency_us(&self) -> f64 {
        let count = self.latency_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        let total = self.latency_total_us.load(Ordering::Relaxed) as f64;
        total / count as f64
    }

    /// Get total operations
    pub fn total_ops(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
            + self.misses.load(Ordering::Relaxed)
            + self.errors.load(Ordering::Relaxed)
    }

    /// Get throughput (ops/sec) — pass elapsed_secs
    pub fn throughput_ops_sec(&self, elapsed_secs: f64) -> f64 {
        self.total_ops() as f64 / elapsed_secs
    }
}
```

### Instrument Your Code

```rust
pub async fn get_user_with_metrics(
    cache: &mut CacheExpander<impl CacheBackend>,
    repo: &UserRepository,
    user_id: String,
    metrics: &CacheMetrics,
) -> Result<Option<User>> {
    let start = Instant::now();
    let mut feeder = UserFeeder {
        id: user_id.clone(),
        user: None,
    };

    match cache.with(&mut feeder, repo, CacheStrategy::Refresh) {
        Ok(_) => {
            let latency_us = start.elapsed().as_micros() as u64;
            if feeder.user.is_some() {
                metrics.record_hit(latency_us);
                info!("Cache HIT for user {}", user_id);
            } else {
                metrics.record_miss(latency_us);
                info!("Cache MISS for user {}", user_id);
            }
            Ok(feeder.user)
        }
        Err(e) => {
            let latency_us = start.elapsed().as_micros() as u64;
            metrics.record_error(latency_us);
            error!("Cache ERROR for user {}: {}", user_id, e);
            Err(e)
        }
    }
}
```

---

## Prometheus Integration

### Expose Metrics Endpoint

See the [Axum example](https://github.com/megamsys/cache-kit.rs/tree/main/examples/axum) for a complete working implementation with:
- Metrics HTTP endpoint
- API server with cache instrumentation
- Prometheus scrape configuration

The metrics endpoint exposes the standard Prometheus format:
```
cache_hits_total (counter)
cache_misses_total (counter)
cache_errors_total (counter)
cache_hit_rate (gauge, 0.0-1.0)
cache_error_rate (gauge, 0.0-1.0)
cache_avg_latency_us (gauge)
```

### Prometheus Scrape Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'cache-kit'
    static_configs:
      - targets: ['localhost:3000']
    metrics_path: '/metrics'
```

### Alert Rules

{% raw %}
```yaml
# alerts.yml
groups:
   - name: cache-kit
     interval: 30s
     rules:
       # Alert if hit rate drops below 30%
       - alert: LowCacheHitRate
         expr: cache_hit_rate < 0.3
         for: 5m
         labels:
           severity: warning
         annotations:
           summary: "Low cache hit rate ({{ $value | humanizePercentage }})"
           description: "Cache hit rate below 30% for 5 minutes"

       # Alert if error rate exceeds 5%
       - alert: HighCacheErrorRate
         expr: cache_error_rate > 0.05
         for: 2m
         labels:
           severity: critical
         annotations:
           summary: "High cache error rate ({{ $value | humanizePercentage }})"
           description: "Cache errors above 5% - likely backend down"

       # Alert if average latency exceeds 100ms
       - alert: SlowCacheLatency
         expr: cache_avg_latency_us > 0.9.00
         for: 5m
         labels:
           severity: warning
         annotations:
           summary: "Slow cache latency ({{ $value | humanizeDuration }})"
           description: "Cache operations averaging > 100ms"
```
{% endraw %}

---

## Grafana Dashboard

### Dashboard JSON

```json
{
  "dashboard": {
    "title": "cache-kit Metrics",
    "panels": [
      {
        "title": "Cache Hit Rate",
        "targets": [
          {
            "expr": "cache_hit_rate"
          }
        ],
        "fieldConfig": {
          "defaults": {
            "unit": "percentunit",
            "thresholds": {
              "steps": [
                { "color": "red", "value": 0 },
                { "color": "yellow", "value": 0.3 },
                { "color": "green", "value": 0.7 }
              ]
            }
          }
        }
      },
      {
        "title": "Operations Per Second",
        "targets": [
          {
            "expr": "rate(cache_hits_total[1m]) + rate(cache_misses_total[1m])"
          }
        ]
      },
      {
        "title": "Error Rate",
        "targets": [
          {
            "expr": "cache_error_rate"
          }
        ],
        "fieldConfig": {
          "defaults": {
            "unit": "percentunit",
            "thresholds": {
              "steps": [
                { "color": "green", "value": 0 },
                { "color": "red", "value": 0.01 }
              ]
            }
          }
        }
      },
      {
        "title": "Average Latency",
        "targets": [
          {
            "expr": "cache_avg_latency_us / 1000"
          }
        ],
        "fieldConfig": {
          "defaults": {
            "unit": "ms"
          }
        }
      }
    ]
  }
}
```

---

## Key Metrics to Monitor

### Business Metrics

| Metric | Target | Alert > |
|--------|--------|---------|
| Cache Hit Rate | > 70% | < 30% |
| Error Rate | < 1% | > 5% |
| P99 Latency | < 50ms | > 100ms |
| Throughput | Baseline | Drop > 20% |

### System Metrics

| Metric | Target | Alert > |
|--------|--------|---------|
| Connection Pool Active | < max | == max (5 min) |
| Memory Usage | Stable | +50% from baseline |
| Cache Size | Bounded | Growing unbounded |
| Evictions | None | > 0/min |

---

## On-Call Runbook

### If Hit Rate Drops Below 30%

**Severity:** Warning  
**Time to investigate:** 15 minutes

1. **Check if this is normal:**
   ```sql
   SELECT avg(hit_rate) FROM metrics WHERE time > now() - interval '1 hour'
   -- Is this actually a change?
   ```

2. **Check for recent changes:**
   - Any code deployments?
   - Any data migrations?
   - Any schema changes?

3. **Investigate root cause:**
   - Hit rate low for specific entity types? (e.g., only users, not products)
   - Did user traffic pattern change? (new traffic type not being cached)
   - Is TTL too short? (`redis-cli TTL "sample-key"`)
   - Are cache keys non-deterministic?

4. **Remediate:**
   - If TTL too short: Increase TTL
   - If key generation wrong: Fix code and redeploy
   - If traffic pattern changed: Update caching strategy

### If Error Rate Exceeds 5%

**Severity:** Critical  
**Time to respond:** < 5 minutes

1. **Check if backend is up:**
   ```bash
   redis-cli ping
   # Response: PONG (good)
   # Response: error (bad - Redis down)
   ```

2. **Check network connectivity:**
   ```bash
   nc -zv localhost 6379
   redis-cli --latency
   ```

3. **Check connection pool:**
   ```rust
   error!("Active: {}, Waiting: {}", active_conns, waiting_reqs);
   ```

4. **Actions:**
   - If backend down: Restart it
   - If pool exhausted: Increase pool size
   - If network issue: Check firewall/connectivity
   - If errors persisting: Failover to backup cache

### If Latency Exceeds 100ms

**Severity:** Warning  
**Time to investigate:** 10 minutes

1. **Check if this is sustained:**
   ```
   Is p99 consistently > 100ms for > 5 min?
   Or is this a temporary spike?
   ```

2. **Check network latency:**
   ```bash
   redis-cli --latency
   # < 1ms (good)
   # > 10ms (network issue)
   ```

3. **Check backend load:**
   ```bash
   redis-cli INFO stats
   # instantaneous_ops_per_sec: ?
   # Is Redis CPU bound?
   ```

4. **Actions:**
   - Increase connection pool size
   - Check for slow queries on database
   - If sustained, scale up Redis
   - Review timeout configuration

---

## Monitoring Best Practices

### DO ✅

1. **Monitor 4 golden signals**
   ```rust
   metrics.record_hit(latency);      // Latency + Traffic
   metrics.record_miss(latency);     // Latency + Traffic
   metrics.record_error(latency);    // Errors + Latency
   ```

2. **Alert on actionable metrics**
   - Hit rate < 30% → Investigate caching strategy
   - Error rate > 5% → Backend issue
   - Latency p99 > 100ms → Performance issue

3. **Baseline your metrics**
   ```
   Healthy state: 80% hit rate, < 1% errors, < 50ms p99
   Use this for anomaly detection
   ```

4. **Include context in metrics**
   ```rust
   // Label by entity type
   cache_hits{entity_type="user"} 1000
   cache_hits{entity_type="product"} 500
   
   // Find which type has low hit rate
   ```

### DON'T ❌

1. **Alert on every metric change**
   ```rust
   // Bad: Too many alerts
   if hit_rate != yesterday_hit_rate {
       alert!();
   }

   // Good: Alert on significant change
   if hit_rate < baseline * 0.7 {  // 30% drop
       alert!();
   }
   ```

2. **Ignore "normal" errors**
   ```rust
   // These are expected, don't alert:
   - Cache miss (expected)
   - Serialization error on schema change (expected after deploy)
   - Timeout during network partition (expected)
   ```

3. **Set thresholds without baselines**
   ```rust
   // Don't guess: 30% error rate good or bad?
   // Establish baseline first
   let baseline_error_rate = 0.01;  // 1%
   let threshold = baseline_error_rate * 5;  // Alert at 5x normal
   ```

4. **Forget about cardinality**
   ```rust
   // Bad: Unbounded labels (infinite cardinality)
   cache_hits{user_id="123"} 1
   cache_hits{user_id="456"} 1
   // Cardinality explodes!

   // Good: Fixed dimensions
   cache_hits{entity_type="user"} 2
   ```

---

## Troubleshooting Metrics

### "All metrics show zeros"

**Cause:** Metrics not being recorded  
**Solution:**
```rust
// Verify code is calling record_hit/record_miss
match cache.with(&mut feeder, &repo, CacheStrategy::Refresh) {
    Ok(_) => metrics.record_hit(latency),  // Add this
    Err(e) => metrics.record_error(latency),  // Add this
}
```

### "Hit rate doesn't match expected"

**Cause:** Metrics counting different things  
**Solution:**
```rust
// Verify what counts as hit vs miss
// Cache hit: Entry exists in cache, returned
metrics.record_hit(latency);

// Cache miss: Entry not in cache, fetched from DB
metrics.record_miss(latency);

// Error: Cache operation failed
metrics.record_error(latency);
```

### "Prometheus shows NaN for hit_rate"

**Cause:** Division by zero (no operations yet)  
**Solution:**
```rust
pub fn hit_rate(&self) -> f64 {
    let total = self.hits.load(Ordering::Relaxed) 
              + self.misses.load(Ordering::Relaxed);
    if total == 0 {
        return 0.0;  // Return 0, not NaN
    }
    // ... calculation
}
```

---

## Monitoring Checklist

Before production:

- [ ] Metrics struct implemented
- [ ] All cache operations instrumented (hit/miss/error)
- [ ] Latency measurement accurate
- [ ] Prometheus scrape endpoint working
- [ ] Prometheus config pointing to correct endpoint
- [ ] Alert rules configured
- [ ] Grafana dashboard created
- [ ] Baseline metrics established
- [ ] On-call runbook written
- [ ] Team trained on runbook

---

## Next Steps

- Read [Error Handling guide](error-handling) for handling metric anomalies
- Check [Troubleshooting guide](troubleshooting) for diagnosis patterns
- Review [Backends guide](../backends) for backend-specific metrics

---

## See Also

- [Troubleshooting Guide](troubleshooting) — Using metrics to diagnose issues
- [Error Handling](error-handling) — Understanding error metrics
- [Performance Guide](performance) — Benchmarking vs. production metrics



Here are shortened versions cutting marketing jargon and focusing on actionable content:

docs/guides/errir-handling.md (Target: 1,200 words)

Keep:

Error categories (1-4)
Recovery patterns (1-2 essential ones)
Testing approaches
DO/DON'T checklist

Remove:

Verbose "Overview" marketing speak
Redundant error flow diagrams
Duplicate examples across patterns

---

docs/guides/troubleshooting.md (Target: 1,100 words)

Keep:

4 common issues (concise root causes + solutions)