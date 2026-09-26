//! Write-Ahead Log (WAL) for grid-db cell storage
//!
//! Uses the shared jet-wal crate with grid-specific operation types.

use crate::core::CellValue;
use crate::db::Result;
use jet_wal::{WalConfig, WalReader, WalWriter};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Grid-specific WAL operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GridWalOp {
    /// Set a cell value
    SetCell {
        /// Row coordinate
        row: u32,
        /// Column coordinate
        col: u32,
        /// Cell value
        value: CellValue,
        /// Version for conflict detection
        version: u64,
    },
    /// Delete a cell
    DeleteCell {
        /// Row coordinate
        row: u32,
        /// Column coordinate
        col: u32,
    },
    /// Batch set multiple cells
    BatchSetCells {
        /// List of (row, col, value, version) tuples
        cells: Vec<(u32, u32, CellValue, u64)>,
    },
    /// Checkpoint marker
    Checkpoint {
        /// Sequence number at checkpoint
        sequence: u64,
    },
}

/// Grid WAL writer wrapper
pub struct GridWalWriter {
    inner: WalWriter<GridWalOp>,
    sequence: u64,
}

impl GridWalWriter {
    /// Create a new grid WAL writer
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
        let config = WalConfig::default();
        let inner = WalWriter::new(data_dir, config)?;
        Ok(Self { inner, sequence: 0 })
    }

    /// Create with custom config
    pub fn with_config(data_dir: impl AsRef<Path>, config: WalConfig) -> Result<Self> {
        let inner = WalWriter::new(data_dir, config)?;
        Ok(Self { inner, sequence: 0 })
    }

    /// Write a set cell operation
    pub fn write_set_cell(
        &mut self,
        row: u32,
        col: u32,
        value: CellValue,
        version: u64,
    ) -> Result<u64> {
        let op = GridWalOp::SetCell {
            row,
            col,
            value,
            version,
        };
        let pos = self.inner.append(&op)?;
        self.sequence += 1;
        Ok(pos)
    }

    /// Write a delete cell operation
    pub fn write_delete_cell(&mut self, row: u32, col: u32) -> Result<u64> {
        let op = GridWalOp::DeleteCell { row, col };
        let pos = self.inner.append(&op)?;
        self.sequence += 1;
        Ok(pos)
    }

    /// Write a batch set operation
    pub fn write_batch_set(&mut self, cells: Vec<(u32, u32, CellValue, u64)>) -> Result<u64> {
        let op = GridWalOp::BatchSetCells { cells };
        let pos = self.inner.append(&op)?;
        self.sequence += 1;
        Ok(pos)
    }

    /// Write a checkpoint marker
    pub fn write_checkpoint(&mut self) -> Result<u64> {
        let op = GridWalOp::Checkpoint {
            sequence: self.sequence,
        };
        let pos = self.inner.append(&op)?;
        Ok(pos)
    }

    /// Flush WAL to disk
    pub fn flush(&mut self) -> Result<()> {
        self.inner.flush()?;
        Ok(())
    }

    /// Check if flush is needed
    pub fn should_flush(&self) -> bool {
        self.inner.should_flush()
    }

    /// Check if rotation is needed
    pub fn should_rotate(&self) -> bool {
        self.inner.should_rotate()
    }

    /// Rotate to a new WAL file
    pub fn rotate(&mut self) -> Result<PathBuf> {
        let path = self.inner.rotate()?;
        Ok(path)
    }

    /// Get current sequence number
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Get current file position
    pub fn position(&self) -> u64 {
        self.inner.position()
    }
}

/// Grid WAL reader wrapper
pub struct GridWalReader {
    inner: WalReader<GridWalOp>,
}

impl GridWalReader {
    /// Open a WAL file for reading
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let inner = WalReader::new(path)?;
        Ok(Self { inner })
    }

    /// Read all operations from the WAL
    pub fn read_all(&mut self) -> Result<Vec<GridWalOp>> {
        let entries = self.inner.read_all()?;
        Ok(entries.into_iter().map(|e| e.op).collect())
    }

    /// Replay WAL with a callback for each operation
    pub fn replay<F>(&mut self, mut callback: F) -> Result<u64>
    where
        F: FnMut(&GridWalOp) -> Result<()>,
    {
        let mut count = 0;
        let entries = self.inner.read_all()?;
        for entry in entries {
            callback(&entry.op)?;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_and_read_operations() {
        let temp_dir = TempDir::new().unwrap();

        // Write operations
        let mut writer = GridWalWriter::new(temp_dir.path()).unwrap();

        writer
            .write_set_cell(0, 0, CellValue::Number(42.0), 1)
            .unwrap();
        writer
            .write_set_cell(1, 1, CellValue::Text("hello".to_string()), 2)
            .unwrap();
        writer.write_delete_cell(0, 0).unwrap();
        writer.flush().unwrap();
        drop(writer);

        // Read operations
        let mut reader = GridWalReader::new(temp_dir.path().join("wal-current.log")).unwrap();
        let ops = reader.read_all().unwrap();

        assert_eq!(ops.len(), 3);

        match &ops[0] {
            GridWalOp::SetCell { row, col, .. } => {
                assert_eq!(*row, 0);
                assert_eq!(*col, 0);
            }
            _ => panic!("Expected SetCell"),
        }

        match &ops[2] {
            GridWalOp::DeleteCell { row, col } => {
                assert_eq!(*row, 0);
                assert_eq!(*col, 0);
            }
            _ => panic!("Expected DeleteCell"),
        }
    }

    #[test]
    fn test_replay() {
        let temp_dir = TempDir::new().unwrap();

        // Write operations
        let mut writer = GridWalWriter::new(temp_dir.path()).unwrap();
        for i in 0..10 {
            writer
                .write_set_cell(i, i, CellValue::Number(i as f64), i as u64)
                .unwrap();
        }
        writer.flush().unwrap();
        drop(writer);

        // Replay
        let mut reader = GridWalReader::new(temp_dir.path().join("wal-current.log")).unwrap();
        let mut count = 0;
        reader
            .replay(|op| {
                match op {
                    GridWalOp::SetCell { .. } => count += 1,
                    _ => {}
                }
                Ok(())
            })
            .unwrap();

        assert_eq!(count, 10);
    }
}
