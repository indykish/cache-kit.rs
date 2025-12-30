# Golden Blob Test Files

This directory contains **golden blob files** - reference serialized cache entries used for regression testing.

## Purpose

Golden blobs ensure **serialization format stability** across code changes:
- ✅ **Backward compatibility**: New code can read old cache entries
- ✅ **Accidental change detection**: Refactoring doesn't break serialization
- ✅ **Version migration validation**: Schema changes are intentional

## Files

| File | Schema Version | Description |
|------|---------------|-------------|
| `user_v1.bin` | 1 | Serialized User entity (id: 42, name: "Alice") |
| `product_v1.bin` | 1 | Serialized Product entity (id: "prod_123") |
| `complex_v1.bin` | 1 | Serialized ComplexEntity with collections |

## How Golden Blobs Work

### Normal Development (No Schema Change)

```rust
// You refactor code, reorder fields, etc.
#[derive(Serialize, Deserialize)]
struct User {
    name: String,  // Reordered fields
    id: u64,
}

// ❌ Golden blob test FAILS!
// → You realize field order changed serialization
// → You fix it or bump version
```

### Intentional Schema Change

```rust
// Step 1: Change struct
#[derive(Serialize, Deserialize)]
struct User {
    id: u64,
    name: String,
    email: String,  // NEW FIELD
}

// Step 2: Bump version
const CURRENT_SCHEMA_VERSION: u32 = 2;  // Was 1

// Step 3: Regenerate golden blobs (see below)

// Step 4: Old cache entries (v1) will be automatically evicted
```

## When to Regenerate Golden Blobs

**Regenerate when you INTENTIONALLY change the schema:**

1. ✅ Added a new field to a struct
2. ✅ Removed a field from a struct
3. ✅ Changed a field type
4. ✅ Upgraded Bincode version
5. ✅ Changed serialization logic

**DO NOT regenerate for:**
- ❌ Refactoring (renaming variables)
- ❌ Code cleanup
- ❌ Documentation updates
- ❌ Non-schema changes

## How to Regenerate Golden Blobs

### Manual Method

Run the golden blob generator:

\`\`\`bash
cargo test --test golden_blob_generator -- --nocapture
\`\`\`

This will:
1. Serialize reference entities
2. Write new `.bin` files to `tests/golden/`
3. Display checksums for verification

### Verification

After regeneration:

\`\`\`bash
# Verify new golden blobs work
cargo test --test golden_blobs

# Should see:
# test test_deserialize_user_v1 ... ok
# test test_deserialize_product_v1 ... ok
# test test_deserialize_complex_v1 ... ok
\`\`\`

## File Format

Each golden blob file contains:

\`\`\`
[MAGIC: b"CKIT" (4 bytes)]
[VERSION: u32 LE (4 bytes)]
[BINCODE PAYLOAD: Variable length]
\`\`\`

### Inspecting Golden Blobs

\`\`\`bash
# View hex dump
hexdump -C tests/golden/user_v1.bin | head -20

# Expected output:
# 00000000  43 4b 49 54 01 00 00 00  ...  |CKIT....|
#            ^^^^^^^^^^^ ^^^^^^^^^^^
#            Magic       Version (1)
\`\`\`

## Troubleshooting

### Golden blob test fails after refactoring

**Cause**: You accidentally changed serialization format (e.g., field order)

**Solution**:
1. Revert the change, OR
2. If intentional, bump `CURRENT_SCHEMA_VERSION` and regenerate

### Golden blob test fails after library update

**Cause**: Bincode version changed serialization format

**Solution**:
1. Bump `CURRENT_SCHEMA_VERSION`
2. Regenerate golden blobs
3. Update CHANGELOG noting cache invalidation

### Can't deserialize old golden blob

**Cause**: You bumped the schema version (expected!)

**Solution**:
- This is correct behavior
- Old cache entries will be evicted in production
- Regenerate golden blobs for new version

## Production Impact

When you regenerate golden blobs (schema change):

### What Happens in Production

1. **Deploy new code** with bumped `CURRENT_SCHEMA_VERSION`
2. **Old cache entries** (version 1) are rejected
3. **Cache misses** trigger database reads
4. **New cache entries** (version 2) are written
5. **Gradual migration** - no manual cache flush needed

### Monitoring

Watch these metrics during deployment:

\`\`\`
cache.version_mismatch  # Should spike temporarily
cache.hit_rate          # Will drop then recover
cache.miss_count        # Will spike then normalize
\`\`\`

## Golden Blob Checklist

Before committing schema changes:

- [ ] Bumped `CURRENT_SCHEMA_VERSION` in `src/serialization/mod.rs`
- [ ] Regenerated golden blobs: `cargo test --test golden_blob_generator`
- [ ] Verified tests pass: `cargo test --test golden_blobs`
- [ ] Updated CHANGELOG with cache invalidation note
- [ ] Documented schema change in PR description
- [ ] Planned for cache hit rate drop during rollout

## References

- Main doc: `docs/architecture/02-serialization-bincode.md`
- Serialization code: `src/serialization/mod.rs`
- Test code: `tests/golden_blobs.rs`
- Generator: `tests/golden_blob_generator.rs`
