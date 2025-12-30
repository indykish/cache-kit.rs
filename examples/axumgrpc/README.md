# axumgrpc: Invoice API with cache-kit

A realistic gRPC + REST example using **Axum**, **Tonic**, **SQLx**, **PostgreSQL**, and **cache-kit**.

This example demonstrates:
- ✅ Multi-table relationships (Customer → Invoice → LineItems)
- ✅ Async database access with SQLx and migrations
- ✅ gRPC service definitions and handlers
- ✅ Cache-kit integration for invoice fetching
- ✅ Postcard serialization with monetary types (using `i64` cents instead of `Decimal`)
- ✅ REST health check endpoint

## Prerequisites

- Rust 1.75+
- PostgreSQL 12+
- Docker (optional, for running Postgres)
- **protoc** (required for gRPC proto compilation)
  - macOS: `brew install protobuf`
  - Linux: `apt-get install protobuf-compiler` (Ubuntu/Debian) or equivalent
  - Or download from: https://github.com/protocolbuffers/protobuf/releases

## Setup

### 1. Start PostgreSQL

Using Docker:
```bash
docker run --name cache-kit-postgres \
  -e POSTGRES_DB=cache_kit \
  -e POSTGRES_PASSWORD=password \
  -p 5432:5432 \
  -d postgres:15
```

Or with your local Postgres:
```bash
createdb cache_kit
```

### 2. Configure Database URL

```bash
export DATABASE_URL="postgres://postgres:password@localhost:5432/cache_kit"
```

### 3. Install Dependencies

```bash
cargo build
```

### 4. Run Migrations

Migrations run automatically on startup, but you can verify them:
```bash
cargo sqlx migrate run
```

## Running the Server

```bash
cargo run --example axumgrpc
```

You should see:
```
REST server listening on http://127.0.0.1:3000
gRPC server listening on 127.0.0.1:50051
```

## REST API

### Health Check
```bash
curl http://127.0.0.1:3000/health
# Output: OK
```

## gRPC Server

The server runs on port `50051` alongside the REST health check on port `3000`.

Use `grpcurl` to test the gRPC service:

```bash
# Install grpcurl if you don't have it
cargo install grpcurl

# Get an invoice (you'll need real IDs from the database)
grpcurl -plaintext \
  -d '{"invoice_id": "550e8400-e29b-41d4-a716-446655440000"}' \
  127.0.0.1:50051 invoices.InvoicesService/GetInvoice

# List invoices for a customer
grpcurl -plaintext \
  -d '{"customer_id": "550e8400-e29b-41d4-a716-446655440000", "limit": 10, "offset": 0}' \
  127.0.0.1:50051 invoices.InvoicesService/ListInvoices
```

## Database Schema

### Customers
```sql
id (UUID)
name (VARCHAR)
email (VARCHAR, UNIQUE)
created_at (TIMESTAMP)
```

### Invoices
```sql
id (UUID)
customer_id (UUID, FK)
invoice_number (VARCHAR, UNIQUE)
amount_cents (BIGINT)  -- Use i64 for Postcard compatibility
status (VARCHAR)
issued_at (TIMESTAMP)
due_at (TIMESTAMP)
created_at (TIMESTAMP)
updated_at (TIMESTAMP)
```

### InvoiceLineItems
```sql
id (UUID)
invoice_id (UUID, FK, CASCADE)
description (VARCHAR)
quantity (INTEGER)
unit_price_cents (BIGINT)  -- Use i64 for Postcard compatibility
created_at (TIMESTAMP)
```

## Key Design Decisions

### Decimal vs i64 for Money

**Problem:** Postcard does not support `rust_decimal::Decimal` natively. This example uses `i64` for cents instead.

**Solution:** Store all monetary values as `i64` (cents):
```rust
// Instead of: Decimal::from_str("99.99")
// Use: 9999i64 (representing $99.99)
```

This approach:
- ✅ Works with Postcard serialization
- ✅ Avoids floating-point precision issues
- ✅ Is widely used in financial systems
- ✅ Makes rounding and calculations explicit

### Async Database Access

Cache-kit's `DataRepository` trait is traditionally sync-oriented. This example provides both:

```rust
// Standard trait implementation (returns error for now)
impl DataRepository<Invoice> for InvoiceRepository {
    fn fetch_by_id(&self, id: &String) -> cache_kit::Result<Option<Invoice>> {
        Err(cache_kit::Error::BackendError("Use async version".into()))
    }
}

// Async methods for actual queries
impl InvoiceRepository {
    pub async fn fetch_by_id_async(&self, id: &Uuid) -> cache_kit::Result<Option<Invoice>> {
        // Async database query
    }
}
```

In production, you'd use `tokio::task::block_in_place` or `spawn_blocking` for sync-to-async bridging if needed.

### Cache Strategy

The example integrates cache-kit for:
- Single invoice caching by ID
- List caching by customer ID (with pagination)
- Invalidation on status updates

See `src/cache_config.rs` for cache key generation.

## File Structure

```
src/
├── main.rs           # Server setup, health check
├── db.rs             # Database queries via SQLx
├── models.rs         # Invoice, Customer, LineItem types
├── cache_config.rs   # CacheEntity implementations
├── repository.rs     # DataRepository for cache-kit
└── grpc.rs           # gRPC service handler

proto/
└── invoices.proto    # gRPC service definitions

migrations/
├── 001_init_schema.sql
└── 002_add_indexes.sql
```

## Testing

Create a test customer and invoice:
```bash
psql $DATABASE_URL << EOF
INSERT INTO customers (id, name, email) VALUES
  ('550e8400-e29b-41d4-a716-446655440000', 'Acme Corp', 'contact@acme.com');

INSERT INTO invoices (id, customer_id, invoice_number, amount_cents, status) VALUES
  ('550e8400-e29b-41d4-a716-446655440001', '550e8400-e29b-41d4-a716-446655440000', 'INV-001', 50000, 'draft');

INSERT INTO invoice_line_items (id, invoice_id, description, quantity, unit_price_cents) VALUES
  ('550e8400-e29b-41d4-a716-446655440002', '550e8400-e29b-41d4-a716-446655440001', 'Product A', 5, 10000);
EOF
```

Then query:
```bash
grpcurl -plaintext \
  -d '{"invoice_id": "550e8400-e29b-41d4-a716-446655440001"}' \
  127.0.0.1:50051 invoices.InvoicesService/GetInvoice
```

## Next Steps

- Add cache-kit caching for frequently accessed invoices
- Implement Redis backend for distributed caching
- Add request validation and error handling
- Write integration tests
- Add OpenTelemetry instrumentation

## License

MIT
