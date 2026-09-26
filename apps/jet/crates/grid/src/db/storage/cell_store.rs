//! Cell store implementation
//!
//! Provides efficient storage and retrieval of spreadsheet cells using Morton encoding.

use crate::core::CellValue;
use crate::db::storage::morton::MortonKey;
use crate::db::storage::wal::{GridWalOp, GridWalReader, GridWalWriter};
use crate::db::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cell store for spreadsheet data
///
/// Uses Morton encoding to map 2D cell coordinates to 1D keys for efficient storage.
/// Supports atomic operations and WAL for durability.
pub struct CellStore {
    /// In-memory cell storage (Morton key -> StoredCell)
    cells: RwLock<BTreeMap<u64, StoredCell>>,
    /// Write-ahead log for durability
    wal: RwLock<GridWalWriter>,
    /// Sheet identifier
    sheet_id: String,
    /// Data directory
    data_dir: PathBuf,
    /// Current version counter
    version: AtomicU64,
    /// Statistics
    stats: RwLock<StoreStats>,
}

/// Stored cell data with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCell {
    /// Cell row coordinate
    pub row: u32,
    /// Cell column coordinate
    pub col: u32,
    /// Cell value
    pub value: CellValue,
    /// Cell version for CRDT
    pub version: u64,
    /// Timestamp of last modification
    pub timestamp: u64,
}

impl CellStore {
    /// Create a new cell store
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    /// * `sheet_id` - Unique identifier for the sheet
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let store = CellStore::new("./data", "sheet-1").await?;
    /// ```
    pub async fn new<P: AsRef<Path>>(path: P, sheet_id: String) -> Result<Self> {
        let data_dir = path.as_ref().to_path_buf();

        // Create sheet-specific directory
        let sheet_dir = data_dir.join(&sheet_id);
        tokio::fs::create_dir_all(&sheet_dir).await?;

        // Initialize WAL
        let wal = GridWalWriter::new(&sheet_dir)?;

        let store = Self {
            cells: RwLock::new(BTreeMap::new()),
            wal: RwLock::new(wal),
            sheet_id,
            data_dir: sheet_dir,
            version: AtomicU64::new(0),
            stats: RwLock::new(StoreStats::default()),
        };

        // Recover from WAL if exists
        store.recover().await?;

        Ok(store)
    }

    /// Recover state from WAL
    async fn recover(&self) -> Result<()> {
        let wal_path = self.data_dir.join("wal-current.log");

        if !wal_path.exists() {
            return Ok(());
        }

        let mut reader = GridWalReader::new(&wal_path)?;
        let mut cells = self.cells.write();
        let mut max_version = 0u64;

        reader.replay(|op| {
            match op {
                GridWalOp::SetCell {
                    row,
                    col,
                    value,
                    version,
                } => {
                    let key = MortonKey::encode(*row, *col);
                    let stored = StoredCell {
                        row: *row,
                        col: *col,
                        value: value.clone(),
                        version: *version,
                        timestamp: 0, // Timestamp not stored in WAL op
                    };
                    cells.insert(key.as_u64(), stored);
                    max_version = max_version.max(*version);
                }
                GridWalOp::DeleteCell { row, col } => {
                    let key = MortonKey::encode(*row, *col);
                    cells.remove(&key.as_u64());
                }
                GridWalOp::BatchSetCells { cells: batch } => {
                    for (row, col, value, version) in batch {
                        let key = MortonKey::encode(*row, *col);
                        let stored = StoredCell {
                            row: *row,
                            col: *col,
                            value: value.clone(),
                            version: *version,
                            timestamp: 0,
                        };
                        cells.insert(key.as_u64(), stored);
                        max_version = max_version.max(*version);
                    }
                }
                GridWalOp::Checkpoint { .. } => {}
            }
            Ok(())
        })?;

        self.version.store(max_version + 1, Ordering::SeqCst);

        let mut stats = self.stats.write();
        stats.cell_count = cells.len() as u64;

        tracing::info!(
            "Recovered {} cells from WAL, max version {}",
            cells.len(),
            max_version
        );

        Ok(())
    }

    /// Get a cell by coordinates
    ///
    /// # Arguments
    ///
    /// * `row` - Row coordinate
    /// * `col` - Column coordinate
    pub async fn get_cell(&self, row: u32, col: u32) -> Result<Option<StoredCell>> {
        let key = MortonKey::encode(row, col);
        let cells = self.cells.read();
        Ok(cells.get(&key.as_u64()).cloned())
    }

    /// Set a cell value
    ///
    /// # Arguments
    ///
    /// * `row` - Row coordinate
    /// * `col` - Column coordinate
    /// * `value` - Cell value to store
    pub async fn set_cell(&self, row: u32, col: u32, value: CellValue) -> Result<()> {
        let key = MortonKey::encode(row, col);
        let version = self.version.fetch_add(1, Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Write to WAL first
        {
            let mut wal = self.wal.write();
            wal.write_set_cell(row, col, value.clone(), version)?;

            if wal.should_flush() {
                wal.flush()?;
            }
        }

        // Then update in-memory store
        let stored = StoredCell {
            row,
            col,
            value,
            version,
            timestamp,
        };

        let mut cells = self.cells.write();
        let is_new = !cells.contains_key(&key.as_u64());
        cells.insert(key.as_u64(), stored);

        if is_new {
            let mut stats = self.stats.write();
            stats.cell_count += 1;
        }

        Ok(())
    }

    /// Delete a cell
    ///
    /// # Arguments
    ///
    /// * `row` - Row coordinate
    /// * `col` - Column coordinate
    pub async fn delete_cell(&self, row: u32, col: u32) -> Result<()> {
        let key = MortonKey::encode(row, col);

        // Write to WAL first
        {
            let mut wal = self.wal.write();
            wal.write_delete_cell(row, col)?;

            if wal.should_flush() {
                wal.flush()?;
            }
        }

        // Then update in-memory store
        let mut cells = self.cells.write();
        if cells.remove(&key.as_u64()).is_some() {
            let mut stats = self.stats.write();
            stats.cell_count = stats.cell_count.saturating_sub(1);
        }

        Ok(())
    }

    /// Query cells in a rectangular range
    ///
    /// # Arguments
    ///
    /// * `start_row` - Starting row (inclusive)
    /// * `start_col` - Starting column (inclusive)
    /// * `end_row` - Ending row (inclusive)
    /// * `end_col` - Ending column (inclusive)
    pub async fn query_range(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Result<Vec<StoredCell>> {
        let ranges = MortonKey::range_for_rect(start_row, start_col, end_row, end_col);
        let cells = self.cells.read();

        let mut results = Vec::new();

        for (min_key, max_key) in ranges {
            // Range scan using BTreeMap
            for (_, cell) in cells.range(min_key.as_u64()..=max_key.as_u64()) {
                // Post-filter to ensure cell is in rectangle
                if cell.row >= start_row
                    && cell.row <= end_row
                    && cell.col >= start_col
                    && cell.col <= end_col
                {
                    results.push(cell.clone());
                }
            }
        }

        // Sort by Morton key for consistent ordering
        results.sort_by_key(|c| MortonKey::encode(c.row, c.col).as_u64());

        Ok(results)
    }

    /// Flush WAL and sync to disk
    pub async fn flush(&self) -> Result<()> {
        let mut wal = self.wal.write();
        wal.flush()?;
        Ok(())
    }

    /// Write a checkpoint marker
    pub async fn checkpoint(&self) -> Result<()> {
        let mut wal = self.wal.write();
        wal.write_checkpoint()?;
        wal.flush()?;
        Ok(())
    }

    /// Get store statistics
    pub fn stats(&self) -> StoreStats {
        self.stats.read().clone()
    }

    /// Get the sheet ID
    pub fn sheet_id(&self) -> &str {
        &self.sheet_id
    }

    /// Get total cell count
    pub fn cell_count(&self) -> usize {
        self.cells.read().len()
    }
}

/// Store statistics
#[derive(Debug, Clone, Default)]
pub struct StoreStats {
    /// Total number of cells
    pub cell_count: u64,
    /// Total data size in bytes (estimated)
    pub data_size: u64,
    /// Number of WAL entries since last checkpoint
    pub wal_entries: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cell_store_basic() {
        let temp_dir = TempDir::new().unwrap();
        let store = CellStore::new(temp_dir.path(), "test-sheet".to_string())
            .await
            .unwrap();

        // Set a cell
        store.set_cell(0, 0, CellValue::Number(42.0)).await.unwrap();

        // Get it back
        let cell = store.get_cell(0, 0).await.unwrap();
        assert!(cell.is_some());
        let cell = cell.unwrap();
        assert_eq!(cell.row, 0);
        assert_eq!(cell.col, 0);
        match cell.value {
            CellValue::Number(n) => assert_eq!(n, 42.0),
            _ => panic!("Expected Number"),
        }
    }

    #[tokio::test]
    async fn test_cell_store_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = CellStore::new(temp_dir.path(), "test-sheet".to_string())
            .await
            .unwrap();

        store.set_cell(0, 0, CellValue::Number(42.0)).await.unwrap();
        assert!(store.get_cell(0, 0).await.unwrap().is_some());

        store.delete_cell(0, 0).await.unwrap();
        assert!(store.get_cell(0, 0).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cell_store_range_query() {
        let temp_dir = TempDir::new().unwrap();
        let store = CellStore::new(temp_dir.path(), "test-sheet".to_string())
            .await
            .unwrap();

        // Create a 5x5 grid of cells
        for row in 0..5 {
            for col in 0..5 {
                store
                    .set_cell(row, col, CellValue::Number((row * 10 + col) as f64))
                    .await
                    .unwrap();
            }
        }

        // Query a 3x3 subrange
        let results = store.query_range(1, 1, 3, 3).await.unwrap();
        assert_eq!(results.len(), 9); // 3x3 = 9 cells

        // Verify all results are in range
        for cell in &results {
            assert!(cell.row >= 1 && cell.row <= 3);
            assert!(cell.col >= 1 && cell.col <= 3);
        }
    }

    #[tokio::test]
    async fn test_cell_store_recovery() {
        let temp_dir = TempDir::new().unwrap();

        // Create store and add cells
        {
            let store = CellStore::new(temp_dir.path(), "test-sheet".to_string())
                .await
                .unwrap();

            store.set_cell(0, 0, CellValue::Number(1.0)).await.unwrap();
            store
                .set_cell(1, 1, CellValue::Text("hello".to_string()))
                .await
                .unwrap();
            store.flush().await.unwrap();
        }

        // Reopen and verify recovery
        {
            let store = CellStore::new(temp_dir.path(), "test-sheet".to_string())
                .await
                .unwrap();

            let cell1 = store.get_cell(0, 0).await.unwrap();
            assert!(cell1.is_some());

            let cell2 = store.get_cell(1, 1).await.unwrap();
            assert!(cell2.is_some());
            match cell2.unwrap().value {
                CellValue::Text(s) => assert_eq!(s, "hello"),
                _ => panic!("Expected Text"),
            }
        }
    }
}
