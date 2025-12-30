---
layout: single
title: API Frameworks & Transport Layers
description: "Using cache-kit across different API frameworks and transport protocols"
permalink: /api-frameworks/
---




---

## Framework Independence

cache-kit is **framework-agnostic**. It does not:

- Depend on HTTP libraries
- Assume REST semantics
- Tie to specific web frameworks
- Make transport-level decisions

This design allows cache-kit to work seamlessly across:
- REST APIs
- gRPC services
- GraphQL resolvers
- Background workers
- WebSocket servers
- CLI applications
- Library/SDK internals

---

## Framework Layer vs Transport Layer

cache-kit distinguishes between **framework** (application structure) and **transport** (communication protocol).

### Framework Layer

Frameworks provide application structure:
- Request routing
- Middleware
- State management
- Error handling

### Transport Layer

Transports handle communication:
- HTTP (REST)
- gRPC (Protocol Buffers)
- WebSockets
- Message queues

**cache-kit sits below both layers**, operating on domain entities regardless of how they're exposed.

---

## Conceptual Separation

```
┌─────────────────────────────────────────┐
│         Transport Layer                 │
│  (HTTP / gRPC / WebSocket / Workers)    │
└──────────────┬──────────────────────────┘
               │ Request/Response DTOs
               ↓
┌─────────────────────────────────────────┐
│        Framework Layer                  │
│     (Axum / Actix / Tonic / Tower)      │
└──────────────┬──────────────────────────┘
               │ Extract params
               ↓
┌─────────────────────────────────────────┐
│         Service Layer                   │
│     (Business logic + cache-kit)        │
└──────────────┬──────────────────────────┘
               │ Domain entities
               ↓
┌─────────────────────────────────────────┐
│      Repository Layer                   │
│        (Database / ORM)                 │
└─────────────────────────────────────────┘
```

**Key principle:** Transport must never leak into cache or business logic. Cached logic should be reusable across transports.

---

## Axum Integration (Recommended)

Axum is a modern, ergonomic web framework built on tokio and tower.

### Installation

```toml
[dependencies]
cache-kit = "0.9"
axum = "0.7"
tokio = { version = "1.41", features = ["full"] }
serde = { version = "0.9", features = ["derive"] }
```

### Complete Example

```rust
use axum::{
    Router,
    routing::get,
    extract::{State, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use cache_kit::{CacheExpander, strategy::CacheStrategy};
use cache_kit::backend::InMemoryBackend;
use std::sync::Arc;

// Shared application state
#[derive(Clone)]
struct AppState {
    cache: Arc<CacheExpander<InMemoryBackend>>,
    user_repo: Arc<UserRepository>,
}

// REST handler - clean and focused
async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<User>, StatusCode> {
    let mut feeder = UserFeeder {
        id: user_id,
        user: None,
    };

    state.cache
        .with(&mut feeder, &*state.user_repo, CacheStrategy::Refresh)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match feeder.user {
        Some(user) => Ok(Json(user)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<User>, StatusCode> {
    let user = User {
        id: uuid::Uuid::new_v4().to_string(),
        name: payload.name,
        email: payload.email,
    };

    // Create in database
    state.user_repo.create(&user).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Cache the new user
    let mut feeder = UserFeeder {
        id: user.id.clone(),
        user: Some(user.clone()),
    };
    state.cache
        .with(&mut feeder, &*state.user_repo, CacheStrategy::Refresh)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(user))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup cache and repository
    let cache = Arc::new(CacheExpander::new(InMemoryBackend::new()));
    let user_repo = Arc::new(UserRepository::new(/* db pool */));

    let state = AppState { cache, user_repo };

    // Build router
    let app = Router::new()
        .route("/users/:id", get(get_user))
        .route("/users", axum::routing::post(create_user))
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

---

## Actix Web Integration

Actix is a mature, high-performance web framework.

See the complete [actixsqlx example](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx) for:
- Service layer pattern
- PostgreSQL + SQLx integration
- CRUD operations with caching
- Docker Compose setup
- Production-ready error handling

### Quick Actix Example

```rust
use actix_web::{web, App, HttpResponse, HttpServer};
use cache_kit::{CacheExpander, strategy::CacheStrategy};
use cache_kit::backend::InMemoryBackend;
use std::sync::Arc;

struct AppState {
    cache: Arc<CacheExpander<InMemoryBackend>>,
    user_repo: Arc<UserRepository>,
}

async fn get_user(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let user_id = path.into_inner();

    let mut feeder = UserFeeder {
        id: user_id.clone(),
        user: None,
    };

    match data.cache.with(&mut feeder, &*data.user_repo, CacheStrategy::Refresh) {
        Ok(_) => match feeder.user {
            Some(user) => HttpResponse::Ok().json(user),
            None => HttpResponse::NotFound().json(serde_json::json!({
                "error": "User not found"
            })),
        },
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("{}", e)
        })),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cache = Arc::new(CacheExpander::new(InMemoryBackend::new()));
    let user_repo = Arc::new(UserRepository::new(/* db pool */));

    let app_state = web::Data::new(AppState { cache, user_repo });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/users/{id}", web::get().to(get_user))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
```

---

## gRPC with Tonic

gRPC services can use cache-kit for caching database entities before serializing to Protocol Buffers.

### Installation

```toml
[dependencies]
cache-kit = "0.9"
tonic = "0.12"
prost = "0.13"
tokio = { version = "1.41", features = ["full"] }
```

### gRPC Service Implementation

```rust
use tonic::{Request, Response, Status};
use cache_kit::{CacheExpander, strategy::CacheStrategy};
use cache_kit::backend::RedisBackend;
use std::sync::Arc;

// Generated from proto file
pub mod user_service {
    tonic::include_proto!("user");
}

use user_service::{UserRequest, UserResponse, user_service_server::UserService};

pub struct UserServiceImpl {
    cache: Arc<CacheExpander<RedisBackend>>,
    repo: Arc<UserRepository>,
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn get_user(
        &self,
        request: Request<UserRequest>,
    ) -> Result<Response<UserResponse>, Status> {
        let user_id = request.into_inner().id;

        let mut feeder = UserFeeder {
            id: user_id.clone(),
            user: None,
        };

        // Use cache-kit to fetch cached entity
        self.cache
            .with(&mut feeder, &*self.repo, CacheStrategy::Refresh)
            .map_err(|e| Status::internal(e.to_string()))?;

        match feeder.user {
            Some(user) => {
                // Convert domain entity to gRPC response
                let response = UserResponse {
                    id: user.id,
                    name: user.name,
                    email: user.email,
                };
                Ok(Response::new(response))
            }
            None => Err(Status::not_found("User not found")),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = Arc::new(CacheExpander::new(RedisBackend::new(/* config */)?));
    let repo = Arc::new(UserRepository::new(/* db pool */));

    let service = UserServiceImpl { cache, repo };

    tonic::transport::Server::builder()
        .add_service(user_service::user_service_server::UserServiceServer::new(service))
        .serve("0.0.0.0:50051".parse()?)
        .await?;

    Ok(())
}
```

---

## Background Workers

cache-kit works in background workers, cron jobs, and task queues.

### Example: Tokio Task

```rust
use cache_kit::{CacheExpander, strategy::CacheStrategy};
use cache_kit::backend::RedisBackend;
use std::sync::Arc;
use std::time::Duration;

async fn background_cache_warmer(
    cache: Arc<CacheExpander<RedisBackend>>,
    repo: Arc<UserRepository>,
) {
    loop {
        tokio::time::sleep(Duration::from_secs(300)).await;

        // Warm cache for popular users
        let popular_user_ids = vec!["user_001", "user_002", "user_003"];

        for user_id in popular_user_ids {
            let mut feeder = UserFeeder {
                id: user_id.to_string(),
                user: None,
            };

            if let Err(e) = cache.with(&mut feeder, &*repo, CacheStrategy::Refresh) {
                eprintln!("Cache warming error for {}: {}", user_id, e);
            }
        }

        println!("Cache warmed for popular users");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = Arc::new(CacheExpander::new(RedisBackend::new(/* config */)?));
    let repo = Arc::new(UserRepository::new(/* db pool */));

    // Spawn background task
    let cache_clone = Arc::clone(&cache);
    let repo_clone = Arc::clone(&repo);
    tokio::spawn(async move {
        background_cache_warmer(cache_clone, repo_clone).await;
    });

    // Your main application logic
    Ok(())
}
```

---

## Reusable Service Layer

Define business logic once, use across transports:

```rust
pub struct UserService {
    cache: Arc<CacheExpander<RedisBackend>>,
    repo: Arc<UserRepository>,
}

impl UserService {
    pub fn get_user(&self, id: &str) -> cache_kit::Result<Option<User>> {
        let mut feeder = UserFeeder {
            id: id.to_string(),
            user: None,
        };

        self.cache.with(&mut feeder, &*self.repo, CacheStrategy::Refresh)?;
        Ok(feeder.user)
    }

    pub async fn create_user(&self, user: User) -> cache_kit::Result<User> {
        // Create logic
    }

    pub async fn update_user(&self, user: User) -> cache_kit::Result<User> {
        // Update logic with cache invalidation
    }
}
```

Now use the same service across transports:

```rust
// REST (Axum)
async fn rest_get_user(
    State(service): State<Arc<UserService>>,
    Path(id): Path<String>,
) -> Result<Json<User>, StatusCode> {
    service.get_user(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

// gRPC (Tonic)
async fn grpc_get_user(
    &self,
    request: Request<UserRequest>,
) -> Result<Response<UserResponse>, Status> {
    let user = self.service.get_user(&request.into_inner().id)
        .map_err(|e| Status::internal(e.to_string()))?
        .ok_or_else(|| Status::not_found("User not found"))?;

    Ok(Response::new(to_grpc_response(user)))
}

// Background worker
async fn worker_task() {
    match service.get_user("user_001") {
        Ok(Some(user)) => process_user(user),
        _ => eprintln!("User not found"),
    }
}
```

---

## API Response Caching Example

Cache API responses (not just database entities):

```rust
use cache_kit::CacheEntity;
use serde::{Deserialize, Serialize};

// API response DTO
#[derive(Clone, Serialize, Deserialize)]
struct UserProfileResponse {
    id: String,
    name: String,
    email: String,
    followers_count: u64,
    posts_count: u64,
}

impl CacheEntity for UserProfileResponse {
    type Key = String;
    fn cache_key(&self) -> Self::Key { self.id.clone() }
    fn cache_prefix() -> &'static str { "user_profile_response" }
}

// Repository fetches from database and aggregates
impl DataRepository<UserProfileResponse> for UserProfileRepository {
    async fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<UserProfileResponse>> {
        // Aggregate from multiple tables
        let user = self.fetch_user(id).await?;
        let followers = self.count_followers(id).await?;
        let posts = self.count_posts(id).await?;

        Ok(Some(UserProfileResponse {
            id: user.id,
            name: user.name,
            email: user.email,
            followers_count: followers,
            posts_count: posts,
        }))
    }
}

// REST endpoint caches the full aggregated response
async fn get_profile(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<UserProfileResponse>, StatusCode> {
    let mut feeder = UserProfileFeeder {
        id: user_id,
        response: None,
    };

    state.cache
        .with(&mut feeder, &*state.profile_repo, CacheStrategy::Refresh)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match feeder.response {
        Some(response) => Ok(Json(response)),
        None => Err(StatusCode::NOT_FOUND),
    }
}
```

---

## Best Practices

### DO

✅ Keep cache logic in service layer
✅ Reuse services across transports
✅ Separate DTOs from domain entities
✅ Handle cache errors gracefully at API boundary

### DON'T

❌ Put cache calls directly in HTTP handlers
❌ Leak HTTP concepts into service layer
❌ Cache transport-specific data (headers, status codes)
❌ Mix serialization formats (use domain entities, not transport DTOs)

---

## Next Steps

- Learn about [Serialization formats](serialization)
- Explore [Cache backend options](backends)
- Review [Design principles](design-principles)
- See the [Actix + SQLx reference implementation](https://github.com/megamsys/cache-kit.rs/tree/main/examples/actixsqlx)
