//! Range query builder for rectangular regions
//!
//! Provides efficient querying of cells in rectangular ranges.

use crate::db::storage::StoredCell;
use crate::db::{Result, SheetDbError};
use serde::{Deserialize, Serialize};

/// Query builder for rectangular cell ranges
///
/// # Example
///
/// ```rust,ignore
/// let query = RangeQuery::new()
///     .row_range(0, 100)
///     .col_range(0, 26)
///     .filter_non_empty()
///     .build();
/// ```
#[derive(Debug, Clone, Default)]
pub struct RangeQuery {
    /// Starting row (inclusive)
    start_row: Option<u32>,
    /// Ending row (inclusive)
    end_row: Option<u32>,
    /// Starting column (inclusive)
    start_col: Option<u32>,
    /// Ending column (inclusive)
    end_col: Option<u32>,
    /// Filter options
    filter: QueryFilter,
    /// Sort options
    sort: Option<SortOption>,
    /// Limit number of results
    limit: Option<usize>,
}

/// Filter options for queries
#[derive(Default)]
pub struct QueryFilter {
    /// Only return non-empty cells
    pub non_empty: bool,
    /// Filter by value type
    pub value_type: Option<ValueTypeFilter>,
    /// Custom filter predicate (not serializable)
    pub custom: Option<Box<dyn Fn(&StoredCell) -> bool + Send + Sync>>,
}

impl std::fmt::Debug for QueryFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryFilter")
            .field("non_empty", &self.non_empty)
            .field("value_type", &self.value_type)
            .field("custom", &self.custom.as_ref().map(|_| "<function>"))
            .finish()
    }
}

impl Clone for QueryFilter {
    fn clone(&self) -> Self {
        Self {
            non_empty: self.non_empty,
            value_type: self.value_type.clone(),
            // Custom predicates cannot be cloned, reset to None
            custom: None,
        }
    }
}

/// Filter by value type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueTypeFilter {
    /// Only number values
    Number,
    /// Only string values
    String,
    /// Only boolean values
    Boolean,
    /// Only formula values
    Formula,
    /// Only error values
    Error,
}

/// Sort options for query results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOption {
    /// Sort by row, then column
    ByRowCol,
    /// Sort by column, then row
    ByColRow,
    /// Sort by value
    ByValue,
    /// Sort by modification timestamp
    ByTimestamp,
}

impl RangeQuery {
    /// Create a new range query builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set row range (inclusive)
    pub fn row_range(mut self, start: u32, end: u32) -> Self {
        self.start_row = Some(start);
        self.end_row = Some(end);
        self
    }

    /// Set column range (inclusive)
    pub fn col_range(mut self, start: u32, end: u32) -> Self {
        self.start_col = Some(start);
        self.end_col = Some(end);
        self
    }

    /// Set specific row
    pub fn row(mut self, row: u32) -> Self {
        self.start_row = Some(row);
        self.end_row = Some(row);
        self
    }

    /// Set specific column
    pub fn col(mut self, col: u32) -> Self {
        self.start_col = Some(col);
        self.end_col = Some(col);
        self
    }

    /// Filter to only non-empty cells
    pub fn filter_non_empty(mut self) -> Self {
        self.filter.non_empty = true;
        self
    }

    /// Filter by value type
    pub fn filter_value_type(mut self, value_type: ValueTypeFilter) -> Self {
        self.filter.value_type = Some(value_type);
        self
    }

    /// Add custom filter predicate
    pub fn filter_custom<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&StoredCell) -> bool + Send + Sync + 'static,
    {
        self.filter.custom = Some(Box::new(predicate));
        self
    }

    /// Set sort option
    pub fn sort_by(mut self, sort: SortOption) -> Self {
        self.sort = Some(sort);
        self
    }

    /// Limit number of results
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Get the range bounds
    pub fn bounds(&self) -> Result<(u32, u32, u32, u32)> {
        let start_row = self
            .start_row
            .ok_or_else(|| SheetDbError::InvalidInput("Row range not specified".to_string()))?;
        let end_row = self
            .end_row
            .ok_or_else(|| SheetDbError::InvalidInput("Row range not specified".to_string()))?;
        let start_col = self
            .start_col
            .ok_or_else(|| SheetDbError::InvalidInput("Column range not specified".to_string()))?;
        let end_col = self
            .end_col
            .ok_or_else(|| SheetDbError::InvalidInput("Column range not specified".to_string()))?;

        Ok((start_row, start_col, end_row, end_col))
    }

    /// Apply filters to a cell
    pub fn matches_filter(&self, _cell: &StoredCell) -> bool {
        // TODO: Implement filter matching
        // - Check non_empty filter
        // - Check value_type filter
        // - Apply custom filter if present
        todo!("Implement matches_filter")
    }

    /// Sort cells according to sort option
    pub fn apply_sort(&self, _cells: &mut Vec<StoredCell>) {
        // TODO: Implement sorting
        // - Sort by specified option
        // - Handle None case (no sorting)
        todo!("Implement apply_sort")
    }

    /// Apply limit to results
    pub fn apply_limit(&self, cells: Vec<StoredCell>) -> Vec<StoredCell> {
        if let Some(limit) = self.limit {
            cells.into_iter().take(limit).collect()
        } else {
            cells
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_query_builder() {
        let query = RangeQuery::new()
            .row_range(0, 100)
            .col_range(0, 26)
            .filter_non_empty();

        let bounds = query.bounds().unwrap();
        assert_eq!(bounds, (0, 0, 100, 26));
    }
}
