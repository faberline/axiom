//! Error types for cclab

use thiserror::Error;

/// Result type alias for cclab operations
pub type Result<T> = std::result::Result<T, DataBridgeError>;

/// Unified error type for all cclab operations
#[derive(Error, Debug, Clone)]
pub enum DataBridgeError {
    #[error("MongoDB error: {0}")]
    MongoDB(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Internal error: {0}")]
    Internal(String),

    // PostgreSQL-specific error types for better error handling
    /// Unique constraint violation (SQLSTATE 23505)
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Foreign key constraint violation (SQLSTATE 23503, 23502)
    #[error("Foreign key constraint violation: {0}")]
    ForeignKey(String),

    /// Deadlock detected (SQLSTATE 40P01) - retryable
    #[error("Deadlock detected: {0}")]
    Deadlock(String),

    /// Connection timeout - retryable
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Transient error that may succeed on retry
    #[error("Transient error: {0}")]
    Transient(String),
}

impl DataBridgeError {
    /// Returns true if this error is potentially retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            DataBridgeError::Deadlock(_)
                | DataBridgeError::Timeout(_)
                | DataBridgeError::Transient(_)
        )
    }

    /// Returns true if this is a constraint violation error
    pub fn is_constraint_violation(&self) -> bool {
        matches!(
            self,
            DataBridgeError::Conflict(_) | DataBridgeError::ForeignKey(_)
        )
    }
}

impl From<serde_json::Error> for DataBridgeError {
    fn from(err: serde_json::Error) -> Self {
        DataBridgeError::Serialization(err.to_string())
    }
}

// MongoDB-specific error conversions (when mongodb-errors feature is enabled)
#[cfg(feature = "mongodb-errors")]
impl From<mongodb::error::Error> for DataBridgeError {
    fn from(err: mongodb::error::Error) -> Self {
        DataBridgeError::MongoDB(err.to_string())
    }
}

#[cfg(feature = "mongodb-errors")]
impl From<bson::ser::Error> for DataBridgeError {
    fn from(err: bson::ser::Error) -> Self {
        DataBridgeError::Serialization(format!("BSON serialization error: {}", err))
    }
}

#[cfg(feature = "mongodb-errors")]
impl From<bson::de::Error> for DataBridgeError {
    fn from(err: bson::de::Error) -> Self {
        DataBridgeError::Serialization(format!("BSON deserialization error: {}", err))
    }
}

// PostgreSQL-specific error conversions (when postgres-errors feature is enabled)
#[cfg(feature = "postgres-errors")]
impl From<sqlx::Error> for DataBridgeError {
    fn from(err: sqlx::Error) -> Self {
        use sqlx::Error;
        match &err {
            Error::Configuration(_) => DataBridgeError::Connection(err.to_string()),
            Error::Database(db_err) => {
                // Classify based on PostgreSQL SQLSTATE codes
                // See: https://www.postgresql.org/docs/current/errcodes-appendix.html
                if let Some(code) = db_err.code() {
                    let code_str: &str = &code;
                    match code_str {
                        // Unique constraint violation
                        "23505" => return DataBridgeError::Conflict(err.to_string()),
                        // Foreign key violation
                        "23503" => return DataBridgeError::ForeignKey(err.to_string()),
                        // Not null violation
                        "23502" => return DataBridgeError::Validation(err.to_string()),
                        // Check constraint violation
                        "23514" => return DataBridgeError::Validation(err.to_string()),
                        // Exclusion constraint violation
                        "23P01" => return DataBridgeError::Conflict(err.to_string()),
                        // Deadlock detected
                        "40P01" => return DataBridgeError::Deadlock(err.to_string()),
                        // Serialization failure (can retry)
                        "40001" => return DataBridgeError::Transient(err.to_string()),
                        // Transaction rollback - deadlock or serialization
                        code if code.starts_with("40") => {
                            return DataBridgeError::Transient(err.to_string())
                        }
                        // Connection errors (class 08)
                        code if code.starts_with("08") => {
                            return DataBridgeError::Connection(err.to_string())
                        }
                        // Operator intervention / admin shutdown (class 57)
                        "57P01" | "57P02" | "57P03" => {
                            return DataBridgeError::Transient(err.to_string())
                        }
                        _ => {}
                    }
                }
                DataBridgeError::Database(err.to_string())
            }
            Error::Io(_) => DataBridgeError::Connection(err.to_string()),
            Error::Tls(_) => DataBridgeError::Connection(err.to_string()),
            Error::Protocol(_) => DataBridgeError::Connection(err.to_string()),
            Error::RowNotFound => DataBridgeError::Query("Row not found".to_string()),
            Error::TypeNotFound { .. } => DataBridgeError::Deserialization(err.to_string()),
            Error::ColumnIndexOutOfBounds { .. } => DataBridgeError::Query(err.to_string()),
            Error::ColumnNotFound(_) => DataBridgeError::Query(err.to_string()),
            Error::ColumnDecode { .. } => DataBridgeError::Deserialization(err.to_string()),
            Error::Decode(_) => DataBridgeError::Deserialization(err.to_string()),
            Error::PoolTimedOut => {
                DataBridgeError::Timeout("Connection pool timed out".to_string())
            }
            Error::PoolClosed => DataBridgeError::Connection("Connection pool closed".to_string()),
            Error::WorkerCrashed => DataBridgeError::Internal("Worker thread crashed".to_string()),
            _ => DataBridgeError::Database(err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_mongodb() {
        let err = DataBridgeError::MongoDB("connection refused".to_string());
        assert_eq!(err.to_string(), "MongoDB error: connection refused");
    }

    #[test]
    fn test_error_display_database() {
        let err = DataBridgeError::Database("invalid query".to_string());
        assert_eq!(err.to_string(), "Database error: invalid query");
    }

    #[test]
    fn test_error_display_serialization() {
        let err = DataBridgeError::Serialization("invalid JSON".to_string());
        assert_eq!(err.to_string(), "Serialization error: invalid JSON");
    }

    #[test]
    fn test_error_display_deserialization() {
        let err = DataBridgeError::Deserialization("missing field".to_string());
        assert_eq!(err.to_string(), "Deserialization error: missing field");
    }

    #[test]
    fn test_error_display_connection() {
        let err = DataBridgeError::Connection("timeout".to_string());
        assert_eq!(err.to_string(), "Connection error: timeout");
    }

    #[test]
    fn test_error_display_query() {
        let err = DataBridgeError::Query("invalid operator".to_string());
        assert_eq!(err.to_string(), "Query error: invalid operator");
    }

    #[test]
    fn test_error_display_validation() {
        let err = DataBridgeError::Validation("field required".to_string());
        assert_eq!(err.to_string(), "Validation error: field required");
    }

    #[test]
    fn test_error_display_internal() {
        let err = DataBridgeError::Internal("unexpected state".to_string());
        assert_eq!(err.to_string(), "Internal error: unexpected state");
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err: DataBridgeError = json_err.into();
        assert!(matches!(err, DataBridgeError::Serialization(_)));
    }

    #[test]
    #[allow(clippy::unnecessary_literal_unwrap)] // Testing Result type alias
    fn test_result_type_ok() {
        let result: Result<i32> = Ok(42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_result_type_err() {
        let result: Result<i32> = Err(DataBridgeError::Query("failed".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_error_display_conflict() {
        let err = DataBridgeError::Conflict("duplicate key value".to_string());
        assert_eq!(err.to_string(), "Conflict: duplicate key value");
    }

    #[test]
    fn test_error_display_foreign_key() {
        let err = DataBridgeError::ForeignKey("violates foreign key constraint".to_string());
        assert_eq!(
            err.to_string(),
            "Foreign key constraint violation: violates foreign key constraint"
        );
    }

    #[test]
    fn test_error_display_deadlock() {
        let err = DataBridgeError::Deadlock("deadlock detected".to_string());
        assert_eq!(err.to_string(), "Deadlock detected: deadlock detected");
    }

    #[test]
    fn test_error_display_timeout() {
        let err = DataBridgeError::Timeout("connection timed out".to_string());
        assert_eq!(err.to_string(), "Timeout: connection timed out");
    }

    #[test]
    fn test_error_display_transient() {
        let err = DataBridgeError::Transient("serialization failure".to_string());
        assert_eq!(err.to_string(), "Transient error: serialization failure");
    }

    #[test]
    fn test_is_retryable() {
        assert!(DataBridgeError::Deadlock("test".to_string()).is_retryable());
        assert!(DataBridgeError::Timeout("test".to_string()).is_retryable());
        assert!(DataBridgeError::Transient("test".to_string()).is_retryable());
        assert!(!DataBridgeError::Conflict("test".to_string()).is_retryable());
        assert!(!DataBridgeError::ForeignKey("test".to_string()).is_retryable());
        assert!(!DataBridgeError::Query("test".to_string()).is_retryable());
    }

    #[test]
    fn test_is_constraint_violation() {
        assert!(DataBridgeError::Conflict("test".to_string()).is_constraint_violation());
        assert!(DataBridgeError::ForeignKey("test".to_string()).is_constraint_violation());
        assert!(!DataBridgeError::Deadlock("test".to_string()).is_constraint_violation());
        assert!(!DataBridgeError::Query("test".to_string()).is_constraint_violation());
    }
}
