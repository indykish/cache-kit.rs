---
layout: single
title: Contributing Guide
parent: Guides
---


Thank you for contributing! This guide covers code standards, testing requirements, and submission process.

---

## Quick Links

- **Development Guides:** See [ARCHITECTURE_*.md](.) files for detailed implementation guides
- **Code Examples:** See [examples/](examples/) directory
- **Testing Guide:** See [TESTING.md](TESTING.md)

---

## Table of Contents

1. [Getting Started](#getting-started)
2. [Development Workflow](#development-workflow)
3. [Architecture Documentation](#architecture-documentation)
4. [Code Style & Standards](#code-style--standards)
5. [Testing Requirements](#testing-requirements)
6. [Submitting Changes](#submitting-changes)

---

## Getting Started

### Prerequisites

- Rust 1.75 or higher
- Cargo
- Git

### Clone and Build

```bash
git clone https://github.com/megamsys/cache-kit.rs
cd cache-kit.rs

# Build
cargo build --all-features

# Run tests
cargo test --all-features

# Run examples
cargo run --example basic_usage
```

---

## Development Workflow

### 1. Choose What to Work On

- Check [GitHub Issues](https://github.com/megamsys/cache-kit.rs/issues)
- Review [ARCHITECTURE_*.md](.) files for planned features
- Propose new features by opening an issue first

### 2. Create a Branch

```bash
git checkout -b feature/my-feature
# or
git checkout -b fix/issue-123
```

### 3. Make Changes

Follow the [Code Style & Standards](#code-style--standards) below.

### 4. Test Your Changes

See [Testing Requirements](#testing-requirements) below.

### 5. Submit Pull Request

See [Submitting Changes](#submitting-changes) below.

---

## Architecture Documentation

For detailed implementation guides, see the architecture documents:

### Implementation Guides

- **[ARCHITECTURE_01_ACTIX_EXAMPLE.md](ARCHITECTURE_01_ACTIX_EXAMPLE.md)** - Actix Web framework integration
  - REST API endpoints with caching
  - Complete code examples
  - Testing strategies

- **[ARCHITECTURE_02_SERIALIZATION.md](ARCHITECTURE_02_SERIALIZATION.md)** - Efficient serialization formats
  - Bincode and MessagePack support
  - CacheSerializer trait implementation
  - Performance benchmarks

- **[ARCHITECTURE_03_BENCHMARK.md](ARCHITECTURE_03_BENCHMARK.md)** - Performance benchmarking
  - Criterion setup and configuration
  - Benchmark groups and test cases
  - Performance baselines

- **[ARCHITECTURE_04_REGISTRY.md](ARCHITECTURE_04_REGISTRY.md)** - Transactional cache registry
  - Multi-entity cache operations
  - Transaction support with rollback
  - Relationship graph for cascade invalidation

### When Building Extensions

**Before implementing a custom backend, feeder, metrics, or repository:**

1. Review the relevant ARCHITECTURE_*.md file
2. Look at existing implementations in `src/`
3. Check `examples/` for usage patterns
4. Follow the trait definitions in the architecture docs

### Project Structure

```
cache-kit/
├── src/
│   ├── lib.rs                    # Library entry point
│   ├── entity.rs                 # CacheEntity<T> trait
│   ├── feed.rs                   # CacheFeed<T> trait
│   ├── repository.rs             # DataRepository<T> trait
│   ├── strategy.rs               # CacheStrategy enum
│   ├── expander.rs               # CacheExpander main orchestrator
│   ├── key.rs                    # Key management utilities
│   ├── observability.rs          # Metrics & TTL traits
│   ├── error.rs                  # Error types
│   └── backend/
│       ├── mod.rs                # CacheBackend trait
│       ├── inmemory.rs           # InMemoryBackend (reference)
│       ├── redis.rs              # RedisBackend
│       └── memcached.rs          # MemcachedBackend
├── examples/                     # Usage examples
├── tests/                        # Integration tests
├── ARCHITECTURE_*.md             # Implementation guides
└── README.md                     # Project overview
```

---

## Code Style & Standards

### Formatting

All code must be formatted with `rustfmt`:

```bash
cargo fmt
```

### Linting

All code must pass `clippy` without warnings:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Documentation

- Write doc comments (`///`) for all public items
- Include examples in doc comments for important functions
- Keep line length ≤ 100 characters
- Use `//!` for module-level documentation

**Example:**

```rust
/// Fetches a cached entity or falls back to the repository.
///
/// # Arguments
///
/// * `feeder` - The feeder that will receive the entity
/// * `repository` - The repository to fetch from on cache miss
/// * `strategy` - The cache strategy to use
///
/// # Returns
///
/// * `Ok(())` - Operation succeeded
/// * `Err(Error)` - Operation failed
///
/// # Example
///
/// ```
/// use cache_kit::*;
/// use cache_kit::backend::InMemoryBackend;
/// use cache_kit::strategy::CacheStrategy;
///
/// let mut expander = CacheExpander::new(InMemoryBackend::new());
/// expander.with(&mut feeder, &repo, CacheStrategy::Refresh)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn with<T, F>(...) { }
```

### Error Handling

- Use `Result<T, Error>` for all fallible operations
- Convert external errors to `crate::error::Error`
- Provide meaningful error messages
- Don't use `unwrap()` or `expect()` in library code

### Thread Safety

- All public types must be `Send + Sync` where appropriate
- Use `Arc` and `Mutex`/`RwLock` for shared state
- Document any thread safety assumptions

---

## Testing Requirements

### Unit Tests

Every module should have unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Test implementation
    }
}
```

### Integration Tests

Add integration tests in `tests/` directory for end-to-end scenarios.

### Running Tests

```bash
# Run all tests
cargo test --all-features

# Run tests for specific feature
cargo test --features redis

# Run with logging
RUST_LOG=debug cargo test

# Run specific test
cargo test test_name
```

### Coverage Requirements

- New features should have >80% test coverage
- Bug fixes should include regression tests
- Breaking changes require updated tests

### Test Checklist

- [ ] Unit tests for new code
- [ ] Integration tests for new features
- [ ] All tests pass: `cargo test --all-features`
- [ ] No test warnings
- [ ] Tests are deterministic (no flaky tests)

---

## Submitting Changes

### Pre-Submission Checklist

Before submitting a pull request, ensure:

```bash
# 1. Code builds
cargo build --all-features

# 2. Tests pass
cargo test --all-features

# 3. Clippy passes
cargo clippy --all-targets --all-features -- -D warnings

# 4. Code is formatted
cargo fmt --check

# 5. Documentation builds
cargo doc --no-deps
```

### Commit Message Format

Write clear, descriptive commit messages:

```
Short description (50 chars max)

Longer explanation if needed. Wrap at 72 characters.

- Bullet points are fine
- Use them to list key changes

Fixes #123
```

**Examples:**

- `Add Redis transaction support for atomic operations`
- `Fix cache key generation for composite keys`
- `Update documentation for builder pattern`

### Pull Request Process

1. **Fork** the repository
2. **Create a feature branch**: `git checkout -b feature/my-feature`
3. **Make changes** following guidelines above
4. **Add tests** for your changes
5. **Run pre-submission checklist** (see above)
6. **Commit** with clear messages
7. **Push** to your fork: `git push origin feature/my-feature`
8. **Create Pull Request** with:
   - Clear title and description
   - Link to related issues
   - Description of changes
   - Testing performed

### Pull Request Review

- Maintainers will review your PR
- Address feedback and comments
- Update your PR as needed
- Once approved, it will be merged

---

## Development Tips

### Testing Backends Locally

**Redis:**

```bash
# Start Redis with Docker
docker run -d -p 6379:6379 redis:latest

# Run Redis tests
cargo test --features redis
```

**Memcached:**

```bash
# Start Memcached with Docker
docker run -d -p 11211:11211 memcached:latest

# Run Memcached tests
cargo test --features memcached
```

### Debugging

Enable debug logging:

```bash
RUST_LOG=debug cargo test test_name -- --nocapture
```

### Benchmarking

See the [Performance Guide](performance) for comprehensive benchmarking with Criterion, including baseline comparison and regression detection.

---

## Version Management

### Releasing a New Version

**Version Management:**

cache-kit uses a `VERSION` file as the single source of truth.

**To bump version:**

```bash
# Updates VERSION, Cargo.toml, README.md, and all docs/*.md files
make version-bump VERSION=0.10.0
```

**Pre-Release Checklist:**

- [ ] Run: `make version-bump VERSION=0.10.0`
- [ ] Update CHANGELOG.md with release notes
- [ ] Run tests: `cargo test --all-features`
- [ ] Commit: `git commit -m "Release v0.10.0"`
- [ ] Tag: `git tag v0.10.0`
- [ ] Push: `git push origin main --tags`
- [ ] Publish: `cargo publish`

**Note:** `build.rs` validates VERSION matches Cargo.toml at compile time.

### Code Example Best Practices

**Use semantic versioning in documentation examples:**

```toml
# ✅ Recommended: Semantic versioning
cache-kit = "1"                    # Latest 1.x.x
cache-kit = { version = "1", features = ["redis"] }

# ❌ Avoid: Exact versions (requires update for every patch)
cache-kit = "0.9.0"
```

This ensures code examples don't need updating for minor/patch releases.

---

## Questions?

- **Issues:** [GitHub Issues](https://github.com/megamsys/cache-kit.rs/issues)
- **Discussions:** [GitHub Discussions](https://github.com/megamsys/cache-kit.rs/discussions)
- **Email:** nkishore@megam.io

---

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
