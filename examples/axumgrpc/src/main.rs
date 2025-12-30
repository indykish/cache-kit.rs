use axum::{extract::State, http::StatusCode, routing::get, Router};
use axumgrpc::AppState;
use cache_kit::backend::InMemoryBackend;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    let _ = {
        use tracing_subscriber::fmt;
        fmt().try_init()
    };

    // Database setup
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/cache_kit".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    tracing::info!("Migrations completed");

    // Cache backend
    let cache_backend = Arc::new(InMemoryBackend::new());

    let state = AppState {
        db: pool,
        cache_backend,
    };

    // REST health check
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(state.clone());

    // Start gRPC server
    let grpc_handle = {
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = axumgrpc::start_grpc_server(state).await {
                tracing::error!("gRPC server error: {}", e);
            }
        })
    };

    // Start REST server
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    tracing::info!("REST server listening on http://127.0.0.1:3000");

    tokio::select! {
        _ = axum::serve(listener, app) => {
            tracing::info!("REST server stopped");
        }
        _ = grpc_handle => {
            tracing::info!("gRPC server stopped");
        }
    }

    Ok(())
}

async fn health_check(State(_state): State<AppState>) -> (StatusCode, String) {
    (StatusCode::OK, "OK".to_string())
}
