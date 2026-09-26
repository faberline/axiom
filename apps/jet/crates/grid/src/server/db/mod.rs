pub mod models;

use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::db::{CellStore, SheetDbError, YrsStore};
use crate::server::error::AppError;
use models::Workbook;

/// Database connection wrapper
///
/// Integrates cclab-grid-db storage for cell data and yrs snapshots.
#[derive(Clone)]
pub struct Database {
    /// Base data directory
    data_dir: PathBuf,
    /// Workbook metadata storage (workbook_id -> Workbook)
    workbooks: Arc<RwLock<HashMap<Uuid, Workbook>>>,
    /// Cell stores per workbook (workbook_id -> CellStore)
    cell_stores: Arc<RwLock<HashMap<Uuid, Arc<CellStore>>>>,
    /// Yrs store for collaborative document persistence
    yrs_store: Arc<YrsStore>,
}

impl From<SheetDbError> for AppError {
    fn from(err: SheetDbError) -> Self {
        AppError::Database(err.to_string())
    }
}

impl Database {
    /// Connect to the database
    ///
    /// Initializes the storage layer with the given data directory.
    pub async fn connect(database_path: &str) -> anyhow::Result<Self> {
        let data_dir = PathBuf::from(database_path);
        tokio::fs::create_dir_all(&data_dir).await?;

        // Initialize yrs store
        let yrs_dir = data_dir.join("yrs");
        let yrs_store = YrsStore::new(&yrs_dir).await?;

        tracing::info!("Database initialized at {:?}", data_dir);

        Ok(Self {
            data_dir,
            workbooks: Arc::new(RwLock::new(HashMap::new())),
            cell_stores: Arc::new(RwLock::new(HashMap::new())),
            yrs_store: Arc::new(yrs_store),
        })
    }

    /// Initialize database (create directories, load existing workbooks)
    pub async fn migrate(&self) -> anyhow::Result<()> {
        // Create cells directory
        let cells_dir = self.data_dir.join("cells");
        tokio::fs::create_dir_all(&cells_dir).await?;

        // Load existing workbook metadata if any
        let metadata_path = self.data_dir.join("workbooks.json");
        if metadata_path.exists() {
            let data = tokio::fs::read_to_string(&metadata_path).await?;
            let workbooks: HashMap<Uuid, Workbook> = serde_json::from_str(&data)?;
            *self.workbooks.write().await = workbooks;
            tracing::info!(
                "Loaded {} workbooks from metadata",
                self.workbooks.read().await.len()
            );
        }

        tracing::info!("Database migration complete");
        Ok(())
    }

    /// Persist workbook metadata to disk
    async fn persist_workbooks(&self) -> Result<(), AppError> {
        let metadata_path = self.data_dir.join("workbooks.json");
        let workbooks = self.workbooks.read().await;
        let data = serde_json::to_string_pretty(&*workbooks)?;
        tokio::fs::write(&metadata_path, data)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// Get or create a cell store for a workbook
    ///
    /// Uses check-create-recheck pattern to avoid holding locks across await points.
    async fn get_cell_store(&self, workbook_id: Uuid) -> Result<Arc<CellStore>, AppError> {
        // Fast path: check with read lock
        {
            let stores = self.cell_stores.read().await;
            if let Some(store) = stores.get(&workbook_id) {
                return Ok(store.clone());
            }
        }
        // Lock is released here before the await

        // Create new store OUTSIDE the lock (avoids holding lock across await)
        let cells_dir = self.data_dir.join("cells");
        let new_store = CellStore::new(&cells_dir, workbook_id.to_string()).await?;
        let new_store = Arc::new(new_store);

        // Now acquire write lock and check if another task created it
        let mut stores = self.cell_stores.write().await;
        if let Some(existing) = stores.get(&workbook_id) {
            // Another task created it while we were awaiting - use theirs, drop ours
            return Ok(existing.clone());
        }

        // We won the race - insert our store
        stores.insert(workbook_id, new_store.clone());
        Ok(new_store)
    }

    /// List all workbooks
    pub async fn list_workbooks(&self) -> Result<Vec<Workbook>, AppError> {
        let workbooks = self.workbooks.read().await;
        let mut result: Vec<_> = workbooks.values().cloned().collect();
        result.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(result)
    }

    /// Create a new workbook
    pub async fn create_workbook(&self, name: &str) -> Result<Workbook, AppError> {
        let workbook = Workbook {
            id: Uuid::new_v4(),
            name: name.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.workbooks
            .write()
            .await
            .insert(workbook.id, workbook.clone());
        self.persist_workbooks().await?;

        // Initialize cell store for this workbook
        self.get_cell_store(workbook.id).await?;

        Ok(workbook)
    }

    /// Get a workbook by ID
    pub async fn get_workbook(&self, id: Uuid) -> Result<Option<Workbook>, AppError> {
        Ok(self.workbooks.read().await.get(&id).cloned())
    }

    /// Update a workbook
    pub async fn update_workbook(&self, id: Uuid, name: &str) -> Result<Workbook, AppError> {
        let mut workbooks = self.workbooks.write().await;

        if let Some(workbook) = workbooks.get_mut(&id) {
            workbook.name = name.to_string();
            workbook.updated_at = Utc::now();
            let updated = workbook.clone();
            drop(workbooks);
            self.persist_workbooks().await?;
            Ok(updated)
        } else {
            Err(AppError::NotFound(format!("Workbook {} not found", id)))
        }
    }

    /// Delete a workbook
    ///
    /// Deletes data first (yrs, cells), then metadata to maintain consistency.
    /// If data deletion fails, metadata remains intact for retry.
    pub async fn delete_workbook(&self, id: Uuid) -> Result<(), AppError> {
        // Verify workbook exists before deleting
        if self.workbooks.read().await.get(&id).is_none() {
            return Err(AppError::NotFound(format!("Workbook {} not found", id)));
        }

        // Delete data FIRST (before metadata) for consistency
        // If these fail, metadata remains and user can retry

        // Delete yrs data
        self.yrs_store.delete_document(&id.to_string()).await?;

        // Remove cell store from cache
        self.cell_stores.write().await.remove(&id);

        // Delete cell store directory on disk
        let cell_store_dir = self.data_dir.join("cells").join(id.to_string());
        if cell_store_dir.exists() {
            tokio::fs::remove_dir_all(&cell_store_dir)
                .await
                .map_err(|e| AppError::Database(format!("Failed to delete cell data: {}", e)))?;
        }

        // Finally remove metadata (data is already cleaned up)
        self.workbooks.write().await.remove(&id);
        self.persist_workbooks().await?;

        Ok(())
    }

    /// Get workbook content as JSON
    ///
    /// Queries all cells in the workbook and returns as JSON.
    pub async fn get_workbook_content(
        &self,
        id: Uuid,
    ) -> Result<Option<serde_json::Value>, AppError> {
        // Check if workbook exists
        if self.workbooks.read().await.get(&id).is_none() {
            return Ok(None);
        }

        let store = self.get_cell_store(id).await?;

        // Query all cells (using large range)
        let cells = store.query_range(0, 0, u32::MAX, u32::MAX).await?;

        // Convert to JSON structure
        let cell_data: Vec<serde_json::Value> = cells
            .iter()
            .map(|cell| {
                serde_json::json!({
                    "row": cell.row,
                    "col": cell.col,
                    "value": cell.value,
                    "version": cell.version
                })
            })
            .collect();

        Ok(Some(serde_json::json!({
            "cells": cell_data,
            "cell_count": cells.len()
        })))
    }

    /// Save workbook content from JSON
    ///
    /// Parses JSON and stores cells in CellStore.
    /// Returns NotFound if workbook doesn't exist.
    pub async fn save_workbook_content(
        &self,
        id: Uuid,
        content: &serde_json::Value,
    ) -> Result<(), AppError> {
        // Verify workbook exists before writing cells (prevents orphaned data)
        if self.workbooks.read().await.get(&id).is_none() {
            return Err(AppError::NotFound(format!("Workbook {} not found", id)));
        }

        let store = self.get_cell_store(id).await?;

        // Parse cells from JSON
        if let Some(cells) = content.get("cells").and_then(|c| c.as_array()) {
            for cell in cells {
                let row = cell.get("row").and_then(|r| r.as_u64()).unwrap_or(0) as u32;
                let col = cell.get("col").and_then(|c| c.as_u64()).unwrap_or(0) as u32;

                if let Some(value) = cell.get("value") {
                    let cell_value = serde_json::from_value(value.clone())
                        .map_err(|e| AppError::BadRequest(format!("Invalid cell value: {}", e)))?;
                    store.set_cell(row, col, cell_value).await?;
                }
            }
        }

        // Flush to disk
        store.flush().await?;

        // Update workbook timestamp
        if let Some(workbook) = self.workbooks.write().await.get_mut(&id) {
            workbook.updated_at = Utc::now();
        }
        self.persist_workbooks().await?;

        Ok(())
    }

    /// Store a CRDT update
    ///
    /// Stores yrs update in YrsStore for persistence.
    pub async fn store_yrs_update(&self, workbook_id: Uuid, update: &[u8]) -> Result<(), AppError> {
        self.yrs_store
            .store_update(&workbook_id.to_string(), update)
            .await?;
        Ok(())
    }

    /// Get all CRDT updates for a workbook
    ///
    /// Retrieves all yrs updates for document recovery.
    pub async fn get_yrs_updates(&self, workbook_id: Uuid) -> Result<Vec<Vec<u8>>, AppError> {
        let updates = self.yrs_store.get_updates(&workbook_id.to_string()).await?;
        Ok(updates.into_iter().map(|u| u.data).collect())
    }

    /// Store a yrs snapshot
    pub async fn store_yrs_snapshot(
        &self,
        workbook_id: Uuid,
        up_to_sequence: u64,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.yrs_store
            .store_snapshot(&workbook_id.to_string(), up_to_sequence, data)
            .await?;
        Ok(())
    }

    /// Get the latest yrs snapshot
    pub async fn get_yrs_snapshot(
        &self,
        workbook_id: Uuid,
    ) -> Result<Option<(u64, Vec<u8>)>, AppError> {
        let snapshot = self
            .yrs_store
            .get_snapshot(&workbook_id.to_string())
            .await?;
        Ok(snapshot.map(|s| (s.up_to_sequence, s.data)))
    }
}
