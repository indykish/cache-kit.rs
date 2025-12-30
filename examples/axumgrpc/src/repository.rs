use crate::db::Database;
use crate::models::Invoice;
use cache_kit::DataRepository;
use sqlx::PgPool;
use uuid::Uuid;

/// Invoice repository for cache-kit integration
#[allow(dead_code)]
pub struct InvoiceRepository {
    pool: PgPool,
}

impl InvoiceRepository {
    #[allow(dead_code)]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl DataRepository<Invoice> for InvoiceRepository {
    async fn fetch_by_id(&self, _id: &String) -> cache_kit::Result<Option<Invoice>> {
        // This would typically be called in an async context via block_on or tokio::spawn_blocking
        // For now, we'll define the async version separately
        Err(cache_kit::Error::BackendError(
            "Use fetch_by_id_async instead".to_string(),
        ))
    }
}

impl InvoiceRepository {
    #[allow(dead_code)]
    pub async fn fetch_by_id(&self, id: &Uuid) -> cache_kit::Result<Option<Invoice>> {
        Database::get_invoice(&self.pool, id)
            .await
            .map_err(|e| cache_kit::Error::BackendError(e.to_string()))
    }

    #[allow(dead_code)]
    pub async fn list_by_customer(
        &self,
        customer_id: &Uuid,
        limit: i64,
        offset: i64,
    ) -> cache_kit::Result<(Vec<Invoice>, i64)> {
        Database::list_invoices(&self.pool, customer_id, limit, offset)
            .await
            .map_err(|e| cache_kit::Error::BackendError(e.to_string()))
    }
}
