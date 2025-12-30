---
layout: single
title: Examples & Patterns
parent: Guides
---


This document covers advanced cache framework patterns and usage scenarios.

---

## Table of Contents

1. [Composite Keys](#composite-keys)
2. [Builder Pattern](#builder-pattern)
3. [Registry Pattern](#registry-pattern)
4. [TTL Strategies](#ttl-strategies)
5. [Error Handling](#error-handling)
6. [Batch Operations](#batch-operations)
7. [Multi-Tier Caching](#multi-tier-caching)
8. [Custom Serialization](#custom-serialization)
9. [Async Patterns](#async-patterns)

---

## Composite Keys

Use composite keys for entities that depend on multiple parameters.

### Example: Employment by Person and Date

```rust
use cache_kit::CacheEntity;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Employment {
    pub person_id: String,
    pub year: i32,
    pub employer: String,
    pub salary: f64,
}

// Define a composite key type
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct EmploymentKey {
    person_id: String,
    year: i32,
}

impl Display for EmploymentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.person_id, self.year)
    }
}

impl CacheEntity for Employment {
    type Key = EmploymentKey;

    fn cache_key(&self) -> Self::Key {
        EmploymentKey {
            person_id: self.person_id.clone(),
            year: self.year,
        }
    }

    fn cache_prefix() -> &'static str {
        "employment"
    }
}

// Usage
struct EmploymentFeeder {
    person_id: String,
    year: i32,
    pub employment: Option<Employment>,
}

impl CacheFeed<Employment> for EmploymentFeeder {
    fn entity_id(&mut self) -> String {
        // Both person_id and year become the entity_id
        format!("{}:{}", self.person_id, self.year)
    }

    fn feed(&mut self, entity: Option<Employment>) {
        self.employment = entity;
    }
}

// Fetch employment for a specific person and year
fn fetch_employment(
    expander: &mut CacheExpander<InMemoryBackend>,
    person_id: String,
    year: i32,
) {
    let mut feeder = EmploymentFeeder {
        person_id,
        year,
        employment: None,
    };

    let repository = EmploymentRepository;
    expander.with(&mut feeder, &repository, CacheStrategy::Refresh).ok();

    if let Some(emp) = feeder.employment {
        println!("Employment: {:?}", emp);
    }
}
```

---

## Builder Pattern

Use a builder for complex cache configurations.

```rust
use cache_kit::{CacheExpander, observability::TtlPolicy};
use std::time::Duration;

pub struct CacheBuilder<B: CacheBackend> {
    expander: CacheExpander<B>,
    ttl_policy: Option<TtlPolicy>,
    metrics: Option<Box<dyn CacheMetrics>>,
}

impl<B: CacheBackend> CacheBuilder<B> {
    pub fn new(backend: B) -> Self {
        CacheBuilder {
            expander: CacheExpander::new(backend),
            ttl_policy: None,
            metrics: None,
        }
    }

    pub fn with_ttl(mut self, policy: TtlPolicy) -> Self {
        self.ttl_policy = Some(policy);
        self
    }

    pub fn with_metrics(mut self, metrics: Box<dyn CacheMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub fn build(mut self) -> CacheExpander<B> {
        if let Some(metrics) = self.metrics {
            self.expander = self.expander.with_metrics(metrics);
        }
        if let Some(ttl) = self.ttl_policy {
            self.expander = self.expander.with_ttl_policy(ttl);
        }
        self.expander
    }
}

// Usage
fn setup_cache() -> CacheExpander<InMemoryBackend> {
    let backend = InMemoryBackend::new();
    
    let ttl_policy = TtlPolicy::PerType(Box::new(|entity_type| {
        match entity_type {
            "employment" => Duration::from_secs(3600),
            "borrower" => Duration::from_secs(7200),
            _ => Duration::from_secs(1800),
        }
    }));

    CacheBuilder::new(backend)
        .with_ttl(ttl_policy)
        .with_metrics(Box::new(MyMetrics))
        .build()
}
```

---

## Registry Pattern

Use a registry for managing multiple feeders and repositories.

```rust
use std::collections::HashMap;
use cache_kit::{CacheEntity, CacheFeed, DataRepository, CacheExpander};

pub struct CacheRegistry<B: CacheBackend> {
    expander: CacheExpander<B>,
    feeders: HashMap<String, Box<dyn std::any::Any>>,
}

impl<B: CacheBackend> CacheRegistry<B> {
    pub fn new(backend: B) -> Self {
        CacheRegistry {
            expander: CacheExpander::new(backend),
            feeders: HashMap::new(),
        }
    }

    /// Register a feeder by name
    pub fn register_feeder<T: 'static>(
        &mut self,
        name: String,
        feeder: Box<dyn std::any::Any>,
    ) {
        self.feeders.insert(name, feeder);
    }

    /// Get a registered feeder
    pub fn get_feeder<T: 'static>(&self, name: &str) -> Option<&T> {
        self.feeders
            .get(name)
            .and_then(|f| f.downcast_ref::<T>())
    }
}

// Simpler approach: Direct registry for specific types
pub struct EntityRegistry {
    employment_feeders: Vec<(String, Option<Employment>)>,
    borrower_feeders: Vec<(String, Option<Borrower>)>,
}

impl EntityRegistry {
    pub fn new() -> Self {
        EntityRegistry {
            employment_feeders: Vec::new(),
            borrower_feeders: Vec::new(),
        }
    }

    pub fn fetch_employment_batch(
        &mut self,
        expander: &mut CacheExpander<InMemoryBackend>,
        ids: Vec<String>,
        repository: &dyn DataRepository<Employment>,
    ) {
        for id in ids {
            let mut feeder = GenericFeeder::<Employment>::new(id.clone());
            expander.with(&mut feeder, repository, CacheStrategy::Refresh).ok();
            self.employment_feeders.push((id, feeder.entity));
        }
    }

    pub fn fetch_borrower_batch(
        &mut self,
        expander: &mut CacheExpander<InMemoryBackend>,
        ids: Vec<String>,
        repository: &dyn DataRepository<Borrower>,
    ) {
        for id in ids {
            let mut feeder = GenericFeeder::<Borrower>::new(id.clone());
            expander.with(&mut feeder, repository, CacheStrategy::Refresh).ok();
            self.borrower_feeders.push((id, feeder.entity));
        }
    }
}

// Usage
fn batch_fetch_entities() {
    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);
    let mut registry = EntityRegistry::new();

    let employment_repo = EmploymentRepository;
    let ids = vec!["emp_1".to_string(), "emp_2".to_string(), "emp_3".to_string()];

    registry.fetch_employment_batch(&mut expander, ids, &employment_repo);

    for (id, employment) in registry.employment_feeders.iter() {
        if let Some(emp) = employment {
            println!("Employment {}: {:?}", id, emp);
        }
    }
}
```

---

## TTL Strategies

Different TTL approaches for different scenarios.

### Fixed TTL (Simple)

```rust
use cache_kit::observability::TtlPolicy;
use std::time::Duration;

let ttl = TtlPolicy::Fixed(Duration::from_secs(3600)); // 1 hour
```

### Per-Type TTL

```rust
let ttl = TtlPolicy::PerType(Box::new(|entity_type| {
    match entity_type {
        // Short-lived volatile data
        "transaction" => Duration::from_secs(60),
        
        // Medium-lived data
        "employment" => Duration::from_secs(1800),
        "borrower" => Duration::from_secs(3600),
        
        // Long-lived stable data
        "lending_term" => Duration::from_secs(86400),
        "site_settings" => Duration::from_secs(86400),
        
        // Default
        _ => Duration::from_secs(1800),
    }
}));
```

### Context-Aware TTL

```rust
use cache_kit::observability::TtlPolicy;
use cache_kit::strategy::CacheContext;
use std::time::Duration;

let ttl = TtlPolicy::Custom(Box::new(|context| {
    // Different TTL based on operation
    match context.strategy {
        // Fresh data is cached longer
        CacheStrategy::Fresh => Some(Duration::from_secs(7200)),
        
        // Refresh data for balance (1 hour)
        CacheStrategy::Refresh => Some(Duration::from_secs(3600)),
        
        // Invalidate has very short TTL
        CacheStrategy::Invalidate => Some(Duration::from_secs(300)),
        
        // Bypass doesn't cache
        CacheStrategy::Bypass => None,
        
        _ => Some(Duration::from_secs(1800)),
    }
}));
```

### User Role-Based TTL

```rust
let ttl = TtlPolicy::PerType(Box::new(|user_role| {
    match user_role {
        // Admin sees stale data for longer (trusted)
        "admin" => Duration::from_secs(7200),
        
        // Regular user gets fresher data
        "user" => Duration::from_secs(1800),
        
        // Guest gets very fresh data
        "guest" => Duration::from_secs(300),
        
        _ => Duration::from_secs(1800),
    }
}));
```

---

## Error Handling

Proper error handling patterns.

### Graceful Degradation

```rust
use cache_kit::error::Error;

fn fetch_with_fallback(
    expander: &mut CacheExpander<InMemoryBackend>,
    feeder: &mut impl CacheFeed<Employment>,
    repository: &dyn DataRepository<Employment>,
) {
    match expander.with(feeder, repository, CacheStrategy::Refresh) {
        Ok(_) => {
            println!("Successfully fetched from cache or database");
        }
        Err(Error::SerializationError(e)) => {
            eprintln!("Serialization error (corrupted cache?): {}", e);
            // Try to fetch fresh copy
            let _ = expander.with(feeder, repository, CacheStrategy::Invalidate);
        }
        Err(Error::RepositoryError(e)) => {
            eprintln!("Database error: {}", e);
            // Try cache-only
            let _ = expander.with(feeder, repository, CacheStrategy::Fresh);
        }
        Err(Error::BackendError(e)) => {
            eprintln!("Cache backend error: {}", e);
            // Bypass cache, go directly to DB
            let _ = expander.with(feeder, repository, CacheStrategy::Bypass);
        }
        Err(e) => {
            eprintln!("Other error: {:?}", e);
        }
    }
}
```

### Retry Logic

```rust
use std::time::Duration;

fn fetch_with_retry(
    expander: &mut CacheExpander<InMemoryBackend>,
    feeder: &mut impl CacheFeed<Employment>,
    repository: &dyn DataRepository<Employment>,
    max_retries: u32,
) -> Result<()> {
    let mut retries = 0;
    
    loop {
        match expander.with(feeder, repository, CacheStrategy::Refresh) {
            Ok(_) => return Ok(()),
            Err(e) => {
                retries += 1;
                if retries >= max_retries {
                    return Err(e);
                }
                
                eprintln!("Retry {}/{}: {:?}", retries, max_retries, e);
                std::thread::sleep(Duration::from_millis(100 * retries as u64));
            }
        }
    }
}
```

---

## Batch Operations

Cache multiple entities efficiently.

### Batch Get

```rust
pub fn batch_get<T: CacheEntity>(
    expander: &mut CacheExpander<impl CacheBackend>,
    ids: Vec<String>,
    repository: &dyn DataRepository<T>,
) -> Vec<Option<T>> {
    let mut results = Vec::new();
    
    for id in ids {
        let mut feeder = GenericFeeder::<T>::new(id);
        
        if expander.with(&mut feeder, repository, CacheStrategy::Refresh).is_ok() {
            results.push(feeder.entity);
        } else {
            results.push(None);
        }
    }
    
    results
}

// Usage
let employment_ids = vec!["emp_1".to_string(), "emp_2".to_string(), "emp_3".to_string()];
let results = batch_get::<Employment>(&mut expander, employment_ids, &repository);
```

### Parallel Batch Processing

```rust
use std::sync::Arc;
use std::sync::Mutex;

pub fn batch_get_parallel<T: CacheEntity + Send + 'static>(
    backend: Arc<InMemoryBackend>,
    ids: Vec<String>,
    repository: Arc<dyn DataRepository<T>>,
) -> Vec<Option<T>> {
    let results = Arc::new(Mutex::new(Vec::new()));
    let mut handles = vec![];

    for id in ids {
        let backend_clone = Arc::clone(&backend);
        let repo_clone = Arc::clone(&repository);
        let results_clone = Arc::clone(&results);

        let handle = std::thread::spawn(move || {
            let mut expander = CacheExpander::new((*backend_clone).clone());
            let mut feeder = GenericFeeder::<T>::new(id);

            if let Ok(_) = expander.with(&mut feeder, &*repo_clone, CacheStrategy::Refresh) {
                results_clone.lock().unwrap().push(feeder.entity);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Arc::into_inner(results)
        .unwrap()
        .into_inner()
        .unwrap()
}
```

---

## Multi-Tier Caching

Implement L1 (in-memory) and L2 (Redis) cache tiers.

```rust
use cache_kit::CacheExpander;
use cache_kit::backend::{InMemoryBackend, RedisBackend};

pub struct TieredCache {
    l1: CacheExpander<InMemoryBackend>, // Fast, local
    l2: CacheExpander<RedisBackend>,    // Distributed
}

impl TieredCache {
    pub fn new(l1_backend: InMemoryBackend, l2_backend: RedisBackend) -> Self {
        TieredCache {
            l1: CacheExpander::new(l1_backend),
            l2: CacheExpander::new(l2_backend),
        }
    }

    pub fn fetch<T, F>(
        &mut self,
        feeder: &mut F,
        repository: &dyn DataRepository<T>,
        strategy: CacheStrategy,
    ) -> Result<()>
    where
        T: CacheEntity,
        F: CacheFeed<T>,
    {
        // Try L1 first (fastest)
        match self.l1.with(feeder, repository, CacheStrategy::Fresh) {
            Ok(_) => {
                println!("L1 cache hit");
                return Ok(());
            }
            Err(_) => {
                // L1 miss, try L2
                println!("L1 cache miss, trying L2");
            }
        }

        // Try L2 (distributed)
        match self.l2.with(feeder, repository, CacheStrategy::Fresh) {
            Ok(_) => {
                println!("L2 cache hit, populating L1");
                // Populate L1 for next access
                self.l1.with(feeder, repository, CacheStrategy::Refresh).ok();
                return Ok(());
            }
            Err(_) => {
                // L2 miss, fetch from DB
                println!("L2 cache miss, fetching from database");
            }
        }

        // Fetch from database and populate both tiers
        self.l2.with(feeder, repository, strategy.clone())?;
        self.l1.with(feeder, repository, strategy)?;
        Ok(())
    }
}

// Usage
fn main() -> Result<()> {
    let l1 = InMemoryBackend::new();
    let l2 = RedisBackend::new(RedisConfig::default())?;
    
    let mut cache = TieredCache::new(l1, l2);
    
    let mut feeder = GenericFeeder::<Employment>::new("emp_1".to_string());
    let repo = EmploymentRepository;
    
    cache.fetch(&mut feeder, &repo, CacheStrategy::Refresh)?;
    
    Ok(())
}
```

---

## Custom Serialization

Override default serialization for specific entities.

```rust
use cache_kit::CacheEntity;
use serde_json::json;

#[derive(Clone, Serialize, Deserialize)]
pub struct LargeEntity {
    pub id: String,
    pub data: Vec<u8>, // Large binary data
}

impl CacheEntity for LargeEntity {
    type Key = String;

    fn cache_key(&self) -> Self::Key {
        self.id.clone()
    }

    fn cache_prefix() -> &'static str {
        "large"
    }

    /// Use MessagePack instead of JSON for efficiency
    fn serialize_for_cache(&self) -> Result<Vec<u8>> {
        rmp_serde::to_vec(self)
            .map_err(|e| Error::SerializationError(e.to_string()))
    }

    fn deserialize_from_cache(bytes: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(bytes)
            .map_err(|e| Error::SerializationError(e.to_string()))
    }
}
```

---

## Async Patterns

cache-kit is async-first. Use direct `.await` calls:

```rust
use cache_kit::{CacheExpander, backend::CacheBackend, CacheEntity, CacheFeed, DataRepository, CacheStrategy};

pub async fn fetch_cached<T, F, R>(
    expander: &CacheExpander<impl CacheBackend>,
    feeder: &mut F,
    repository: &R,
    strategy: CacheStrategy,
) -> cache_kit::Result<()>
where
    T: CacheEntity,
    F: CacheFeed<T>,
    R: DataRepository<T>,
{
    // Direct async call - no blocking needed
    expander.with::<T, F, R>(feeder, repository, strategy).await
}

// Usage in async context
#[tokio::main]
async fn main() -> cache_kit::Result<()> {
    let backend = InMemoryBackend::new();
    let expander = CacheExpander::new(backend);

    let mut feeder = GenericFeeder::<Employment>::new("emp_1".to_string());
    let repo = EmploymentRepository;

    fetch_cached(&expander, &mut feeder, &repo, CacheStrategy::Refresh).await?;

    Ok(())
}
```

### Async Batch Processing

```rust
use cache_kit::{CacheService, CacheEntity, CacheFeed, DataRepository, strategy::CacheStrategy};
use cache_kit::backend::InMemoryBackend;
use std::sync::Arc;

pub async fn fetch_batch_async<T: CacheEntity + Send + 'static>(
    cache: CacheService<InMemoryBackend>,
    ids: Vec<String>,
    repository: Arc<dyn DataRepository<T> + Send + Sync>,
) -> Vec<Option<T>> {
    let mut tasks = vec![];

    for id in ids {
        let cache_clone = cache.clone();
        let repo_clone = Arc::clone(&repository);

        let task = tokio::spawn(async move {
            let mut feeder = GenericFeeder::<T>::new(id);

            if cache_clone.execute(&mut feeder, &*repo_clone, CacheStrategy::Refresh).await.is_ok() {
                feeder.entity
            } else {
                None
            }
        });

        tasks.push(task);
    }

    let mut results = vec![];
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }

    results
}
```

---

## See Also

- [CONTRIBUTING.md](CONTRIBUTING.md) - How to extend the framework
- [TESTING.md](TESTING.md) - Testing strategies
- [README.md](README.md) - Quick start guide
