//! Document trait and types for MongoDB ORM
//!
//! This module provides the core `Document` trait that all MongoDB document types
//! must implement. It supports full CRUD operations with automatic BSON serialization.

use async_trait::async_trait;
use bson::{doc, oid::ObjectId, Document as BsonDocument};
use mamba_core::{DataBridgeError, Result};
use futures::TryStreamExt;
use mongodb::{Collection, Database};
use serde::{de::DeserializeOwned, Serialize};

/// Core trait for MongoDB documents
///
/// This trait provides the foundation for all document types in cclab.
/// Implementing types must be Serialize + DeserializeOwned to enable automatic
/// BSON conversion.
///
/// # Example
///
/// ```ignore
/// use serde::{Deserialize, Serialize};
/// use mongokit::Document;
///
/// #[derive(Debug, Serialize, Deserialize)]
/// struct User {
///     #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
///     id: Option<ObjectId>,
///     email: String,
///     name: String,
/// }
///
/// impl Document for User {
///     fn collection_name() -> &'static str {
///         "users"
///     }
/// }
/// ```
#[async_trait]
pub trait Document: Serialize + DeserializeOwned + Send + Sync + Sized {
    /// Get the collection name for this document type
    fn collection_name() -> &'static str;

    /// Get the document's ObjectId (if it has one)
    fn get_id(&self) -> Option<ObjectId> {
        None
    }

    /// Set the document's ObjectId
    fn set_id(&mut self, _id: ObjectId) {
        // Default implementation does nothing
        // Override this if your document has an _id field
    }

    /// Convert document to BSON
    fn to_bson(&self) -> Result<BsonDocument> {
        bson::to_document(self).map_err(|e| DataBridgeError::Serialization(e.to_string()))
    }

    /// Create document from BSON
    fn from_bson(doc: BsonDocument) -> Result<Self> {
        bson::from_document(doc).map_err(|e| DataBridgeError::Deserialization(e.to_string()))
    }

    /// Get a typed collection for this document
    fn collection(db: &Database) -> Collection<BsonDocument> {
        db.collection(Self::collection_name())
    }

    /// Insert this document into the database
    ///
    /// Returns the ObjectId of the inserted document
    async fn insert_one(&mut self, db: &Database) -> Result<ObjectId> {
        let collection = Self::collection(db);
        let bson_doc = self.to_bson()?;

        let result = collection
            .insert_one(bson_doc)
            .await
            .map_err(|e| DataBridgeError::Database(e.to_string()))?;

        // Extract the ObjectId from the result
        let id = result
            .inserted_id
            .as_object_id()
            .ok_or_else(|| DataBridgeError::Database("Invalid inserted ID".to_string()))?;

        // Set the ID on the document
        self.set_id(id);

        Ok(id)
    }

    /// Find a single document matching the filter
    async fn find_one(db: &Database, filter: BsonDocument) -> Result<Option<Self>> {
        let collection = Self::collection(db);

        let result = collection
            .find_one(filter)
            .await
            .map_err(|e| DataBridgeError::Database(e.to_string()))?;

        match result {
            Some(doc) => Ok(Some(Self::from_bson(doc)?)),
            None => Ok(None),
        }
    }

    /// Find a document by its ObjectId
    async fn find_by_id(db: &Database, id: ObjectId) -> Result<Option<Self>> {
        Self::find_one(db, doc! { "_id": id }).await
    }

    /// Find all documents matching the filter
    async fn find(db: &Database, filter: BsonDocument) -> Result<Vec<Self>> {
        let collection = Self::collection(db);

        let cursor = collection
            .find(filter)
            .await
            .map_err(|e| DataBridgeError::Database(e.to_string()))?;

        let docs: Vec<BsonDocument> = cursor
            .try_collect()
            .await
            .map_err(|e| DataBridgeError::Database(e.to_string()))?;

        docs.into_iter().map(Self::from_bson).collect()
    }

    /// Find all documents in the collection
    async fn find_all(db: &Database) -> Result<Vec<Self>> {
        Self::find(db, doc! {}).await
    }

    /// Update a single document matching the filter
    ///
    /// Returns true if a document was modified
    async fn update_one(db: &Database, filter: BsonDocument, update: BsonDocument) -> Result<bool> {
        let collection = Self::collection(db);

        let result = collection
            .update_one(filter, doc! { "$set": update })
            .await
            .map_err(|e| DataBridgeError::Database(e.to_string()))?;

        Ok(result.modified_count > 0)
    }

    /// Update this document in the database
    ///
    /// Requires the document to have an _id set
    async fn save(&self, db: &Database) -> Result<bool> {
        let id = self
            .get_id()
            .ok_or_else(|| DataBridgeError::Database("Document has no _id".to_string()))?;

        let bson_doc = self.to_bson()?;

        // Remove _id from the update document
        let mut update_doc = bson_doc;
        update_doc.remove("_id");

        Self::update_one(db, doc! { "_id": id }, update_doc).await
    }

    /// Delete a single document matching the filter
    ///
    /// Returns true if a document was deleted
    async fn delete_one(db: &Database, filter: BsonDocument) -> Result<bool> {
        let collection = Self::collection(db);

        let result = collection
            .delete_one(filter)
            .await
            .map_err(|e| DataBridgeError::Database(e.to_string()))?;

        Ok(result.deleted_count > 0)
    }

    /// Delete this document from the database
    ///
    /// Requires the document to have an _id set
    async fn delete(&self, db: &Database) -> Result<bool> {
        let id = self
            .get_id()
            .ok_or_else(|| DataBridgeError::Database("Document has no _id".to_string()))?;

        Self::delete_one(db, doc! { "_id": id }).await
    }

    /// Delete a document by its ObjectId
    async fn delete_by_id(db: &Database, id: ObjectId) -> Result<bool> {
        Self::delete_one(db, doc! { "_id": id }).await
    }

    /// Count documents matching the filter
    async fn count(db: &Database, filter: BsonDocument) -> Result<u64> {
        let collection = Self::collection(db);

        collection
            .count_documents(filter)
            .await
            .map_err(|e| DataBridgeError::Database(e.to_string()))
    }

    /// Count all documents in the collection
    async fn count_all(db: &Database) -> Result<u64> {
        Self::count(db, doc! {}).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestDoc {
        #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
        id: Option<ObjectId>,
        name: String,
        value: i32,
    }

    impl Document for TestDoc {
        fn collection_name() -> &'static str {
            "test_docs"
        }

        fn get_id(&self) -> Option<ObjectId> {
            self.id
        }

        fn set_id(&mut self, id: ObjectId) {
            self.id = Some(id);
        }
    }

    #[test]
    fn test_collection_name() {
        assert_eq!(TestDoc::collection_name(), "test_docs");
    }

    #[test]
    fn test_to_bson() {
        let doc = TestDoc {
            id: None,
            name: "test".to_string(),
            value: 42,
        };

        let bson = doc.to_bson().unwrap();
        assert_eq!(bson.get_str("name").unwrap(), "test");
        assert_eq!(bson.get_i32("value").unwrap(), 42);
    }

    #[test]
    fn test_from_bson() {
        let bson = doc! {
            "name": "test",
            "value": 42
        };

        let doc = TestDoc::from_bson(bson).unwrap();
        assert_eq!(doc.name, "test");
        assert_eq!(doc.value, 42);
    }

    #[test]
    fn test_roundtrip() {
        let original = TestDoc {
            id: Some(ObjectId::new()),
            name: "roundtrip".to_string(),
            value: 100,
        };

        let bson = original.to_bson().unwrap();
        let recovered = TestDoc::from_bson(bson).unwrap();

        assert_eq!(original, recovered);
    }
}
