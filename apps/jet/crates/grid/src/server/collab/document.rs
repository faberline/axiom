use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use yrs::updates::decoder::Decode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

/// A collaborative document with its y-crdt state
pub struct CollabDocument {
    /// The y-crdt document
    pub doc: Doc,
    /// Broadcast channel for updates
    pub updates: broadcast::Sender<Vec<u8>>,
}

impl CollabDocument {
    pub fn new() -> Self {
        let doc = Doc::new();
        let (updates, _) = broadcast::channel(256);
        Self { doc, updates }
    }

    /// Apply an update from a client
    pub fn apply_update(&self, update: &[u8]) -> anyhow::Result<()> {
        let mut txn = self.doc.transact_mut();
        let update = Update::decode_v1(update)?;
        txn.apply_update(update)?;
        Ok(())
    }

    /// Get the current state as an encoded update
    pub fn encode_state(&self) -> Vec<u8> {
        let txn = self.doc.transact();
        txn.encode_state_as_update_v1(&StateVector::default())
    }

    /// Subscribe to updates
    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.updates.subscribe()
    }

    /// Broadcast an update to all subscribers
    pub fn broadcast(&self, update: Vec<u8>) {
        // Ignore errors if no receivers
        let _ = self.updates.send(update);
    }
}

impl Default for CollabDocument {
    fn default() -> Self {
        Self::new()
    }
}

/// Store for managing multiple collaborative documents
pub struct DocumentStore {
    documents: RwLock<HashMap<Uuid, Arc<RwLock<CollabDocument>>>>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            documents: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a document for a workbook
    pub async fn get_or_create(&self, workbook_id: Uuid) -> Arc<RwLock<CollabDocument>> {
        // First try to get existing
        {
            let docs = self.documents.read().await;
            if let Some(doc) = docs.get(&workbook_id) {
                return Arc::clone(doc);
            }
        }

        // Create new document
        let mut docs = self.documents.write().await;
        // Double-check after acquiring write lock
        if let Some(doc) = docs.get(&workbook_id) {
            return Arc::clone(doc);
        }

        let doc = Arc::new(RwLock::new(CollabDocument::new()));
        docs.insert(workbook_id, Arc::clone(&doc));
        doc
    }

    /// Remove a document from the store
    pub async fn remove(&self, workbook_id: Uuid) {
        let mut docs = self.documents.write().await;
        docs.remove(&workbook_id);
    }

    /// Get document count
    pub async fn count(&self) -> usize {
        self.documents.read().await.len()
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}
