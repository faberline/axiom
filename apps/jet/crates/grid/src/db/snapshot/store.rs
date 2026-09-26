//! yrs snapshot storage implementation

use crate::db::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Store for yrs updates and snapshots
///
/// Provides persistence for collaborative document state. Updates are stored
/// incrementally, and periodic snapshots allow for efficient recovery.
pub struct YrsStore {
    /// Base directory for storage
    data_dir: PathBuf,
    /// In-memory cache of updates by document ID
    updates: RwLock<HashMap<String, Vec<YrsUpdate>>>,
    /// In-memory cache of latest snapshots
    snapshots: RwLock<HashMap<String, YrsSnapshot>>,
}

/// A stored yrs update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YrsUpdate {
    /// Sequence number for ordering
    pub sequence: u64,
    /// Timestamp when update was received
    pub timestamp: u64,
    /// The raw yrs update bytes
    pub data: Vec<u8>,
}

/// A stored yrs snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YrsSnapshot {
    /// Sequence number this snapshot covers up to
    pub up_to_sequence: u64,
    /// Timestamp when snapshot was created
    pub timestamp: u64,
    /// The raw yrs state vector
    pub data: Vec<u8>,
}

impl YrsStore {
    /// Create a new yrs store
    pub async fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        fs::create_dir_all(&data_dir).await?;

        Ok(Self {
            data_dir,
            updates: RwLock::new(HashMap::new()),
            snapshots: RwLock::new(HashMap::new()),
        })
    }

    /// Store a yrs update for a document
    pub async fn store_update(&self, doc_id: &str, update: &[u8]) -> Result<u64> {
        let mut updates = self.updates.write();
        let doc_updates = updates.entry(doc_id.to_string()).or_default();

        let sequence = doc_updates.len() as u64;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let yrs_update = YrsUpdate {
            sequence,
            timestamp,
            data: update.to_vec(),
        };

        doc_updates.push(yrs_update);

        // Persist to disk
        self.persist_update(doc_id, sequence, update).await?;

        Ok(sequence)
    }

    /// Get all updates for a document (for recovery)
    pub async fn get_updates(&self, doc_id: &str) -> Result<Vec<YrsUpdate>> {
        // First check in-memory cache
        {
            let updates = self.updates.read();
            if let Some(doc_updates) = updates.get(doc_id) {
                return Ok(doc_updates.clone());
            }
        }

        // Load from disk
        self.load_updates(doc_id).await
    }

    /// Store a snapshot for a document
    pub async fn store_snapshot(
        &self,
        doc_id: &str,
        up_to_sequence: u64,
        data: &[u8],
    ) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let snapshot = YrsSnapshot {
            up_to_sequence,
            timestamp,
            data: data.to_vec(),
        };

        // Store in memory
        {
            let mut snapshots = self.snapshots.write();
            snapshots.insert(doc_id.to_string(), snapshot.clone());
        }

        // Persist to disk
        self.persist_snapshot(doc_id, &snapshot).await?;

        // Optionally compact old updates
        self.compact_updates(doc_id, up_to_sequence).await?;

        Ok(())
    }

    /// Get the latest snapshot for a document
    pub async fn get_snapshot(&self, doc_id: &str) -> Result<Option<YrsSnapshot>> {
        // Check in-memory cache
        {
            let snapshots = self.snapshots.read();
            if let Some(snapshot) = snapshots.get(doc_id) {
                return Ok(Some(snapshot.clone()));
            }
        }

        // Load from disk
        self.load_snapshot(doc_id).await
    }

    /// Delete all data for a document
    pub async fn delete_document(&self, doc_id: &str) -> Result<()> {
        // Remove from memory
        {
            let mut updates = self.updates.write();
            updates.remove(doc_id);
        }
        {
            let mut snapshots = self.snapshots.write();
            snapshots.remove(doc_id);
        }

        // Remove from disk
        let doc_dir = self.data_dir.join(doc_id);
        if doc_dir.exists() {
            fs::remove_dir_all(&doc_dir).await?;
        }

        Ok(())
    }

    // Private helper methods

    async fn persist_update(&self, doc_id: &str, sequence: u64, data: &[u8]) -> Result<()> {
        let doc_dir = self.data_dir.join(doc_id).join("updates");
        fs::create_dir_all(&doc_dir).await?;

        let update_path = doc_dir.join(format!("{:016}.bin", sequence));
        let mut file = fs::File::create(&update_path).await?;
        file.write_all(data).await?;
        file.sync_all().await?;

        Ok(())
    }

    async fn load_updates(&self, doc_id: &str) -> Result<Vec<YrsUpdate>> {
        let doc_dir = self.data_dir.join(doc_id).join("updates");

        if !doc_dir.exists() {
            return Ok(Vec::new());
        }

        let mut updates = Vec::new();
        let mut entries = fs::read_dir(&doc_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "bin").unwrap_or(false) {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let sequence: u64 = filename.parse().unwrap_or(0);

                let mut file = fs::File::open(&path).await?;
                let mut data = Vec::new();
                file.read_to_end(&mut data).await?;

                updates.push(YrsUpdate {
                    sequence,
                    timestamp: 0, // Not stored in filename
                    data,
                });
            }
        }

        // Sort by sequence
        updates.sort_by_key(|u| u.sequence);

        // Cache in memory
        {
            let mut cache = self.updates.write();
            cache.insert(doc_id.to_string(), updates.clone());
        }

        Ok(updates)
    }

    async fn persist_snapshot(&self, doc_id: &str, snapshot: &YrsSnapshot) -> Result<()> {
        let doc_dir = self.data_dir.join(doc_id);
        fs::create_dir_all(&doc_dir).await?;

        let snapshot_path = doc_dir.join("snapshot.json");
        let encoded = serde_json::to_vec(snapshot)?;

        let mut file = fs::File::create(&snapshot_path).await?;
        file.write_all(&encoded).await?;
        file.sync_all().await?;

        Ok(())
    }

    async fn load_snapshot(&self, doc_id: &str) -> Result<Option<YrsSnapshot>> {
        let snapshot_path = self.data_dir.join(doc_id).join("snapshot.json");

        if !snapshot_path.exists() {
            return Ok(None);
        }

        let mut file = fs::File::open(&snapshot_path).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;

        let snapshot: YrsSnapshot = serde_json::from_slice(&data)?;

        // Cache in memory
        {
            let mut cache = self.snapshots.write();
            cache.insert(doc_id.to_string(), snapshot.clone());
        }

        Ok(Some(snapshot))
    }

    async fn compact_updates(&self, doc_id: &str, up_to_sequence: u64) -> Result<()> {
        let doc_dir = self.data_dir.join(doc_id).join("updates");

        if !doc_dir.exists() {
            return Ok(());
        }

        // Remove old update files
        let mut entries = fs::read_dir(&doc_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map(|e| e == "bin").unwrap_or(false) {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let sequence: u64 = filename.parse().unwrap_or(0);

                if sequence <= up_to_sequence {
                    let _ = fs::remove_file(&path).await;
                }
            }
        }

        // Update in-memory cache
        {
            let mut updates = self.updates.write();
            if let Some(doc_updates) = updates.get_mut(doc_id) {
                doc_updates.retain(|u| u.sequence > up_to_sequence);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_and_get_updates() {
        let temp_dir = TempDir::new().unwrap();
        let store = YrsStore::new(temp_dir.path()).await.unwrap();

        let doc_id = "test-doc";
        let update1 = vec![1, 2, 3, 4];
        let update2 = vec![5, 6, 7, 8];

        let seq1 = store.store_update(doc_id, &update1).await.unwrap();
        let seq2 = store.store_update(doc_id, &update2).await.unwrap();

        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);

        let updates = store.get_updates(doc_id).await.unwrap();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].data, update1);
        assert_eq!(updates[1].data, update2);
    }

    #[tokio::test]
    async fn test_store_and_get_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let store = YrsStore::new(temp_dir.path()).await.unwrap();

        let doc_id = "test-doc";
        let snapshot_data = vec![10, 20, 30, 40];

        store
            .store_snapshot(doc_id, 5, &snapshot_data)
            .await
            .unwrap();

        let snapshot = store.get_snapshot(doc_id).await.unwrap();
        assert!(snapshot.is_some());

        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.up_to_sequence, 5);
        assert_eq!(snapshot.data, snapshot_data);
    }

    #[tokio::test]
    async fn test_compact_updates() {
        let temp_dir = TempDir::new().unwrap();
        let store = YrsStore::new(temp_dir.path()).await.unwrap();

        let doc_id = "test-doc";

        // Store 10 updates
        for i in 0..10 {
            store.store_update(doc_id, &vec![i as u8]).await.unwrap();
        }

        // Store snapshot covering first 5
        store
            .store_snapshot(doc_id, 4, &vec![0, 1, 2, 3, 4])
            .await
            .unwrap();

        // Check updates - should only have 5-9
        let updates = store.get_updates(doc_id).await.unwrap();
        assert_eq!(updates.len(), 5);
        assert_eq!(updates[0].sequence, 5);
    }

    #[tokio::test]
    async fn test_delete_document() {
        let temp_dir = TempDir::new().unwrap();
        let store = YrsStore::new(temp_dir.path()).await.unwrap();

        let doc_id = "test-doc";
        store.store_update(doc_id, &vec![1, 2, 3]).await.unwrap();
        store
            .store_snapshot(doc_id, 0, &vec![1, 2, 3])
            .await
            .unwrap();

        store.delete_document(doc_id).await.unwrap();

        let updates = store.get_updates(doc_id).await.unwrap();
        assert!(updates.is_empty());

        let snapshot = store.get_snapshot(doc_id).await.unwrap();
        assert!(snapshot.is_none());
    }
}
