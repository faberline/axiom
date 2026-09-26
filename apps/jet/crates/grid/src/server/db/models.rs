use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Workbook database model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workbook {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// CRDT update entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YrsUpdate {
    pub id: i64,
    pub workbook_id: Uuid,
    pub update_data: Vec<u8>,
    pub created_at: DateTime<Utc>,
}
