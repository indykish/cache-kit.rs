---
layout: single
title: Database & ORM Compatibility
description: "Using cache-kit with different ORMs and database layers"
permalink: /database-compatibility/
---




---

## ORM-Agnostic Design

cache-kit does **not depend on ORMs**.

It operates on three simple concepts:
- **Serializable entities** — Any type implementing `CacheEntity`
- **Deterministic cache keys** — Consistent identifiers
- **Explicit cache boundaries** — Clear separation via `CacheFeed`

This means:
- ✅ Swap ORMs without changing cache logic
- ✅ Use multiple ORMs in the same application
- ✅ Cache data from any source (DB, API, file system)

---

## Supported ORMs & Database Layers

### Tier-1: Recommended (with Examples)

| ORM | Status | Example | Notes |
|-----|--------|---------|-------|
| **SQLx** | ✅ Full Support | [actixsqlx](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx) | Async-first, compile-time checked SQL |

### Tier-1: Compatible (Community Examples Welcome)

| ORM | Status | Example | Notes |
|-----|--------|---------|-------|
| **SeaORM** | ✅ Compatible | Community contributions welcome | Async ORM with migrations |
| **Diesel** | ✅ Compatible | Community contributions welcome | Mature, type-safe ORM |
| **tokio-postgres** | ✅ Compatible | Works with any database layer | Pure async PostgreSQL client |

### Tier-2: Any Database Layer

cache-kit works with **any** Rust code that can:
1. Fetch entities by ID
2. Return `Option<T>` (entity or not found)
3. Implement `DataRepository<T>` trait

This includes:
- Custom SQL builders
- NoSQL databases (MongoDB, DynamoDB)
- REST API clients
- File-based storage
- In-memory data structures

---

## Conceptual Flow

```
┌─────────────────────┐
│ Database / ORM      │ ← Your choice
└──────────┬──────────┘
           │
           ↓ Fetch entities
┌─────────────────────┐
│ Domain Entities     │ ← impl CacheEntity
└──────────┬──────────┘
           │
           ↓ Cache operations
┌─────────────────────┐
│ cache-kit           │ ← Framework-agnostic
└──────────┬──────────┘
           │
           ↓ Store/retrieve
┌─────────────────────┐
│ Cache Backend       │ ← Redis, Memcached, InMemory
└─────────────────────┘
```

**Key principle:** Database models live in your database layer, cache-kit just coordinates caching.

---

## SQLx Integration

SQLx is an async, compile-time checked SQL library. It's the recommended database layer for new projects.

### Installation

```toml
[dependencies]
cache-kit = "0.9"
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono"] }
tokio = { version = "1.41", features = ["full"] }
serde = { version = "0.9", features = ["derive"] }
```

### Entity Definition

```rust
use serde::{Deserialize, Serialize};
use cache_kit::CacheEntity;

#[derive(Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl CacheEntity for User {
    type Key = String;

    fn cache_key(&self) -> Self::Key {
        self.id.clone()
    }

    fn cache_prefix() -> &'static str {
        "user"
    }
}
```

### Repository Implementation

```rust
use cache_kit::DataRepository;
use sqlx::PgPool;

pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, user: &User) -> Result<User, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (id, username, email)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
            user.id,
            user.username,
            user.email
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update(&self, user: &User) -> Result<User, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"
            UPDATE users
            SET username = $2, email = $3
            WHERE id = $1
            RETURNING *
            "#,
            user.id,
            user.username,
            user.email
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM users WHERE id = $1", id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

impl DataRepository<User> for UserRepository {
    async fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<User>> {
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
```

### Usage in Service Layer

```rust
use cache_kit::{CacheService, CacheFeed, DataRepository, strategy::CacheStrategy};
use cache_kit::backend::InMemoryBackend;
use std::sync::Arc;

pub struct UserService {
    cache: CacheService<InMemoryBackend>,
    repo: Arc<UserRepository>,
}

impl UserService {
    pub async fn get_user(&self, id: &str) -> cache_kit::Result<Option<User>> {
        let mut feeder = UserFeeder {
            id: id.to_string(),
            user: None,
        };

        self.cache
            .execute(&mut feeder, &*self.repo, CacheStrategy::Refresh)
            .await?;

        Ok(feeder.user)
    }
}
```

See the full [Actix + SQLx example](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx) for a complete implementation.

---

## SeaORM Integration
use std::sync::Arc;
use cache_kit::{CacheExpander, strategy::CacheStrategy};
use cache_kit::backend::InMemoryBackend;

pub struct UserService {
    repo: Arc<UserRepository>,
    cache: Arc<CacheExpander<InMemoryBackend>>,
}

impl UserService {
    pub fn new(repo: Arc<UserRepository>, cache: Arc<CacheExpander<InMemoryBackend>>) -> Self {
        Self { repo, cache }
    }

    pub async fn get(&self, id: &str) -> cache_kit::Result<Option<User>> {
        let mut feeder = UserFeeder {
            id: id.to_string(),
            user: None,
        };

        self.cache.with(&mut feeder, &*self.repo, CacheStrategy::Refresh)?;

        Ok(feeder.user)
    }

    pub async fn create(&self, user: User) -> cache_kit::Result<User> {
        // 1. Insert into database
        let created = self.repo.create(&user).await
            .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        // 2. Cache the new entity
        let mut feeder = UserFeeder {
            id: created.id.clone(),
            user: Some(created.clone()),
        };
        self.cache.with(&mut feeder, &*self.repo, CacheStrategy::Refresh)?;

        Ok(created)
    }

    pub async fn update(&self, user: User) -> cache_kit::Result<User> {
        // 1. Update database
        let updated = self.repo.update(&user).await
            .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        // 2. Invalidate cache and fetch fresh
        let mut feeder = UserFeeder {
            id: updated.id.clone(),
            user: None,
        };
        self.cache.with(&mut feeder, &*self.repo, CacheStrategy::Invalidate)?;

        Ok(updated)
    }

    pub async fn delete(&self, id: &str) -> cache_kit::Result<()> {
        // 1. Delete from database
        self.repo.delete(id).await
            .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        // 2. Invalidate cache
        let mut feeder = UserFeeder {
            id: id.to_string(),
            user: None,
        };
        self.cache.with(&mut feeder, &*self.repo, CacheStrategy::Invalidate)?;

        Ok(())
    }
}
```

See the complete [actixsqlx example](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx) for a full working implementation.

---

## SeaORM Integration

SeaORM is an async ORM with migrations and schema management.

### Entity Definition

```rust
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use cache_kit::CacheEntity;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub username: String,
    pub email: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Implement CacheEntity for the SeaORM Model
impl CacheEntity for Model {
    type Key = String;

    fn cache_key(&self) -> Self::Key {
        self.id.clone()
    }

    fn cache_prefix() -> &'static str {
        "user"
    }
}
```

### Repository Implementation

```rust
use sea_orm::{DatabaseConnection, EntityTrait};
use cache_kit::DataRepository;

pub struct UserRepository {
    db: DatabaseConnection,
}

impl DataRepository<user::Model> for UserRepository {
    async fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<user::Model>> {
        let user = user::Entity::find_by_id(id.clone())
            .one(&self.db)
            .await
            .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        Ok(user)
    }
}
```

**Community contributions welcome!** Share your SeaORM + cache-kit examples.

---

## Diesel Integration

Diesel is a mature, type-safe ORM with excellent compile-time guarantees.

### Entity Definition

```rust
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use cache_kit::CacheEntity;

#[derive(Queryable, Selectable, Clone, Serialize, Deserialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
}

impl CacheEntity for User {
    type Key = String;

    fn cache_key(&self) -> Self::Key {
        self.id.clone()
    }

    fn cache_prefix() -> &'static str {
        "user"
    }
}
```

### Repository Implementation (Sync)

```rust
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use cache_kit::DataRepository;

pub struct UserRepository {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl DataRepository<User> for UserRepository {
    fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<User>> {
        use crate::schema::users::dsl::*;

        let mut conn = self.pool.get()
            .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        let user = users
            .filter(id.eq(id))
            .first::<User>(&mut conn)
            .optional()
            .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))?;

        Ok(user)
    }
}
```

**Note:** Diesel is synchronous, which works perfectly with cache-kit's sync `DataRepository` trait.

**Community contributions welcome!** Share your Diesel + cache-kit examples.

---

## Custom Database Layer

cache-kit works with any custom database layer:

```rust
use cache_kit::DataRepository;

pub struct CustomRepository {
    // Your custom database client
    client: MyDatabaseClient,
}

impl DataRepository<MyEntity> for CustomRepository {
    fn fetch_by_id(&self, id: &MyEntityId) -> cache_kit::Result<Option<MyEntity>> {
        // Your custom fetching logic
        match self.client.query(id) {
            Ok(Some(data)) => Ok(Some(MyEntity::from(data))),
            Ok(None) => Ok(None),
            Err(e) => Err(cache_kit::Error::RepositoryError(e.to_string())),
        }
    }
}
```

---

## Database Best Practices

### Separate Concerns

```
✅ Good:
    Database Models → Repository → Cache → Service → API

❌ Bad:
    Database Models with embedded cache logic
```

### Repository Pattern

Keep repositories focused on data access:

```rust
impl UserRepository {
    // ✅ Simple, focused data access
    pub async fn find_by_id(&self, id: &str) -> Result<Option<User>, DbError> {
        sqlx::query_as!(...).await
    }

    // ❌ Don't mix cache logic in repository
    pub async fn find_by_id_cached(&self, id: &str) -> Result<Option<User>, DbError> {
        // BAD: Repository shouldn't know about caching
    }
}
```

### Error Handling

Convert database errors to cache-kit errors:

```rust
impl DataRepository<User> for UserRepository {
    fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<User>> {
        self.internal_fetch(id)
            .map_err(|e| cache_kit::Error::RepositoryError(e.to_string()))
    }
}
```

---

## Database Migrations

cache-kit does not handle database migrations. Use your ORM's migration tools:

### SQLx

```bash
sqlx migrate add create_users_table
```

Edit `migrations/001_create_users_table.sql`:

```sql
CREATE TABLE users (
    id VARCHAR(255) PRIMARY KEY,
    username VARCHAR(100) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);
```

Run migrations:

```bash
sqlx migrate run --database-url postgres://localhost/mydb
```

### SeaORM

```bash
sea-orm-cli migrate generate create_users_table
```

### Diesel

```bash
diesel migration generate create_users_table
```

---

## Next Steps

- Learn about [API Frameworks and Transport Layers](api-frameworks)
- Explore [Serialization options](serialization)
- Review [Cache backend choices](backends)
- See the [Actix + SQLx reference implementation](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx)
