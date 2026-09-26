//! MongoDB implementation for cclab
//!
//! This crate provides a high-performance MongoDB ORM with full Beanie compatibility.
//!
//! # Features
//! - Zero-copy BSON deserialization
//! - Async/await support via tokio
//! - Query builder with Beanie-compatible API
//! - Aggregation pipeline support
//! - Index management
//! - Revision tracking
//! - State management
//! - Type-safe bulk operations

pub mod aggregation;
pub mod bulk;
pub mod connection;
pub mod document;
pub mod link;
pub mod query;
pub mod state;
pub mod validation;

pub use aggregation::{Accumulator, AggregationBuilder, AggregationStage, GroupId};
pub use bulk::{BulkOperation, BulkWriteResult};
pub use mamba_core::{DataBridgeError, Result};
pub use connection::{Connection, PoolConfig};
pub use document::Document;
pub use link::{BatchFetchResult, CollectedRefs, LinkField, LinkRef, LinkType};
pub use query::{QueryBuilder, QueryExpr};
pub use state::StateTracker;
pub use validation::{
    validate_document, validate_field, validate_query, BsonConstraints, BsonTypeDescriptor,
    ObjectIdParser, ValidatedCollectionName, ValidatedFieldName,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
