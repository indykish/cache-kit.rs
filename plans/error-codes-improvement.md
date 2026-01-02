# Error Codes Improvement Proposal

## Problem Statement

Currently, [`Error`](src/error.rs:13) variants only contain string messages, making it difficult for API consumers to:
1. **Programmatically handle** specific error conditions
2. **Internationalize** error messages
3. **Track and monitor** specific error types in production
4. **Document** error conditions in API specifications

## Current Implementation

```rust
pub enum Error {
    SerializationError(String),
    DeserializationError(String),
    ValidationError(String),
    CacheMiss,
    BackendError(String),
    RepositoryError(String),
    Timeout(String),
    ConfigError(String),
    NotImplemented(String),
    InvalidCacheEntry(String),
    VersionMismatch { expected: u32, found: u32 },
    Other(String),
}
```

**Issues:**
- No machine-readable error codes
- Hard to distinguish between different validation errors
- Difficult to create error catalogs for documentation
- No structured context (e.g., which key failed, which operation)

---

## Proposed Solution

### 1. Add Error Codes Enum

Create a new error code system that's both human-readable and machine-parseable:

```rust
// src/error.rs

/// Machine-readable error codes for programmatic error handling.
///
/// These codes are stable across versions and can be used for:
/// - API error responses
/// - Monitoring and alerting
/// - Error documentation
/// - Client-side error handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ErrorCode {
    // Serialization errors (1000-1099)
    SerializationFailed = 1000,
    PostcardEncodeFailed = 1001,
    
    // Deserialization errors (1100-1199)
    DeserializationFailed = 1100,
    PostcardDecodeFailed = 1101,
    InvalidMagicHeader = 1102,
    SchemaVersionMismatch = 1103,
    CorruptedPayload = 1104,
    
    // Validation errors (1200-1299)
    ValidationFailed = 1200,
    InvalidEntityId = 1201,
    InvalidCacheKey = 1202,
    InvalidUuid = 1203,
    EmptyField = 1204,
    FieldTooLong = 1205,
    
    // Cache errors (1300-1399)
    CacheMiss = 1300,
    CacheKeyNotFound = 1301,
    InvalidCacheEntry = 1302,
    
    // Backend errors (1400-1499)
    BackendUnavailable = 1400,
    RedisConnectionFailed = 1401,
    RedisOperationFailed = 1402,
    MemcachedConnectionFailed = 1403,
    MemcachedOperationFailed = 1404,
    BackendTimeout = 1405,
    
    // Repository errors (1500-1599)
    RepositoryFailed = 1500,
    DatabaseConnectionFailed = 1501,
    DatabaseQueryFailed = 1502,
    DatabaseTimeout = 1503,
    RecordNotFound = 1504,
    
    // Configuration errors (1600-1699)
    ConfigInvalid = 1600,
    InvalidConnectionString = 1601,
    InvalidTtlPolicy = 1602,
    MissingRequiredConfig = 1603,
    
    // Operation errors (1700-1799)
    OperationTimeout = 1700,
    OperationCancelled = 1701,
    RetryExhausted = 1702,
    
    // Feature errors (1800-1899)
    FeatureNotEnabled = 1800,
    FeatureNotImplemented = 1801,
    
    // Generic errors (1900-1999)
    Unknown = 1900,
    Internal = 1901,
}

impl ErrorCode {
    /// Get the error code as a u32.
    pub fn as_u32(self) -> u32 {
        self as u32
    }
    
    /// Get the error code as a string (e.g., "E1000").
    pub fn as_string(self) -> String {
        format!("E{:04}", self as u32)
    }
    
    /// Get a human-readable description of the error code.
    pub fn description(self) -> &'static str {
        match self {
            ErrorCode::SerializationFailed => "Failed to serialize entity for cache storage",
            ErrorCode::PostcardEncodeFailed => "Postcard encoding failed",
            ErrorCode::DeserializationFailed => "Failed to deserialize entity from cache",
            ErrorCode::PostcardDecodeFailed => "Postcard decoding failed",
            ErrorCode::InvalidMagicHeader => "Cache entry has invalid magic header",
            ErrorCode::SchemaVersionMismatch => "Cache entry schema version mismatch",
            ErrorCode::CorruptedPayload => "Cache entry payload is corrupted",
            ErrorCode::ValidationFailed => "Entity or feeder validation failed",
            ErrorCode::InvalidEntityId => "Entity ID is invalid",
            ErrorCode::InvalidCacheKey => "Cache key format is invalid",
            ErrorCode::InvalidUuid => "UUID format is invalid",
            ErrorCode::EmptyField => "Required field is empty",
            ErrorCode::FieldTooLong => "Field exceeds maximum length",
            ErrorCode::CacheMiss => "Cache key not found",
            ErrorCode::CacheKeyNotFound => "Requested cache key does not exist",
            ErrorCode::InvalidCacheEntry => "Cache entry is invalid or corrupted",
            ErrorCode::BackendUnavailable => "Cache backend is unavailable",
            ErrorCode::RedisConnectionFailed => "Failed to connect to Redis",
            ErrorCode::RedisOperationFailed => "Redis operation failed",
            ErrorCode::MemcachedConnectionFailed => "Failed to connect to Memcached",
            ErrorCode::MemcachedOperationFailed => "Memcached operation failed",
            ErrorCode::BackendTimeout => "Cache backend operation timed out",
            ErrorCode::RepositoryFailed => "Data repository operation failed",
            ErrorCode::DatabaseConnectionFailed => "Failed to connect to database",
            ErrorCode::DatabaseQueryFailed => "Database query failed",
            ErrorCode::DatabaseTimeout => "Database operation timed out",
            ErrorCode::RecordNotFound => "Database record not found",
            ErrorCode::ConfigInvalid => "Configuration is invalid",
            ErrorCode::InvalidConnectionString => "Connection string format is invalid",
            ErrorCode::InvalidTtlPolicy => "TTL policy configuration is invalid",
            ErrorCode::MissingRequiredConfig => "Required configuration is missing",
            ErrorCode::OperationTimeout => "Operation exceeded timeout threshold",
            ErrorCode::OperationCancelled => "Operation was cancelled",
            ErrorCode::RetryExhausted => "Maximum retry attempts exhausted",
            ErrorCode::FeatureNotEnabled => "Required feature is not enabled",
            ErrorCode::FeatureNotImplemented => "Feature is not implemented",
            ErrorCode::Unknown => "Unknown error occurred",
            ErrorCode::Internal => "Internal error occurred",
        }
    }
    
    /// Check if this error is retryable.
    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            ErrorCode::BackendTimeout
                | ErrorCode::BackendUnavailable
                | ErrorCode::RedisConnectionFailed
                | ErrorCode::MemcachedConnectionFailed
                | ErrorCode::DatabaseConnectionFailed
                | ErrorCode::DatabaseTimeout
                | ErrorCode::OperationTimeout
        )
    }
    
    /// Get the HTTP status code that should be returned for this error.
    pub fn http_status(self) -> u16 {
        match self {
            // 400 Bad Request
            ErrorCode::ValidationFailed
            | ErrorCode::InvalidEntityId
            | ErrorCode::InvalidCacheKey
            | ErrorCode::InvalidUuid
            | ErrorCode::EmptyField
            | ErrorCode::FieldTooLong
            | ErrorCode::InvalidCacheEntry
            | ErrorCode::ConfigInvalid
            | ErrorCode::InvalidConnectionString
            | ErrorCode::InvalidTtlPolicy
            | ErrorCode::MissingRequiredConfig => 400,
            
            // 404 Not Found
            ErrorCode::CacheMiss
            | ErrorCode::CacheKeyNotFound
            | ErrorCode::RecordNotFound => 404,
            
            // 408 Request Timeout
            ErrorCode::OperationTimeout
            | ErrorCode::BackendTimeout
            | ErrorCode::DatabaseTimeout => 408,
            
            // 500 Internal Server Error
            ErrorCode::SerializationFailed
            | ErrorCode::PostcardEncodeFailed
            | ErrorCode::DeserializationFailed
            | ErrorCode::PostcardDecodeFailed
            | ErrorCode::CorruptedPayload
            | ErrorCode::Internal
            | ErrorCode::Unknown => 500,
            
            // 501 Not Implemented
            ErrorCode::FeatureNotImplemented => 501,
            
            // 503 Service Unavailable
            ErrorCode::BackendUnavailable
            | ErrorCode::RedisConnectionFailed
            | ErrorCode::RedisOperationFailed
            | ErrorCode::MemcachedConnectionFailed
            | ErrorCode::MemcachedOperationFailed
            | ErrorCode::RepositoryFailed
            | ErrorCode::DatabaseConnectionFailed
            | ErrorCode::DatabaseQueryFailed
            | ErrorCode::RetryExhausted => 503,
            
            // 422 Unprocessable Entity
            ErrorCode::SchemaVersionMismatch => 422,
            
            // 424 Failed Dependency
            ErrorCode::FeatureNotEnabled => 424,
            
            // 499 Client Closed Request
            ErrorCode::OperationCancelled => 499,
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_string())
    }
}
```

### 2. Add Structured Error Context

```rust
/// Structured context for errors.
///
/// Provides additional machine-readable information about the error.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorContext {
    /// The cache key involved in the operation (if applicable)
    pub cache_key: Option<String>,
    
    /// The entity type involved (if applicable)
    pub entity_type: Option<String>,
    
    /// The operation that was being performed
    pub operation: Option<String>,
    
    /// Additional key-value metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl ErrorContext {
    pub fn new() -> Self {
        Self {
            cache_key: None,
            entity_type: None,
            operation: None,
            metadata: std::collections::HashMap::new(),
        }
    }
    
    pub fn with_cache_key(mut self, key: impl Into<String>) -> Self {
        self.cache_key = Some(key.into());
        self
    }
    
    pub fn with_entity_type(mut self, entity_type: impl Into<String>) -> Self {
        self.entity_type = Some(entity_type.into());
        self
    }
    
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }
    
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new()
    }
}
```

### 3. Update Error Enum

```rust
/// Error types for cache framework with error codes and context.
#[derive(Debug, Clone)]
pub enum Error {
    /// Serialization failed when converting entity to cache bytes.
    SerializationError {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },

    /// Deserialization failed when converting cache bytes to entity.
    DeserializationError {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },

    /// Validation failed in feeder or entity.
    ValidationError {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },

    /// Cache miss: key not found in cache.
    CacheMiss {
        code: ErrorCode,
        context: ErrorContext,
    },

    /// Backend storage error (Redis, Memcached, etc).
    BackendError {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },

    /// Data repository error (database, etc).
    RepositoryError {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },

    /// Operation exceeded configured timeout threshold.
    Timeout {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },

    /// Configuration error during crate initialization.
    ConfigError {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },

    /// Feature not implemented or not enabled.
    NotImplemented {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },

    /// Invalid cache entry: corrupted envelope or bad magic.
    InvalidCacheEntry {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },

    /// Schema version mismatch between code and cached data.
    VersionMismatch {
        code: ErrorCode,
        expected: u32,
        found: u32,
        context: ErrorContext,
    },

    /// Generic error with custom message.
    Other {
        code: ErrorCode,
        message: String,
        context: ErrorContext,
    },
}

impl Error {
    /// Get the error code.
    pub fn code(&self) -> ErrorCode {
        match self {
            Error::SerializationError { code, .. } => *code,
            Error::DeserializationError { code, .. } => *code,
            Error::ValidationError { code, .. } => *code,
            Error::CacheMiss { code, .. } => *code,
            Error::BackendError { code, .. } => *code,
            Error::RepositoryError { code, .. } => *code,
            Error::Timeout { code, .. } => *code,
            Error::ConfigError { code, .. } => *code,
            Error::NotImplemented { code, .. } => *code,
            Error::InvalidCacheEntry { code, .. } => *code,
            Error::VersionMismatch { code, .. } => *code,
            Error::Other { code, .. } => *code,
        }
    }
    
    /// Get the error context.
    pub fn context(&self) -> &ErrorContext {
        match self {
            Error::SerializationError { context, .. } => context,
            Error::DeserializationError { context, .. } => context,
            Error::ValidationError { context, .. } => context,
            Error::CacheMiss { context, .. } => context,
            Error::BackendError { context, .. } => context,
            Error::RepositoryError { context, .. } => context,
            Error::Timeout { context, .. } => context,
            Error::ConfigError { context, .. } => context,
            Error::NotImplemented { context, .. } => context,
            Error::InvalidCacheEntry { context, .. } => context,
            Error::VersionMismatch { context, .. } => context,
            Error::Other { context, .. } => context,
        }
    }
    
    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        self.code().is_retryable()
    }
    
    /// Get the HTTP status code for this error.
    pub fn http_status(&self) -> u16 {
        self.code().http_status()
    }
    
    /// Convert to a JSON-serializable error response.
    #[cfg(feature = "serde")]
    pub fn to_error_response(&self) -> ErrorResponse {
        ErrorResponse {
            code: self.code().as_string(),
            message: self.to_string(),
            details: self.context().clone(),
            retryable: self.is_retryable(),
        }
    }
}

/// JSON-serializable error response for APIs.
#[cfg(feature = "serde")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorResponse {
    /// Error code (e.g., "E1000")
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Structured error context
    pub details: ErrorContext,
    /// Whether the operation can be retried
    pub retryable: bool,
}
```

### 4. Add Builder Methods for Errors

```rust
impl Error {
    /// Create a serialization error with context.
    pub fn serialization(message: impl Into<String>) -> Self {
        Error::SerializationError {
            code: ErrorCode::SerializationFailed,
            message: message.into(),
            context: ErrorContext::new(),
        }
    }
    
    /// Create a deserialization error with context.
    pub fn deserialization(message: impl Into<String>) -> Self {
        Error::DeserializationError {
            code: ErrorCode::DeserializationFailed,
            message: message.into(),
            context: ErrorContext::new(),
        }
    }
    
    /// Create a validation error with context.
    pub fn validation(message: impl Into<String>) -> Self {
        Error::ValidationError {
            code: ErrorCode::ValidationFailed,
            message: message.into(),
            context: ErrorContext::new(),
        }
    }
    
    /// Create an invalid UUID error.
    pub fn invalid_uuid(uuid: impl Into<String>) -> Self {
        Error::ValidationError {
            code: ErrorCode::InvalidUuid,
            message: format!("Invalid UUID format: {}", uuid.into()),
            context: ErrorContext::new(),
        }
    }
    
    /// Create a cache miss error.
    pub fn cache_miss(key: impl Into<String>) -> Self {
        Error::CacheMiss {
            code: ErrorCode::CacheMiss,
            context: ErrorContext::new().with_cache_key(key),
        }
    }
    
    /// Create a backend error.
    pub fn backend(code: ErrorCode, message: impl Into<String>) -> Self {
        Error::BackendError {
            code,
            message: message.into(),
            context: ErrorContext::new(),
        }
    }
    
    /// Add context to an existing error.
    pub fn with_context(mut self, context: ErrorContext) -> Self {
        match &mut self {
            Error::SerializationError { context: c, .. } => *c = context,
            Error::DeserializationError { context: c, .. } => *c = context,
            Error::ValidationError { context: c, .. } => *c = context,
            Error::CacheMiss { context: c, .. } => *c = context,
            Error::BackendError { context: c, .. } => *c = context,
            Error::RepositoryError { context: c, .. } => *c = context,
            Error::Timeout { context: c, .. } => *c = context,
            Error::ConfigError { context: c, .. } => *c = context,
            Error::NotImplemented { context: c, .. } => *c = context,
            Error::InvalidCacheEntry { context: c, .. } => *c = context,
            Error::VersionMismatch { context: c, .. } => *c = context,
            Error::Other { context: c, .. } => *c = context,
        }
        self
    }
    
    /// Add cache key to error context.
    pub fn with_cache_key(self, key: impl Into<String>) -> Self {
        let mut context = self.context().clone();
        context.cache_key = Some(key.into());
        self.with_context(context)
    }
    
    /// Add entity type to error context.
    pub fn with_entity_type(self, entity_type: impl Into<String>) -> Self {
        let mut context = self.context().clone();
        context.entity_type = Some(entity_type.into());
        self.with_context(context)
    }
    
    /// Add operation to error context.
    pub fn with_operation(self, operation: impl Into<String>) -> Self {
        let mut context = self.context().clone();
        context.operation = Some(operation.into());
        self.with_context(context)
    }
}
```

---

## Usage Examples

### Example 1: Creating Errors with Context

```rust
// Before
return Err(Error::ValidationError("Invalid UUID".to_string()));

// After
return Err(Error::invalid_uuid(id)
    .with_cache_key(format!("user:{}", id))
    .with_operation("get_user"));
```

### Example 2: Handling Errors in API Layer

```rust
// In Actix handler
async fn get_user(id: web::Path<String>, service: web::Data<UserService>) -> Result<HttpResponse> {
    match service.get(&id).await {
        Ok(Some(user)) => Ok(HttpResponse::Ok().json(user)),
        Ok(None) => Ok(HttpResponse::NotFound().finish()),
        Err(e) => {
            // Use error code for HTTP status
            let status = StatusCode::from_u16(e.http_status()).unwrap();
            
            // Return structured error response
            Ok(HttpResponse::build(status).json(e.to_error_response()))
        }
    }
}
```

### Example 3: Monitoring and Alerting

```rust
// In observability layer
impl CacheMetrics for PrometheusMetrics {
    fn record_error(&self, key: &str, error: &str) {
        // Parse error to get code
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(error) {
            self.error_counter
                .with_label_values(&[&err.code, key])
                .inc();
            
            // Alert on specific error codes
            if matches!(err.code.as_str(), "E1400" | "E1500") {
                self.alert_manager.send_alert(&err);
            }
        }
    }
}
```

### Example 4: Client-Side Error Handling

```rust
// In client code
match cache.execute(&mut feeder, &repo, CacheStrategy::Refresh).await {
    Ok(_) => println!("Success"),
    Err(e) => {
        match e.code() {
            ErrorCode::BackendUnavailable | ErrorCode::DatabaseConnectionFailed => {
                // Retry with exponential backoff
                retry_with_backoff(operation).await?;
            }
            ErrorCode::InvalidUuid | ErrorCode::ValidationFailed => {
                // Don't retry, fix the input
                log::error!("Invalid input: {}", e);
                return Err(e);
            }
            ErrorCode::SchemaVersionMismatch => {
                // Cache will be automatically refreshed
                log::warn!("Schema version mismatch, cache will refresh");
            }
            _ => {
                log::error!("Unexpected error: {}", e);
                return Err(e);
            }
        }
    }
}
```

---

## Migration Strategy

### Phase 1: Add New Types (Non-Breaking)

1. Add `ErrorCode` enum
2. Add `ErrorContext` struct
3. Add `ErrorResponse` struct
4. Keep existing `Error` enum unchanged

### Phase 2: Deprecate Old Constructors

1. Add `#[deprecated]` to old error constructors
2. Add new constructors with error codes
3. Update internal code to use new constructors
4. Update examples

### Phase 3: Breaking Change (v1.0)

1. Update `Error` enum to include codes and context
2. Remove deprecated constructors
3. Update all error creation sites
4. Update documentation

---

## Benefits

1. **Programmatic Error Handling**
   - Clients can handle specific error codes
   - No string parsing needed

2. **Better Monitoring**
   - Track error rates by code
   - Alert on specific error conditions
   - Identify patterns in production

3. **Improved Documentation**
   - Error catalog with all codes
   - Clear error handling guidelines
   - API documentation includes error codes

4. **Internationalization**
   - Error codes are language-independent
   - Messages can be translated based on code

5. **Debugging**
   - Structured context helps identify issues
   - Cache key, entity type, operation included
   - Easier to reproduce issues

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_code_display() {
        assert_eq!(ErrorCode::SerializationFailed.as_string(), "E1000");
        assert_eq!(ErrorCode::InvalidUuid.as_string(), "E1203");
    }
    
    #[test]
    fn test_error_code_retryable() {
        assert!(ErrorCode::BackendTimeout.is_retryable());
        assert!(!ErrorCode::ValidationFailed.is_retryable());
    }
    
    #[test]
    fn test_error_code_http_status() {
        assert_eq!(ErrorCode::ValidationFailed.http_status(), 400);
        assert_eq!(ErrorCode::CacheMiss.http_status(), 404);
        assert_eq!(ErrorCode::BackendUnavailable.http_status(), 503);
    }
    
    #[test]
    fn test_error_with_context() {
        let err = Error::invalid_uuid("not-a-uuid")
            .with_cache_key("user:123")
            .with_operation("get_user");
        
        assert_eq!(err.code(), ErrorCode::InvalidUuid);
        assert_eq!(err.context().cache_key, Some("user:123".to_string()));
        assert_eq!(err.context().operation, Some("get_user".to_string()));
    }
    
    #[test]
    fn test_error_response_serialization() {
        let err = Error::validation("Field is required")
            .with_cache_key("user:123");
        
        let response = err.to_error_response();
        assert_eq!(response.code, "E1200");
        assert!(!response.retryable);
    }
}
```

---

## Documentation Updates

### Error Catalog

Create a new documentation page: `docs/_pages/error-catalog.md`

```markdown
# Error Catalog

## Error Code Ranges

- **1000-1099**: Serialization errors
- **1100-1199**: Deserialization errors
- **1200-1299**: Validation errors
- **1300-1399**: Cache errors
- **1400-1499**: Backend errors
- **1500-1599**: Repository errors
- **1600-1699**: Configuration errors
- **1700-1799**: Operation errors
- **1800-1899**: Feature errors
- **1900-1999**: Generic errors

## Error Codes

### E1000: Serialization Failed
**Category:** Serialization  
**HTTP Status:** 500  
**Retryable:** No

**Description:** Failed to serialize entity for cache storage.

**Common Causes:**
- Entity contains non-serializable types
- Postcard codec error

**Resolution:**
- Check entity implements `Serialize` correctly
- Verify all fields are serializable

### E1203: Invalid UUID
**Category:** Validation  
**HTTP Status:** 400  
**Retryable:** No

**Description:** UUID format is invalid.

**Common Causes:**
- Malformed UUID string
- Wrong UUID version

**Resolution:**
- Validate UUID format before calling API
- Use proper UUID generation

[... continue for all error codes ...]
```

---

## Implementation Checklist

- [ ] Add `ErrorCode` enum with all codes
- [ ] Add `ErrorContext` struct
- [ ] Add `ErrorResponse` struct
- [ ] Update `Error` enum with codes and context
- [ ] Add builder methods for errors
- [ ] Update all error creation sites in codebase
- [ ] Add tests for error codes
- [ ] Update documentation with error catalog
- [ ] Update examples to use new error handling
- [ ] Add migration guide for users

---

## Conclusion

This proposal adds structured error codes and context to cache-kit, making it more production-ready and easier to integrate into larger systems. The error codes are:

- **Stable** - Won't change across versions
- **Documented** - Clear catalog of all codes
- **Actionable** - Include retry hints and HTTP status
- **Structured** - Machine-readable context

This is a common pattern in production systems (AWS, Google Cloud, Stripe, etc.) and will significantly improve the developer experience.
