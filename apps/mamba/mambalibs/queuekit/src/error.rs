//! Task-specific error types

use mamba_core::DataBridgeError;
use std::time::Duration;
use thiserror::Error;

/// Task-specific error types
#[derive(Error, Debug)]
pub enum TaskError {
    #[error("Broker error: {0}")]
    Broker(String),

    #[error("Backend error: {0}")]
    Backend(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Invalid task ID: {0}")]
    InvalidTaskId(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Task revoked: {0}")]
    Revoked(String),

    #[error("Max retries exceeded: {0}")]
    MaxRetriesExceeded(String),

    #[error("Invalid workflow: {0}")]
    InvalidWorkflow(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Rate limited, retry after {0:?}")]
    RateLimited(Duration),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<TaskError> for DataBridgeError {
    fn from(err: TaskError) -> Self {
        match err {
            TaskError::Broker(s) => DataBridgeError::Connection(s),
            TaskError::Backend(s) => DataBridgeError::Connection(s),
            TaskError::Connection(s) => DataBridgeError::Connection(s),
            TaskError::Serialization(s) => DataBridgeError::Serialization(s),
            TaskError::Deserialization(s) => DataBridgeError::Deserialization(s),
            TaskError::Configuration(s) => DataBridgeError::Internal(s),
            TaskError::NotConnected => DataBridgeError::Connection("Not connected".to_string()),
            _ => DataBridgeError::Internal(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for TaskError {
    fn from(err: serde_json::Error) -> Self {
        TaskError::Serialization(err.to_string())
    }
}

impl From<uuid::Error> for TaskError {
    fn from(err: uuid::Error) -> Self {
        TaskError::InvalidTaskId(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // T1: Display — Broker
    #[test]
    fn display_broker() {
        assert_eq!(
            TaskError::Broker("conn refused".into()).to_string(),
            "Broker error: conn refused"
        );
    }

    // T2: Display — Backend
    #[test]
    fn display_backend() {
        let msg = TaskError::Backend("pool exhausted".into()).to_string();
        assert!(msg.starts_with("Backend error: "));
    }

    // T3: Display — Connection
    #[test]
    fn display_connection() {
        let msg = TaskError::Connection("timeout".into()).to_string();
        assert!(msg.starts_with("Connection error: "));
    }

    // T4: Display — TaskNotFound
    #[test]
    fn display_task_not_found() {
        let msg = TaskError::TaskNotFound("abc-123".into()).to_string();
        assert!(msg.starts_with("Task not found: "));
    }

    // T5: Display — InvalidTaskId
    #[test]
    fn display_invalid_task_id() {
        let msg = TaskError::InvalidTaskId("not-a-uuid".into()).to_string();
        assert!(msg.starts_with("Invalid task ID: "));
    }

    // T6: Display — Serialization
    #[test]
    fn display_serialization() {
        let msg = TaskError::Serialization("bad json".into()).to_string();
        assert!(msg.starts_with("Serialization error: "));
    }

    // T7: Display — Deserialization
    #[test]
    fn display_deserialization() {
        let msg = TaskError::Deserialization("missing field".into()).to_string();
        assert!(msg.starts_with("Deserialization error: "));
    }

    // T8: Display — Timeout
    #[test]
    fn display_timeout() {
        let msg = TaskError::Timeout("30s elapsed".into()).to_string();
        assert!(msg.starts_with("Timeout: "));
    }

    // T9: Display — Revoked
    #[test]
    fn display_revoked() {
        let msg = TaskError::Revoked("user requested".into()).to_string();
        assert!(msg.starts_with("Task revoked: "));
    }

    // T10: Display — MaxRetriesExceeded
    #[test]
    fn display_max_retries() {
        let msg = TaskError::MaxRetriesExceeded("3 attempts".into()).to_string();
        assert!(msg.starts_with("Max retries exceeded: "));
    }

    // T11: Display — InvalidWorkflow
    #[test]
    fn display_invalid_workflow() {
        let msg = TaskError::InvalidWorkflow("cycle detected".into()).to_string();
        assert!(msg.starts_with("Invalid workflow: "));
    }

    // T12: Display — Configuration
    #[test]
    fn display_configuration() {
        let msg = TaskError::Configuration("missing key".into()).to_string();
        assert!(msg.starts_with("Configuration error: "));
    }

    // T13: Display — Authentication
    #[test]
    fn display_authentication() {
        let msg = TaskError::Authentication("invalid token".into()).to_string();
        assert!(msg.starts_with("Authentication error: "));
    }

    // T14: Display — AlreadyExists
    #[test]
    fn display_already_exists() {
        let msg = TaskError::AlreadyExists("task-xyz".into()).to_string();
        assert!(msg.starts_with("Already exists: "));
    }

    // T15: Display — NotConnected
    #[test]
    fn display_not_connected() {
        assert_eq!(TaskError::NotConnected.to_string(), "Not connected");
    }

    // T16: Display — RateLimited
    #[test]
    fn display_rate_limited() {
        let msg = TaskError::RateLimited(Duration::from_secs(5)).to_string();
        assert!(msg.contains("5s"), "expected '5s' in: {msg}");
    }

    // T17: Display — Internal
    #[test]
    fn display_internal() {
        let msg = TaskError::Internal("unexpected".into()).to_string();
        assert!(msg.starts_with("Internal error: "));
    }

    // T18: From<TaskError> for DataBridgeError — Broker → Connection
    #[test]
    fn from_broker_to_databridge() {
        let err: DataBridgeError = TaskError::Broker("fail".into()).into();
        assert!(matches!(err, DataBridgeError::Connection(s) if s == "fail"));
    }

    // T19: From<TaskError> for DataBridgeError — Backend → Connection
    #[test]
    fn from_backend_to_databridge() {
        let err: DataBridgeError = TaskError::Backend("fail".into()).into();
        assert!(matches!(err, DataBridgeError::Connection(s) if s == "fail"));
    }

    // T20: From<TaskError> for DataBridgeError — Connection → Connection
    #[test]
    fn from_connection_to_databridge() {
        let err: DataBridgeError = TaskError::Connection("fail".into()).into();
        assert!(matches!(err, DataBridgeError::Connection(s) if s == "fail"));
    }

    // T21: From<TaskError> for DataBridgeError — Serialization → Serialization
    #[test]
    fn from_serialization_to_databridge() {
        let err: DataBridgeError = TaskError::Serialization("bad".into()).into();
        assert!(matches!(err, DataBridgeError::Serialization(s) if s == "bad"));
    }

    // T22: From<TaskError> for DataBridgeError — Deserialization → Deserialization
    #[test]
    fn from_deserialization_to_databridge() {
        let err: DataBridgeError = TaskError::Deserialization("bad".into()).into();
        assert!(matches!(err, DataBridgeError::Deserialization(s) if s == "bad"));
    }

    // T23: From<TaskError> for DataBridgeError — Configuration → Internal
    #[test]
    fn from_configuration_to_databridge() {
        let err: DataBridgeError = TaskError::Configuration("cfg".into()).into();
        assert!(matches!(err, DataBridgeError::Internal(s) if s == "cfg"));
    }

    // T24: From<TaskError> for DataBridgeError — NotConnected → Connection("Not connected")
    #[test]
    fn from_not_connected_to_databridge() {
        let err: DataBridgeError = TaskError::NotConnected.into();
        assert!(matches!(err, DataBridgeError::Connection(s) if s == "Not connected"));
    }

    // T25: From<TaskError> for DataBridgeError — catch-all TaskNotFound → Internal
    #[test]
    fn from_wildcard_to_databridge_internal() {
        let err: DataBridgeError = TaskError::TaskNotFound("abc".into()).into();
        assert!(matches!(err, DataBridgeError::Internal(s) if s == "Task not found: abc"));
    }

    // T26: From<TaskError> for DataBridgeError — catch-all Revoked → Internal
    #[test]
    fn from_wildcard_revoked() {
        let err: DataBridgeError = TaskError::Revoked("reason".into()).into();
        assert!(matches!(err, DataBridgeError::Internal(s) if s == "Task revoked: reason"));
    }

    // T27: From<TaskError> for DataBridgeError — catch-all RateLimited → Internal(display)
    #[test]
    fn from_wildcard_rate_limited() {
        let dur = Duration::from_secs(10);
        let err: DataBridgeError = TaskError::RateLimited(dur).into();
        match err {
            DataBridgeError::Internal(s) => {
                assert!(s.contains("10s"), "expected '10s' in: {s}");
            }
            other => panic!("expected Internal, got: {other:?}"),
        }
    }

    // T28: From<serde_json::Error> for TaskError → Serialization
    #[test]
    fn from_serde_json_error() {
        let json_err = serde_json::from_str::<String>("not valid json").unwrap_err();
        let err: TaskError = json_err.into();
        assert!(matches!(err, TaskError::Serialization(s) if !s.is_empty()));
    }

    // T29: From<uuid::Error> for TaskError → InvalidTaskId
    #[test]
    fn from_uuid_error() {
        let uuid_err = "not-a-uuid".parse::<uuid::Uuid>().unwrap_err();
        let err: TaskError = uuid_err.into();
        assert!(matches!(err, TaskError::InvalidTaskId(s) if !s.is_empty()));
    }

    // T30: TaskError is Send + Sync
    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TaskError>();
    }

    // T31: Debug impl exists and does not panic
    #[test]
    fn debug_impl_exists() {
        let dbg = format!("{:?}", TaskError::NotConnected);
        assert!(!dbg.is_empty());
    }
}
