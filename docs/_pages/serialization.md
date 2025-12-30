---
layout: single
title: Serialization Support
description: "Understanding serialization formats and limitations in cache-kit"
permalink: /serialization/
---




---

## ⚠️ Critical Limitation

**Decimal types (`rust_decimal::Decimal`, `bigdecimal::BigDecimal`) are NOT supported by Postcard serialization.**

If your entities use Decimal fields (common in financial apps), you MUST convert to `String` or `i64` before caching. See [Decimal Workarounds](#decimal-types-workaround) below.

---

## Serialization as a First-Class Concern

cache-kit treats serialization as a **pluggable, first-class concern**.

Serialization determines:
- **Storage format** in the cache backend
- **Performance** characteristics (speed, size)
- **Type support** (which Rust types can be cached)
- **Interoperability** (can other languages read the cache?)

---

## Supported Formats

### Tier-1: Postcard (Recommended)

**Postcard** is the primary recommended serialization format for cache-kit.

| Feature | Postcard |
|---------|----------|
| **Performance** | ⚡ Very fast (10-15x faster than JSON) |
| **Size** | 📦 Compact (40-50% smaller than JSON) |
| **Type safety** | ✅ Strong Rust type preservation |
| **Determinism** | ✅ Same input → same output |
| **Language support** | ❌ Rust-only |
| **Decimal support** | ❌ No (see limitations below) |

#### Why Postcard?

- **Optimized for Rust** — Zero-copy deserialization where possible
- **No schema evolution** — Simple, explicit versioning
- **Minimal overhead** — Field order matters, no field names stored
- **Fast** — Designed for embedded and performance-critical systems

#### Installation

Postcard is included by default:

```toml
[dependencies]
cache-kit = "0.9"
```

#### Usage

No explicit configuration needed — cache-kit uses Postcard automatically:

```rust
use cache_kit::CacheEntity;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct User {
    id: String,
    name: String,
    age: u32,
}

impl CacheEntity for User {
    type Key = String;
    fn cache_key(&self) -> Self::Key { self.id.clone() }
    fn cache_prefix() -> &'static str { "user" }
}

// Serialization to Postcard happens automatically
```

---

### Tier-2: MessagePack (Planned)

**MessagePack** will be available as an alternative serialization format.

| Feature | MessagePack (Planned) |
|---------|----------------------|
| **Performance** | ⚡ Fast (4-6x faster than JSON) |
| **Size** | 📦 Compact (50% smaller than JSON) |
| **Type safety** | ⚠️ Partial |
| **Determinism** | ⚠️ Partial (field order varies) |
| **Language support** | ✅ Many languages |
| **Decimal support** | ⚠️ Depends on implementation |

**Community contributions welcome!** Help us add MessagePack support.

---

## Serialization Characteristics

### Postcard: Binary, Deterministic

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Product {
    id: u64,          // 8 bytes (compact)
    name: String,     // length-prefixed
    price: f64,       // 8 bytes (IEEE 754)
}

// Serialized format (example):
// [id: 8 bytes][name_len: varint][name: UTF-8 bytes][price: 8 bytes]
```

**Key property:** Serializing the same value twice produces **identical bytes**.

```rust
let product1 = Product { id: 123, name: "Widget".to_string(), price: 99.99 };
let product2 = Product { id: 123, name: "Widget".to_string(), price: 99.99 };

let bytes1 = postcard::to_allocvec(&product1)?;
let bytes2 = postcard::to_allocvec(&product2)?;

assert_eq!(bytes1, bytes2);  // ✅ Always true
```

This enables:
- **Reliable cache keys** based on content
- **Deduplication** in distributed caches
- **Reproducible testing**

---

## Known Limitations

### Decimal Types Not Supported

Postcard (and many binary formats) do **not support** arbitrary-precision decimal types out of the box.

Affected types:
- `rust_decimal::Decimal`
- `bigdecimal::BigDecimal`
- Database `NUMERIC` / `DECIMAL` columns

#### Why This Limitation Exists

Binary formats like Postcard serialize types based on their in-memory representation. Decimal types have complex internal structures that don't map cleanly to portable binary formats.

#### Workaround Strategies

##### Strategy 1: Convert to Supported Primitives

Store monetary values as **integer cents** instead of decimal dollars:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct Product {
    id: String,
    name: String,
    price_cents: i64,  // ✅ Store $99.99 as 9999 cents
}

impl Product {
    pub fn price_dollars(&self) -> f64 {
        self.price_cents as f64 / 0.9.0
    }

    pub fn set_price_dollars(&mut self, dollars: f64) {
        self.price_cents = (dollars * 0.9.0).round() as i64;
    }
}
```

**Pros:**
- ✅ No precision loss for monetary values
- ✅ Fast serialization
- ✅ Compact storage

**Cons:**
- ❌ Manual conversion needed
- ❌ Limited to representable range of `i64`

##### Strategy 2: Cache-Specific DTOs

Create separate types for caching:

```rust
// Database model (with Decimal)
#[derive(sqlx::FromRow)]
struct ProductRow {
    id: String,
    name: String,
    price: rust_decimal::Decimal,  // Database DECIMAL type
}

// Cache model (with supported types)
#[derive(Clone, Serialize, Deserialize)]
struct CachedProduct {
    id: String,
    name: String,
    price_cents: i64,  // Converted from Decimal
}

impl CacheEntity for CachedProduct {
    type Key = String;
    fn cache_key(&self) -> Self::Key { self.id.clone() }
    fn cache_prefix() -> &'static str { "product" }
}

impl From<ProductRow> for CachedProduct {
    fn from(row: ProductRow) -> Self {
        CachedProduct {
            id: row.id,
            name: row.name,
            price_cents: (row.price * rust_decimal::Decimal::from(100))
                .to_i64()
                .unwrap_or(0),
        }
    }
}
```

**Pros:**
- ✅ Clean separation of concerns
- ✅ Database can use appropriate types
- ✅ Cache uses efficient types

**Cons:**
- ❌ Requires type conversion
- ❌ More boilerplate

##### Strategy 3: String Representation

Store decimals as strings (not recommended for performance):

```rust
#[derive(Clone, Serialize, Deserialize)]
struct Product {
    id: String,
    name: String,
    price: String,  // "99.99" as string
}
```

**Pros:**
- ✅ No precision loss
- ✅ Preserves exact decimal representation

**Cons:**
- ❌ Slower serialization
- ❌ Larger storage footprint
- ❌ Manual parsing required

##### Strategy 4: Use MessagePack (Future)

When MessagePack support is added, you may have more flexibility for decimal types.

**Community contributions welcome!**

---

## Serialization Best Practices

### DO

✅ Use primitive types where possible (`i64`, `f64`, `String`)
✅ Convert decimals to integers (cents) for monetary values
✅ Create cache-specific DTOs if needed
✅ Document conversion logic clearly
✅ Test roundtrip serialization

### DON'T

❌ Assume all Rust types are serializable
❌ Mix database types with cache types without conversion
❌ Ignore serialization errors
❌ Use `unwrap()` on deserialization
❌ Store sensitive data without encryption

---

## Custom Serialization

If you need custom serialization for specific types, implement `serde` traits:

```rust
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use rust_decimal::Decimal;

#[derive(Clone)]
struct CustomProduct {
    id: String,
    name: String,
    price: Decimal,
}

impl Serialize for CustomProduct {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("CustomProduct", 3)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("name", &self.name)?;

        // Convert Decimal to i64 cents
        let price_cents = (self.price * Decimal::from(100))
            .to_i64()
            .ok_or_else(|| serde::ser::Error::custom("Price out of range"))?;
        state.serialize_field("price_cents", &price_cents)?;

        state.end()
    }
}

impl<'de> Deserialize<'de> for CustomProduct {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            id: String,
            name: String,
            price_cents: i64,
        }

        let helper = Helper::deserialize(deserializer)?;

        Ok(CustomProduct {
            id: helper.id,
            name: helper.name,
            price: Decimal::from(helper.price_cents) / Decimal::from(100),
        })
    }
}
```

---

## Versioning and Schema Evolution

cache-kit uses **explicit versioning** for cached data.

### Current Approach

cache-kit wraps all cached entries in a versioned envelope:

```
[MAGIC (4 bytes)] [VERSION (4 bytes)] [POSTCARD PAYLOAD]
```

- **MAGIC:** `b"CKIT"` — Identifies cache-kit entries
- **VERSION:** `u32` — Schema version number
- **PAYLOAD:** Postcard-serialized entity

### Version Mismatches

When the schema version changes:

1. **Old entries rejected** — Cannot be deserialized
2. **Cache miss triggered** — Fetch from database
3. **New entry cached** — With updated version

**No migration** — Cache naturally repopulates with new schema.

### Handling Schema Changes

When you modify an entity structure:

```rust
// Version 1
struct User {
    id: String,
    name: String,
}

// Version 2 (added field)
struct User {
    id: String,
    name: String,
    email: Option<String>,  // New field
}
```

**Action required:**
1. Bump schema version in cache-kit configuration
2. Deploy new code
3. Old cache entries will be invalidated automatically
4. New entries cached with updated schema

---

## Performance Characteristics

### Postcard Performance

Based on typical workloads:

| Operation | Time | Throughput |
|-----------|------|------------|
| Serialize (1KB entity) | 50-100 ns | 10-20M ops/sec |
| Deserialize (1KB entity) | 60-120 ns | 8-16M ops/sec |

**Comparison with JSON:**

| Metric | JSON | Postcard | Improvement |
|--------|------|----------|-------------|
| Serialize | 1.2 µs | 80 ns | **15x faster** |
| Deserialize | 1.5 µs | 100 ns | **15x faster** |
| Size (1KB entity) | 158 bytes | 95 bytes | **40% smaller** |

---

## Example: Complete Serialization Flow

```rust
use cache_kit::{CacheEntity, CacheExpander};
use cache_kit::backend::InMemoryBackend;
use serde::{Deserialize, Serialize};

// 1. Define entity (automatically uses Postcard)
#[derive(Clone, Serialize, Deserialize, Debug)]
struct User {
    id: String,
    name: String,
    age: u32,
}

impl CacheEntity for User {
    type Key = String;
    fn cache_key(&self) -> Self::Key { self.id.clone() }
    fn cache_prefix() -> &'static str { "user" }
}

fn main() -> cache_kit::Result<()> {
    let backend = InMemoryBackend::new();
    let mut expander = CacheExpander::new(backend);

    let user = User {
        id: "user_001".to_string(),
        name: "Alice".to_string(),
        age: 30,
    };

    // Serialization happens automatically
    // [MAGIC][VERSION][Postcard bytes]
    expander.set(&user, None)?;

    // Deserialization happens automatically
    let cached: Option<User> = expander.get(&"user_001".to_string())?;

    println!("Cached user: {:?}", cached);

    Ok(())
}
```

---

## Troubleshooting

### Error: "Serialization failed"

**Cause:** Entity contains unsupported types (e.g., `Decimal`)

**Solution:** Convert to supported primitives or use cache-specific DTOs

### Error: "Version mismatch"

**Cause:** Cached entry has different schema version

**Solution:** This is expected after schema changes. Entry will be invalidated and refetched.

### Error: "Invalid magic header"

**Cause:** Cache entry is corrupted or not created by cache-kit

**Solution:** Clear the cache key manually or let it expire

---

## Next Steps

- Learn about [Cache backend options](backends)
- Review [Design principles](design-principles)
- Explore the [Actix + SQLx reference implementation](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx)
- **Contribute MessagePack support!** See [CONTRIBUTING.md](https://github.com/megamsys/cache-kit.rs/blob/main/CONTRIBUTING.md)
