//! # cache-kit
//!
//! A type-safe, fully generic, production-ready caching framework for Rust.
//!
//! ## Features
//!
//! - **Fully Generic:** Cache any type `T` that implements `CacheEntity`
//! - **Backend Agnostic:** Support for in-memory, Redis, Memcached, and custom backends
//! - **Database Agnostic:** Works with SQLx, tokio-postgres, Diesel, or custom repositories
//! - **Framework Independent:** Zero dependencies on web frameworks (Axum, Actix, Rocket, etc.)
//! - **Production Ready:** Built-in logging, metrics support, and error handling
//! - **Type Safe:** Compile-time verified, no magic strings
//!
//! ## Quick Start
//!
//! ```ignore
//! use cache_kit::{
//!     CacheEntity, CacheFeed, DataRepository, CacheExpander,
//!     backend::InMemoryBackend,
//!     strategy::CacheStrategy,
//! };
//!
//! // 1. Define your entity
//! #[derive(Clone, Serialize, Deserialize)]
//! struct User {
//!     id: String,
//!     name: String,
//! }
//!
//! // 2. Implement CacheEntity
//! impl CacheEntity for User {
//!     type Key = String;
//!
//!     fn cache_key(&self) -> Self::Key {
//!         self.id.clone()
//!     }
//!
//!     fn cache_prefix() -> &'static str {
//!         "user"
//!     }
//! }
//!
//! // 3. Create feeder
//! struct UserFeeder {
//!     id: String,
//!     user: Option<User>,
//! }
//!
//! impl CacheFeed<User> for UserFeeder {
//!     fn entity_id(&mut self) -> String {
//!         self.id.clone()
//!     }
//!
//!     fn feed(&mut self, entity: Option<User>) {
//!         self.user = entity;
//!     }
//! }
//!
//! // 4. Use it
//! let backend = InMemoryBackend::new();
//! let mut expander = CacheExpander::new(backend);
//! ```

#[macro_use]
extern crate log;

pub mod backend;
pub mod builder;
pub mod entity;
pub mod error;
pub mod expander;
pub mod feed;
pub mod key;
pub mod observability;
pub mod repository;
pub mod serialization;
pub mod service;
pub mod strategy;

// Re-exports for convenience
pub use backend::CacheBackend;
pub use builder::CacheOperationBuilder;
pub use entity::CacheEntity;
pub use error::{Error, Result};
pub use expander::CacheExpander;
pub use feed::CacheFeed;
pub use repository::DataRepository;
pub use service::CacheService;
pub use strategy::CacheStrategy;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
