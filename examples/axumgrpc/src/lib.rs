pub mod cache_config;
pub mod db;
pub mod grpc;
pub mod models;
pub mod repository;

pub use grpc::start_grpc_server;

use cache_kit::backend::InMemoryBackend;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub cache_backend: Arc<InMemoryBackend>,
}

impl AppState {
    pub fn new(db: sqlx::PgPool, cache_backend: Arc<InMemoryBackend>) -> Self {
        Self { db, cache_backend }
    }
}
