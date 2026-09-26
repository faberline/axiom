//! # jet-wal
//!
//! Shared Write-Ahead Log (WAL) implementation for cclab storage engines.
//!
//! This crate provides a generic, reusable WAL implementation that can be used
//! by different storage engines (cclab-ion, cclab-grid-db, etc.).
//!
//! ## Features
//!
//! - Generic entry type support via `WalEntryType` trait
//! - CRC32 checksums for corruption detection
//! - Batched fsync for performance
//! - File rotation support
//! - Crash recovery
//!
//! ## Example
//!
//! ```rust,ignore
//! use jet_wal::{WalWriter, WalReader, WalConfig};
//!
//! // Define your entry type
//! #[derive(Serialize, Deserialize)]
//! enum MyOp {
//!     Set { key: String, value: Vec<u8> },
//!     Delete { key: String },
//! }
//!
//! // Create writer
//! let mut writer = WalWriter::new("./data", WalConfig::default())?;
//! writer.append(&MyOp::Set { key: "k".into(), value: vec![1,2,3] })?;
//! writer.flush()?;
//!
//! // Read back
//! let mut reader = WalReader::<MyOp>::new("./data/wal-current.log")?;
//! while let Some(entry) = reader.read_entry()? {
//!     println!("Entry at {}: {:?}", entry.timestamp, entry.op);
//! }
//! ```

mod entry;
mod error;
mod reader;
mod writer;

pub use entry::{WalEntry, WalHeader};
pub use error::{Result, WalError};
pub use reader::WalReader;
pub use writer::WalWriter;

use std::path::PathBuf;

/// WAL configuration
#[derive(Debug, Clone)]
pub struct WalConfig {
    /// Flush interval in milliseconds (default: 100ms)
    pub flush_interval_ms: u64,
    /// Maximum WAL file size before rotation (default: 1GB)
    pub max_file_size: u64,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            flush_interval_ms: 100,
            max_file_size: 1024 * 1024 * 1024, // 1GB
        }
    }
}

impl WalConfig {
    /// Create config for testing with smaller limits
    pub fn for_testing() -> Self {
        Self {
            flush_interval_ms: 10,
            max_file_size: 1024, // 1KB
        }
    }
}

/// Find all WAL files in a directory
pub fn find_wal_files(data_dir: impl AsRef<std::path::Path>) -> Result<Vec<PathBuf>> {
    let data_dir = data_dir.as_ref();

    if !data_dir.exists() {
        return Ok(Vec::new());
    }

    let mut wal_files = Vec::new();

    for entry in std::fs::read_dir(data_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(filename) = path.file_name() {
            if let Some(name) = filename.to_str() {
                if name.starts_with("wal-") && name.ends_with(".log") {
                    wal_files.push(path);
                }
            }
        }
    }

    // Sort by filename (timestamp order)
    wal_files.sort();

    Ok(wal_files)
}

/// Delete old WAL files, keeping only the most recent N
pub fn cleanup_old_wal_files(
    data_dir: impl AsRef<std::path::Path>,
    keep_count: usize,
) -> Result<usize> {
    let mut wal_files = find_wal_files(data_dir)?;

    // Don't delete wal-current.log
    wal_files.retain(|p| {
        !p.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s == "wal-current.log")
            .unwrap_or(false)
    });

    if wal_files.len() <= keep_count {
        return Ok(0);
    }

    let to_delete = wal_files.len() - keep_count;
    let mut deleted = 0;

    for path in wal_files.iter().take(to_delete) {
        match std::fs::remove_file(path) {
            Ok(_) => {
                tracing::debug!("Deleted old WAL file: {}", path.display());
                deleted += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to delete WAL file {}: {}", path.display(), e);
            }
        }
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    enum TestOp {
        Set { key: String, value: String },
        Delete { key: String },
    }

    #[test]
    fn test_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        // Write entries
        let mut writer = WalWriter::new(&data_dir, WalConfig::default()).unwrap();

        let ops = vec![
            TestOp::Set {
                key: "key1".to_string(),
                value: "value1".to_string(),
            },
            TestOp::Set {
                key: "key2".to_string(),
                value: "value2".to_string(),
            },
            TestOp::Delete {
                key: "key1".to_string(),
            },
        ];

        for op in &ops {
            writer.append(op).unwrap();
        }
        writer.flush().unwrap();

        // Read entries back
        let wal_path = data_dir.join("wal-current.log");
        let mut reader = WalReader::<TestOp>::new(&wal_path).unwrap();

        let mut read_ops = Vec::new();
        while let Some(entry) = reader.read_entry().unwrap() {
            read_ops.push(entry.op);
        }

        assert_eq!(read_ops, ops);
    }

    #[test]
    fn test_find_wal_files() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        // Create some WAL files
        let mut writer = WalWriter::<TestOp>::new(&data_dir, WalConfig::default()).unwrap();
        writer
            .append(&TestOp::Delete {
                key: "test".to_string(),
            })
            .unwrap();
        writer.flush().unwrap();

        let files = find_wal_files(&data_dir).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_str().unwrap().contains("wal-current.log"));
    }
}
