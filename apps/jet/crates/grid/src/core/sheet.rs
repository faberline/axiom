use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::core::cell::{Cell, CellContent, CellValue};
use crate::core::chunk::ChunkedGrid;
use crate::core::conditional_format::ConditionalFormattingRule;
use crate::core::format::CellFormat;
use crate::core::range::{CellCoord, CellRange};
use crate::core::spatial::SpatialIndex;
use crate::core::validation::{DataValidationRule, ValidationResult};

/// Represents a filter applied to a column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterState {
    pub col: u32,
    pub visible_values: HashSet<String>,
}

/// Default row height in pixels
pub const DEFAULT_ROW_HEIGHT: f64 = 24.0;
/// Default column width in pixels
pub const DEFAULT_COL_WIDTH: f64 = 100.0;

/// A single spreadsheet sheet with sparse storage for cells
#[derive(Debug, Clone, Serialize)]
pub struct Sheet {
    /// Sheet name (displayed in tab)
    pub name: String,
    /// Sparse storage for cells using chunked grid - only non-empty cells are stored
    #[serde(default, with = "chunked_grid_serde")]
    cells: ChunkedGrid<Cell>,
    /// Custom row heights (row index -> height in pixels) - kept for serialization
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub row_heights: HashMap<u32, f64>,
    /// Custom column widths (column index -> width in pixels) - kept for serialization
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub col_widths: HashMap<u32, f64>,
    /// Default height for rows without custom height
    #[serde(default = "default_row_height")]
    pub default_row_height: f64,
    /// Default width for columns without custom width
    #[serde(default = "default_col_width")]
    pub default_col_width: f64,
    /// Number of frozen rows (scroll lock)
    #[serde(default)]
    pub frozen_rows: u32,
    /// Number of frozen columns (scroll lock)
    #[serde(default)]
    pub frozen_cols: u32,
    /// Merged cell ranges - each range represents a merged area with top-left as master cell
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merged_ranges: Vec<CellRange>,
    /// Active column filters
    #[serde(default)]
    pub active_filters: Vec<FilterState>,
    /// Conditional formatting rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditional_formatting: Vec<ConditionalFormattingRule>,
    /// Data validation rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_validation: Vec<DataValidationRule>,
    /// Spatial index for O(log N) position lookups (rebuilt on deserialize)
    #[serde(skip, default = "SpatialIndex::new")]
    spatial: SpatialIndex,
}

fn default_row_height() -> f64 {
    DEFAULT_ROW_HEIGHT
}

fn default_col_width() -> f64 {
    DEFAULT_COL_WIDTH
}

/// Compare two CellValues for sorting purposes.
/// Ordering: Empty < Number < Text < Boolean < Error
fn compare_cell_values(a: &Option<CellValue>, b: &Option<CellValue>) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a_val), Some(b_val)) => match (a_val, b_val) {
            (CellValue::Empty, CellValue::Empty) => Ordering::Equal,
            (CellValue::Empty, _) => Ordering::Less,
            (_, CellValue::Empty) => Ordering::Greater,

            (CellValue::Number(a_num), CellValue::Number(b_num)) => {
                a_num.partial_cmp(b_num).unwrap_or(Ordering::Equal)
            }
            (CellValue::Number(_), _) => Ordering::Less,
            (_, CellValue::Number(_)) => Ordering::Greater,

            (CellValue::Text(a_txt), CellValue::Text(b_txt)) => a_txt.cmp(b_txt),
            (CellValue::Text(_), _) => Ordering::Less,
            (_, CellValue::Text(_)) => Ordering::Greater,

            (CellValue::Boolean(a_bool), CellValue::Boolean(b_bool)) => a_bool.cmp(b_bool),
            (CellValue::Boolean(_), _) => Ordering::Less,
            (_, CellValue::Boolean(_)) => Ordering::Greater,

            (CellValue::Error(_), CellValue::Error(_)) => Ordering::Equal,
        },
    }
}

/// Custom serialization for ChunkedGrid to maintain backward compatibility
mod chunked_grid_serde {
    use super::*;
    use serde::ser::SerializeMap;
    use serde::{de, Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S>(grid: &ChunkedGrid<Cell>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as a map with stringified tuple keys "row,col" for JSON compatibility
        let mut map = serializer.serialize_map(Some(grid.len()))?;
        for ((row, col), cell) in grid.iter() {
            let key = format!("{},{}", row, col);
            map.serialize_entry(&key, cell)?;
        }
        map.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ChunkedGrid<Cell>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GridVisitor;

        impl<'de> de::Visitor<'de> for GridVisitor {
            type Value = ChunkedGrid<Cell>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with coordinate keys")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                let mut grid = ChunkedGrid::new();

                while let Some(key) = map.next_key::<String>()? {
                    let cell: Cell = map.next_value()?;

                    // Parse "row,col" format
                    let parts: Vec<&str> = key.split(',').collect();
                    if parts.len() == 2 {
                        if let (Ok(row), Ok(col)) =
                            (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                        {
                            grid.insert(row as usize, col as usize, cell);
                        }
                    }
                }

                Ok(grid)
            }
        }

        deserializer.deserialize_map(GridVisitor)
    }
}

impl Sheet {
    /// Maximum number of rows (Excel compatibility)
    pub const MAX_ROWS: u32 = 1_048_576;
    /// Maximum number of columns (Column XFD)
    pub const MAX_COLS: u32 = 16_384;

    /// Create a new empty sheet with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cells: ChunkedGrid::new(),
            row_heights: HashMap::new(),
            col_widths: HashMap::new(),
            default_row_height: DEFAULT_ROW_HEIGHT,
            default_col_width: DEFAULT_COL_WIDTH,
            frozen_rows: 0,
            frozen_cols: 0,
            merged_ranges: Vec::new(),
            active_filters: Vec::new(),
            conditional_formatting: Vec::new(),
            data_validation: Vec::new(),
            spatial: SpatialIndex::new(),
        }
    }

    /// Get a reference to a cell at the given coordinate
    pub fn get_cell(&self, coord: CellCoord) -> Option<&Cell> {
        self.cells.get(coord.row as usize, coord.col as usize)
    }

    /// Get a mutable reference to a cell, creating it if it doesn't exist
    pub fn get_cell_mut(&mut self, coord: CellCoord) -> &mut Cell {
        let row = coord.row as usize;
        let col = coord.col as usize;

        // Check if cell exists, if not insert default
        if self.cells.get(row, col).is_none() {
            self.cells.insert(row, col, Cell::default());
        }

        // Now we can safely get mutable reference
        self.cells
            .get_mut(row, col)
            .expect("Cell was just inserted")
    }

    /// Set a cell at the given coordinate
    pub fn set_cell(&mut self, coord: CellCoord, cell: Cell) {
        let row = coord.row as usize;
        let col = coord.col as usize;

        if cell.is_empty() {
            // Remove empty cells to save memory
            self.cells.remove(row, col);
        } else {
            self.cells.insert(row, col, cell);
        }
    }

    /// Set the value of a cell (parses input to determine type)
    pub fn set_cell_value(&mut self, coord: CellCoord, value: &str) {
        let content = parse_cell_input(value);
        let cell = self.get_cell_mut(coord);
        cell.content = content;

        // Clean up if cell became empty
        if cell.is_empty() {
            self.cells.remove(coord.row as usize, coord.col as usize);
        }
    }

    /// Remove a cell (make it empty)
    pub fn remove_cell(&mut self, coord: CellCoord) {
        self.cells.remove(coord.row as usize, coord.col as usize);
    }

    /// Get the computed value of a cell (returns Empty for non-existent cells)
    pub fn get_cell_value(&self, coord: CellCoord) -> &CellValue {
        self.get_cell(coord)
            .map(|c| c.computed_value())
            .unwrap_or(&CellValue::Empty)
    }

    /// Get the row height for a specific row
    pub fn get_row_height(&self, row: u32) -> f64 {
        *self
            .row_heights
            .get(&row)
            .unwrap_or(&self.default_row_height)
    }

    /// Set the row height for a specific row
    pub fn set_row_height(&mut self, row: u32, height: f64) {
        if (height - self.default_row_height).abs() < 0.01 {
            self.row_heights.remove(&row);
            // Reset to default in spatial index
            self.spatial
                .set_row_height(row as usize, self.default_row_height);
        } else {
            self.row_heights.insert(row, height);
            // Update spatial index
            self.spatial.set_row_height(row as usize, height);
        }
    }

    /// Get the column width for a specific column
    pub fn get_col_width(&self, col: u32) -> f64 {
        *self.col_widths.get(&col).unwrap_or(&self.default_col_width)
    }

    /// Set the column width for a specific column
    pub fn set_col_width(&mut self, col: u32, width: f64) {
        if (width - self.default_col_width).abs() < 0.01 {
            self.col_widths.remove(&col);
            // Reset to default in spatial index
            self.spatial
                .set_col_width(col as usize, self.default_col_width);
        } else {
            self.col_widths.insert(col, width);
            // Update spatial index
            self.spatial.set_col_width(col as usize, width);
        }
    }

    /// Get the number of non-empty cells
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Check if the sheet is empty
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Get all non-empty cell coordinates
    pub fn non_empty_coords(&self) -> impl Iterator<Item = CellCoord> + '_ {
        self.cells
            .iter()
            .map(|((row, col), _)| CellCoord::new(row as u32, col as u32))
    }

    /// Get all cells in a range
    pub fn get_cells_in_range(&self, start: CellCoord, end: CellCoord) -> Vec<(CellCoord, &Cell)> {
        let min_row = start.row.min(end.row) as usize;
        let max_row = start.row.max(end.row) as usize;
        let min_col = start.col.min(end.col) as usize;
        let max_col = start.col.max(end.col) as usize;

        self.cells
            .cells_in_range(min_row, min_col, max_row, max_col)
            .into_iter()
            .map(|((row, col), cell)| (CellCoord::new(row as u32, col as u32), cell))
            .collect()
    }

    /// Get the bounding box of non-empty cells
    pub fn used_range(&self) -> Option<(CellCoord, CellCoord)> {
        if self.cells.is_empty() {
            return None;
        }

        let mut min_row = usize::MAX;
        let mut max_row = 0;
        let mut min_col = usize::MAX;
        let mut max_col = 0;

        for ((row, col), _) in self.cells.iter() {
            min_row = min_row.min(row);
            max_row = max_row.max(row);
            min_col = min_col.min(col);
            max_col = max_col.max(col);
        }

        Some((
            CellCoord::new(min_row as u32, min_col as u32),
            CellCoord::new(max_row as u32, max_col as u32),
        ))
    }

    /// Calculate Y position of a row (sum of heights of all rows above)
    pub fn row_y_position(&self, row: u32) -> f64 {
        self.spatial.get_row_offset(row as usize)
    }

    /// Calculate X position of a column (sum of widths of all columns before)
    pub fn col_x_position(&self, col: u32) -> f64 {
        self.spatial.get_col_offset(col as usize)
    }

    /// Find row at given Y position
    pub fn row_at_y(&self, y: f64) -> u32 {
        self.spatial.find_row_at_offset(y) as u32
    }

    /// Find column at given X position
    pub fn col_at_x(&self, x: f64) -> u32 {
        self.spatial.find_col_at_offset(x) as u32
    }

    /// Rebuild the spatial index from row_heights and col_widths
    /// Called after deserialization to reconstruct the runtime index
    fn rebuild_spatial_index(&mut self) {
        self.spatial = SpatialIndex::new();

        // Apply custom row heights
        for (&row, &height) in &self.row_heights {
            self.spatial.set_row_height(row as usize, height);
        }

        // Apply custom column widths
        for (&col, &width) in &self.col_widths {
            self.spatial.set_col_width(col as usize, width);
        }
    }

    /// Helper to shift HashMap keys by a delta (positive or negative).
    /// Keys >= `from` are shifted by `delta`.
    fn shift_dimension_keys(map: &mut HashMap<u32, f64>, from: u32, delta: i32) {
        if delta == 0 {
            return;
        }

        let mut new_map = HashMap::new();

        for (&key, &value) in map.iter() {
            if key >= from {
                if delta > 0 {
                    // Shift right/down
                    new_map.insert(key + delta as u32, value);
                } else {
                    // Shift left/up
                    let shift_amount = (-delta) as u32;
                    if key >= shift_amount {
                        new_map.insert(key - shift_amount, value);
                    }
                    // If key < shift_amount, it would go negative, so skip it
                }
            } else {
                new_map.insert(key, value);
            }
        }

        *map = new_map;
    }

    /// Insert rows at the given position, shifting existing rows down.
    /// Returns the list of cells that were shifted (old_coord, new_coord).
    ///
    /// # Arguments
    /// * `at_row` - The row index where new rows should be inserted
    /// * `count` - The number of rows to insert
    pub fn insert_rows(&mut self, at_row: u32, count: u32) -> Vec<(CellCoord, CellCoord)> {
        if count == 0 {
            return Vec::new();
        }

        // Shift cells in the ChunkedGrid
        let shifts = self.cells.shift_rows_down(at_row as usize, count as usize);

        // Update spatial index
        self.spatial.insert_rows(at_row as usize, count as usize);

        // Shift row heights HashMap
        Self::shift_dimension_keys(&mut self.row_heights, at_row, count as i32);

        // Update spatial index with shifted row heights
        for (&row, &height) in &self.row_heights {
            self.spatial.set_row_height(row as usize, height);
        }

        // Adjust merged ranges
        self.adjust_merges_on_row_insert(at_row, count);

        // Convert shifts to CellCoord
        shifts
            .into_iter()
            .map(|((old_row, old_col), (new_row, new_col))| {
                (
                    CellCoord::new(old_row as u32, old_col as u32),
                    CellCoord::new(new_row as u32, new_col as u32),
                )
            })
            .collect()
    }

    /// Delete rows at the given position, shifting remaining rows up.
    /// Returns the deleted cells.
    ///
    /// # Arguments
    /// * `at_row` - The row index where deletion should start
    /// * `count` - The number of rows to delete
    pub fn delete_rows(&mut self, at_row: u32, count: u32) -> Vec<(CellCoord, Cell)> {
        if count == 0 {
            return Vec::new();
        }

        let end_row = at_row + count;

        // Shift cells in the ChunkedGrid
        let (deleted_cells, _shifts) = self.cells.shift_rows_up(at_row as usize, count as usize);

        // Update spatial index
        self.spatial.delete_rows(at_row as usize, count as usize);

        // Remove row heights in deleted range and shift remaining
        let mut new_row_heights = HashMap::new();
        for (&row, &height) in &self.row_heights {
            if row < at_row {
                new_row_heights.insert(row, height);
            } else if row >= end_row {
                new_row_heights.insert(row - count, height);
            }
            // Rows in [at_row, end_row) are deleted
        }
        self.row_heights = new_row_heights;

        // Update spatial index with new row heights
        for (&row, &height) in &self.row_heights {
            self.spatial.set_row_height(row as usize, height);
        }

        // Adjust merged ranges
        self.adjust_merges_on_row_delete(at_row, count);

        // Convert deleted cells to CellCoord
        deleted_cells
            .into_iter()
            .map(|((row, col), cell)| (CellCoord::new(row as u32, col as u32), cell))
            .collect()
    }

    /// Insert columns at the given position, shifting existing columns right.
    /// Returns the list of cells that were shifted (old_coord, new_coord).
    ///
    /// # Arguments
    /// * `at_col` - The column index where new columns should be inserted
    /// * `count` - The number of columns to insert
    pub fn insert_cols(&mut self, at_col: u32, count: u32) -> Vec<(CellCoord, CellCoord)> {
        if count == 0 {
            return Vec::new();
        }

        // Shift cells in the ChunkedGrid
        let shifts = self.cells.shift_cols_right(at_col as usize, count as usize);

        // Update spatial index
        self.spatial.insert_cols(at_col as usize, count as usize);

        // Shift column widths HashMap
        Self::shift_dimension_keys(&mut self.col_widths, at_col, count as i32);

        // Update spatial index with shifted column widths
        for (&col, &width) in &self.col_widths {
            self.spatial.set_col_width(col as usize, width);
        }

        // Adjust merged ranges
        self.adjust_merges_on_col_insert(at_col, count);

        // Convert shifts to CellCoord
        shifts
            .into_iter()
            .map(|((old_row, old_col), (new_row, new_col))| {
                (
                    CellCoord::new(old_row as u32, old_col as u32),
                    CellCoord::new(new_row as u32, new_col as u32),
                )
            })
            .collect()
    }

    /// Sort rows in a range by a specific column.
    /// Returns the original row order (for undo purposes).
    ///
    /// # Arguments
    /// * `start_row` - First row of the range to sort
    /// * `end_row` - Last row of the range to sort (inclusive)
    /// * `start_col` - First column of the range
    /// * `end_col` - Last column of the range (inclusive)
    /// * `sort_col` - Column to sort by
    /// * `ascending` - True for ascending, false for descending
    ///
    /// # Returns
    /// Vec of (original_row_index, sorted_row_index) pairs for undo
    pub fn sort_range(
        &mut self,
        start_row: u32,
        end_row: u32,
        start_col: u32,
        end_col: u32,
        sort_col: u32,
        ascending: bool,
    ) -> Vec<(u32, u32)> {
        if start_row >= end_row || sort_col < start_col || sort_col > end_col {
            return Vec::new();
        }

        // Block sort if merged regions overlap the sort range
        if self.has_merge_in_range(start_row, end_row, start_col, end_col) {
            return Vec::new();
        }

        let num_rows = (end_row - start_row + 1) as usize;

        // Collect row data: (original_row_index, sort_value, all cells in row)
        #[allow(clippy::type_complexity)]
        let mut row_data: Vec<(u32, Option<CellValue>, Vec<(u32, Cell)>)> =
            Vec::with_capacity(num_rows);

        for row in start_row..=end_row {
            // Get sort value for this row
            let sort_value = self
                .get_cell(CellCoord::new(row, sort_col))
                .map(|c| c.computed_value().clone());

            // Collect all cells in this row within the column range
            let mut row_cells: Vec<(u32, Cell)> = Vec::new();
            for col in start_col..=end_col {
                if let Some(cell) = self.get_cell(CellCoord::new(row, col)) {
                    row_cells.push((col, cell.clone()));
                }
            }

            row_data.push((row, sort_value, row_cells));
        }

        // Sort by the sort_value
        row_data.sort_by(|a, b| {
            let cmp = compare_cell_values(&a.1, &b.1);
            if ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });

        // Build mapping: new_position -> original_row
        let row_mapping: Vec<(u32, u32)> = row_data
            .iter()
            .enumerate()
            .map(|(new_idx, (orig_row, _, _))| (*orig_row, start_row + new_idx as u32))
            .collect();

        // Clear all cells in the range
        for row in start_row..=end_row {
            for col in start_col..=end_col {
                self.cells.remove(row as usize, col as usize);
            }
        }

        // Reinsert cells in sorted order
        for (new_idx, (_, _, cells)) in row_data.into_iter().enumerate() {
            let new_row = start_row + new_idx as u32;
            for (col, cell) in cells {
                if !cell.is_empty() {
                    self.cells.insert(new_row as usize, col as usize, cell);
                }
            }
        }

        // Also sort row heights if any are custom
        let mut height_changes: Vec<(u32, f64)> = Vec::new();
        for &(orig_row, new_row) in &row_mapping {
            if let Some(&height) = self.row_heights.get(&orig_row) {
                height_changes.push((new_row, height));
            }
        }

        // Clear old row heights in range
        for row in start_row..=end_row {
            self.row_heights.remove(&row);
        }

        // Apply new row heights
        for (row, height) in height_changes {
            self.row_heights.insert(row, height);
            self.spatial.set_row_height(row as usize, height);
        }

        row_mapping
    }

    /// Restore row order from a previous sort operation (for undo).
    /// Takes the mapping returned by sort_range and reverses it.
    pub fn unsort_range(
        &mut self,
        start_row: u32,
        end_row: u32,
        start_col: u32,
        end_col: u32,
        original_mapping: &[(u32, u32)],
    ) {
        // Create reverse mapping: current_row -> original_row
        let mut reverse_mapping: Vec<(u32, u32)> = original_mapping
            .iter()
            .map(|&(orig, curr)| (curr, orig))
            .collect();
        reverse_mapping.sort_by_key(|&(curr, _)| curr);

        // Collect current row data
        let mut row_data: Vec<(u32, Vec<(u32, Cell)>)> = Vec::new();
        for (curr_row, orig_row) in &reverse_mapping {
            let mut row_cells: Vec<(u32, Cell)> = Vec::new();
            for col in start_col..=end_col {
                if let Some(cell) = self.get_cell(CellCoord::new(*curr_row, col)) {
                    row_cells.push((col, cell.clone()));
                }
            }
            row_data.push((*orig_row, row_cells));
        }

        // Clear all cells in the range
        for row in start_row..=end_row {
            for col in start_col..=end_col {
                self.cells.remove(row as usize, col as usize);
            }
        }

        // Reinsert cells at original positions
        for (orig_row, cells) in row_data {
            for (col, cell) in cells {
                if !cell.is_empty() {
                    self.cells.insert(orig_row as usize, col as usize, cell);
                }
            }
        }
    }

    /// Delete columns at the given position, shifting remaining columns left.
    /// Returns the deleted cells.
    ///
    /// # Arguments
    /// * `at_col` - The column index where deletion should start
    /// * `count` - The number of columns to delete
    pub fn delete_cols(&mut self, at_col: u32, count: u32) -> Vec<(CellCoord, Cell)> {
        if count == 0 {
            return Vec::new();
        }

        let end_col = at_col + count;

        // Shift cells in the ChunkedGrid
        let (deleted_cells, _shifts) = self.cells.shift_cols_left(at_col as usize, count as usize);

        // Update spatial index
        self.spatial.delete_cols(at_col as usize, count as usize);

        // Remove column widths in deleted range and shift remaining
        let mut new_col_widths = HashMap::new();
        for (&col, &width) in &self.col_widths {
            if col < at_col {
                new_col_widths.insert(col, width);
            } else if col >= end_col {
                new_col_widths.insert(col - count, width);
            }
            // Columns in [at_col, end_col) are deleted
        }
        self.col_widths = new_col_widths;

        // Update spatial index with new column widths
        for (&col, &width) in &self.col_widths {
            self.spatial.set_col_width(col as usize, width);
        }

        // Adjust merged ranges
        self.adjust_merges_on_col_delete(at_col, count);

        // Convert deleted cells to CellCoord
        deleted_cells
            .into_iter()
            .map(|((row, col), cell)| (CellCoord::new(row as u32, col as u32), cell))
            .collect()
    }

    // =========================================================================
    // Merge Range Adjustment on Row/Column Operations
    // =========================================================================

    /// Adjust merged ranges when rows are inserted.
    /// Merges below insertion shift down; merges spanning the insertion expand.
    fn adjust_merges_on_row_insert(&mut self, at_row: u32, count: u32) {
        for merge in &mut self.merged_ranges {
            if merge.start.row >= at_row {
                // Entire merge is at or below insertion: shift down
                merge.start.row += count;
                merge.end.row += count;
            } else if merge.end.row >= at_row {
                // Merge spans the insertion point: expand
                merge.end.row += count;
            }
        }
    }

    /// Adjust merged ranges when rows are deleted.
    /// Merges fully inside deletion are removed; partial overlaps shrink; below shifts up.
    fn adjust_merges_on_row_delete(&mut self, at_row: u32, count: u32) {
        let end_row = at_row + count;
        self.merged_ranges.retain_mut(|merge| {
            if merge.start.row >= end_row {
                // Entire merge is below deletion: shift up
                merge.start.row -= count;
                merge.end.row -= count;
                true
            } else if merge.end.row < at_row {
                // Entire merge is above deletion: no change
                true
            } else if merge.start.row >= at_row && merge.end.row < end_row {
                // Merge fully inside deletion: remove
                false
            } else {
                // Partial overlap: clamp and shift
                let overlap_start = merge.start.row.max(at_row);
                let overlap_end = merge.end.row.min(end_row - 1);
                let overlap = overlap_end - overlap_start + 1;
                if merge.start.row >= at_row {
                    merge.start.row = at_row;
                }
                merge.end.row -= overlap;
                // Remove if merge collapsed to single cell
                merge.start.row != merge.end.row || merge.start.col != merge.end.col
            }
        });
    }

    /// Adjust merged ranges when columns are inserted.
    fn adjust_merges_on_col_insert(&mut self, at_col: u32, count: u32) {
        for merge in &mut self.merged_ranges {
            if merge.start.col >= at_col {
                merge.start.col += count;
                merge.end.col += count;
            } else if merge.end.col >= at_col {
                merge.end.col += count;
            }
        }
    }

    /// Adjust merged ranges when columns are deleted.
    fn adjust_merges_on_col_delete(&mut self, at_col: u32, count: u32) {
        let end_col = at_col + count;
        self.merged_ranges.retain_mut(|merge| {
            if merge.start.col >= end_col {
                merge.start.col -= count;
                merge.end.col -= count;
                true
            } else if merge.end.col < at_col {
                true
            } else if merge.start.col >= at_col && merge.end.col < end_col {
                false
            } else {
                let overlap_start = merge.start.col.max(at_col);
                let overlap_end = merge.end.col.min(end_col - 1);
                let overlap = overlap_end - overlap_start + 1;
                if merge.start.col >= at_col {
                    merge.start.col = at_col;
                }
                merge.end.col -= overlap;
                merge.start.col != merge.end.col || merge.start.row != merge.end.row
            }
        });
    }

    /// Check if any merged regions overlap with the given range.
    /// Used to block sort operations on merged ranges.
    pub fn has_merge_in_range(
        &self,
        start_row: u32,
        end_row: u32,
        start_col: u32,
        end_col: u32,
    ) -> bool {
        self.merged_ranges.iter().any(|merge| {
            merge.start.row <= end_row
                && merge.end.row >= start_row
                && merge.start.col <= end_col
                && merge.end.col >= start_col
        })
    }

    // =========================================================================
    // Cell Merging
    // =========================================================================

    /// Merge cells in the given range.
    /// The top-left cell becomes the "master" cell that holds the value.
    /// Returns true if merge was successful, false if range overlaps existing merge.
    pub fn merge_cells(&mut self, range: CellRange) -> bool {
        // Don't merge single cells
        if range.is_single_cell() {
            return false;
        }

        // Check for overlap with existing merges
        for existing in &self.merged_ranges {
            if existing.intersects(&range) {
                return false;
            }
        }

        // Move all non-empty cell values to the master cell (top-left)
        let master = range.start;
        let mut master_value: Option<CellContent> = None;

        // Find first non-empty cell in the range to use as master value
        for coord in range.iter() {
            if let Some(cell) = self.get_cell(coord) {
                if !cell.content.is_empty() {
                    if master_value.is_none() {
                        master_value = Some(cell.content.clone());
                    }
                    // Clear non-master cells
                    if coord != master {
                        self.remove_cell(coord);
                    }
                }
            }
        }

        // Set master cell value if we found one
        if let Some(content) = master_value {
            let cell = self.get_cell_mut(master);
            cell.content = content;
        }

        self.merged_ranges.push(range);
        true
    }

    /// Unmerge cells in the given range.
    /// Returns true if a merge was found and removed.
    pub fn unmerge_cells(&mut self, range: CellRange) -> bool {
        // Find and remove merge that matches or contains this range
        if let Some(idx) = self
            .merged_ranges
            .iter()
            .position(|r| r == &range || r.contains(range.start))
        {
            self.merged_ranges.remove(idx);
            true
        } else {
            false
        }
    }

    /// Get the merge range containing the given coordinate, if any.
    pub fn get_merge_at(&self, coord: CellCoord) -> Option<&CellRange> {
        self.merged_ranges.iter().find(|r| r.contains(coord))
    }

    /// Check if a coordinate is inside a merged range but not the master cell.
    pub fn is_merged_slave(&self, coord: CellCoord) -> bool {
        self.merged_ranges
            .iter()
            .any(|r| r.contains(coord) && r.start != coord)
    }

    /// Get the master cell coordinate for a merged cell.
    /// Returns Some(master) if the coord is in a merged range, None otherwise.
    pub fn get_master_cell(&self, coord: CellCoord) -> Option<CellCoord> {
        self.get_merge_at(coord).map(|r| r.start)
    }

    /// Get all merged ranges.
    pub fn get_merged_ranges(&self) -> &[CellRange] {
        &self.merged_ranges
    }

    /// Check if a range would overlap with any existing merged ranges.
    pub fn would_overlap_merge(&self, range: &CellRange) -> bool {
        self.merged_ranges.iter().any(|r| r.intersects(range))
    }

    // =========================================================================
    // Filtering
    // =========================================================================

    /// Hide specific rows
    pub fn hide_rows(&mut self, rows: &[u32]) {
        for &row in rows {
            self.spatial.hide_row(row as usize);
        }
    }

    /// Show (unhide) specific rows
    pub fn show_rows(&mut self, rows: &[u32]) {
        for &row in rows {
            self.spatial.unhide_row(row as usize);
        }
    }

    /// Check if a row is hidden
    pub fn is_row_hidden(&self, row: u32) -> bool {
        self.spatial.is_row_hidden(row as usize)
    }

    /// Get all hidden rows
    pub fn get_hidden_rows(&self) -> Vec<u32> {
        self.spatial
            .get_hidden_rows()
            .iter()
            .map(|&r| r as u32)
            .collect()
    }

    /// Get unique values in a column (for filter dropdown)
    pub fn get_unique_values_in_column(&self, col: u32, max_rows: u32) -> Vec<String> {
        let mut values = HashSet::new();
        for row in 0..max_rows {
            if let Some(cell) = self.get_cell(CellCoord::new(row, col)) {
                let value = cell.computed_value().as_text();
                if !value.is_empty() {
                    values.insert(value);
                }
            }
        }
        let mut result: Vec<String> = values.into_iter().collect();
        result.sort();
        result
    }

    /// Apply a column filter - hide rows where cell value not in visible_values
    /// Returns the rows that were hidden
    pub fn apply_column_filter(
        &mut self,
        col: u32,
        visible_values: &HashSet<String>,
        max_rows: u32,
    ) -> Vec<u32> {
        let mut rows_to_hide = Vec::new();

        for row in 0..max_rows {
            let value = self
                .get_cell(CellCoord::new(row, col))
                .map(|c| c.computed_value().as_text())
                .unwrap_or_default();

            // If cell value is not in visible set, hide the row
            // Empty cells are hidden unless empty string is in visible_values
            if !visible_values.contains(&value) {
                rows_to_hide.push(row);
            }
        }

        // Hide the rows
        self.hide_rows(&rows_to_hide);

        // Store the filter state
        self.active_filters.retain(|f| f.col != col);
        self.active_filters.push(FilterState {
            col,
            visible_values: visible_values.clone(),
        });

        rows_to_hide
    }

    /// Clear filter on a specific column
    /// Returns the rows that were unhidden
    pub fn clear_column_filter(&mut self, col: u32) -> Vec<u32> {
        let hidden_before: HashSet<u32> = self.get_hidden_rows().into_iter().collect();

        // Remove the filter
        self.active_filters.retain(|f| f.col != col);

        // Show all rows (we'll re-hide based on remaining filters)
        for row in hidden_before.iter() {
            self.show_rows(&[*row]);
        }

        // Re-apply remaining filters
        // This is needed because multiple filters can hide the same row
        let rows_still_hidden = self.reapply_all_filters();

        // Return rows that were unhidden
        hidden_before
            .difference(&rows_still_hidden.into_iter().collect())
            .copied()
            .collect()
    }

    /// Clear all filters
    pub fn clear_all_filters(&mut self) -> Vec<u32> {
        let hidden_rows = self.get_hidden_rows();

        // Show all rows
        self.show_rows(&hidden_rows);

        // Clear all filter state
        self.active_filters.clear();

        hidden_rows
    }

    /// Re-apply all active filters (used after clearing one filter)
    fn reapply_all_filters(&mut self) -> Vec<u32> {
        let filters = self.active_filters.clone();
        self.active_filters.clear();

        let mut all_hidden = HashSet::new();
        for filter in filters {
            let hidden = self.apply_column_filter(filter.col, &filter.visible_values, 10000);
            all_hidden.extend(hidden);
        }

        all_hidden.into_iter().collect()
    }

    /// Get active filters
    pub fn get_active_filters(&self) -> &[FilterState] {
        &self.active_filters
    }

    // =========================================================================
    // Conditional Formatting
    // =========================================================================

    /// Add a conditional formatting rule
    pub fn add_conditional_formatting(&mut self, rule: ConditionalFormattingRule) {
        self.conditional_formatting.push(rule);
        self.conditional_formatting.sort_by_key(|r| r.priority);
    }

    /// Remove a conditional formatting rule by ID
    /// Returns true if a rule was removed
    pub fn remove_conditional_formatting(&mut self, rule_id: &str) -> bool {
        let len_before = self.conditional_formatting.len();
        self.conditional_formatting.retain(|r| r.id != rule_id);
        self.conditional_formatting.len() < len_before
    }

    /// Get all conditional formatting rules
    pub fn get_conditional_formatting_rules(&self) -> &[ConditionalFormattingRule] {
        &self.conditional_formatting
    }

    /// Get the effective format for a cell, applying conditional formatting rules
    /// This combines the base format with any matching conditional formats
    pub fn get_effective_format(
        &self,
        row: u32,
        col: u32,
        base_format: &CellFormat,
        value: &CellValue,
    ) -> CellFormat {
        let mut result = base_format.clone();
        let coord = CellCoord::new(row, col);

        // Calculate min/max for color scales (could be optimized with caching)
        let (min_val, max_val) = self.calculate_range_min_max();

        for rule in &self.conditional_formatting {
            if !rule.enabled || !rule.range.contains(coord) {
                continue;
            }

            if let Some(fmt) = rule.rule.evaluate(value, min_val, max_val) {
                result = fmt.apply_to(&result);
            }
        }

        result
    }

    /// Calculate the min and max numeric values in the sheet
    /// Used for color scale calculations
    fn calculate_range_min_max(&self) -> (f64, f64) {
        let mut min = f64::MAX;
        let mut max = f64::MIN;
        let mut found_any = false;

        for ((_, _), cell) in self.cells.iter() {
            if let CellValue::Number(n) = cell.content.computed_value() {
                min = min.min(*n);
                max = max.max(*n);
                found_any = true;
            }
        }

        if !found_any || min > max {
            (0.0, 0.0)
        } else {
            (min, max)
        }
    }

    // =========================================================================
    // Data Validation
    // =========================================================================

    /// Add a data validation rule
    /// If a rule with the same ID exists, it will be replaced
    pub fn add_data_validation(&mut self, rule: DataValidationRule) {
        // Remove existing rule with same ID if exists
        self.data_validation.retain(|r| r.id != rule.id);
        self.data_validation.push(rule);
    }

    /// Remove a data validation rule by ID
    /// Returns true if a rule was removed
    pub fn remove_data_validation(&mut self, rule_id: &str) -> bool {
        let len_before = self.data_validation.len();
        self.data_validation.retain(|r| r.id != rule_id);
        self.data_validation.len() < len_before
    }

    /// Get the validation rule that applies to a specific cell
    /// Returns the first enabled rule whose range contains the cell
    pub fn get_validation_rule(&self, row: u32, col: u32) -> Option<&DataValidationRule> {
        let coord = CellCoord::new(row, col);
        self.data_validation
            .iter()
            .find(|r| r.enabled && r.range.contains(coord))
    }

    /// Validate a cell value at the given position
    /// Returns Valid if no rule applies or validation passes
    pub fn validate_cell_value(&self, row: u32, col: u32, value: &CellValue) -> ValidationResult {
        if let Some(rule) = self.get_validation_rule(row, col) {
            rule.validate(value)
        } else {
            ValidationResult::Valid
        }
    }

    /// Get dropdown items for a cell (if it has a list validation with static values)
    /// Returns None if the cell has no validation, or if validation is not a dropdown,
    /// or if the dropdown uses a range reference (needs context)
    pub fn get_cell_dropdown_items(&self, row: u32, col: u32) -> Option<Vec<String>> {
        self.get_validation_rule(row, col)
            .and_then(|rule| rule.get_dropdown_items())
    }

    /// Get all data validation rules
    pub fn get_data_validation_rules(&self) -> &[DataValidationRule] {
        &self.data_validation
    }
}

// Custom Deserialize implementation to rebuild spatial index after deserialization
impl<'de> Deserialize<'de> for Sheet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Helper struct with same fields for deserialization
        #[derive(Deserialize)]
        struct SheetHelper {
            name: String,
            #[serde(default, with = "chunked_grid_serde")]
            cells: ChunkedGrid<Cell>,
            #[serde(default)]
            row_heights: HashMap<u32, f64>,
            #[serde(default)]
            col_widths: HashMap<u32, f64>,
            #[serde(default = "default_row_height")]
            default_row_height: f64,
            #[serde(default = "default_col_width")]
            default_col_width: f64,
            #[serde(default)]
            frozen_rows: u32,
            #[serde(default)]
            frozen_cols: u32,
            #[serde(default)]
            merged_ranges: Vec<CellRange>,
            #[serde(default)]
            active_filters: Vec<FilterState>,
            #[serde(default)]
            conditional_formatting: Vec<ConditionalFormattingRule>,
            #[serde(default)]
            data_validation: Vec<DataValidationRule>,
        }

        let helper = SheetHelper::deserialize(deserializer)?;

        let mut sheet = Sheet {
            name: helper.name,
            cells: helper.cells,
            row_heights: helper.row_heights,
            col_widths: helper.col_widths,
            default_row_height: helper.default_row_height,
            default_col_width: helper.default_col_width,
            frozen_rows: helper.frozen_rows,
            frozen_cols: helper.frozen_cols,
            merged_ranges: helper.merged_ranges,
            active_filters: helper.active_filters,
            conditional_formatting: helper.conditional_formatting,
            data_validation: helper.data_validation,
            spatial: SpatialIndex::new(),
        };

        // Rebuild the spatial index from the deserialized data
        sheet.rebuild_spatial_index();

        Ok(sheet)
    }
}

/// Parse user input to determine cell content type
pub fn parse_cell_input(input: &str) -> CellContent {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return CellContent::Value {
            value: CellValue::Empty,
            original_input: None,
        };
    }

    // Formulas: store expression
    if trimmed.starts_with('=') {
        return CellContent::Formula {
            expression: trimmed.to_string(),
            cached_value: CellValue::Empty,
        };
    }

    // Store original input for all value types
    let original = trimmed.to_string();

    // Boolean
    match trimmed.to_uppercase().as_str() {
        "TRUE" => {
            return CellContent::Value {
                value: CellValue::Boolean(true),
                original_input: Some(original),
            }
        }
        "FALSE" => {
            return CellContent::Value {
                value: CellValue::Boolean(false),
                original_input: Some(original),
            }
        }
        _ => {}
    }

    // Number
    if let Ok(num) = trimmed.parse::<f64>() {
        return CellContent::Value {
            value: CellValue::Number(num),
            original_input: Some(original),
        };
    }

    // Percentage
    if let Some(stripped) = trimmed.strip_suffix('%') {
        if let Ok(num) = stripped.parse::<f64>() {
            return CellContent::Value {
                value: CellValue::Number(num / 100.0),
                original_input: Some(original),
            };
        }
    }

    // Text
    CellContent::Value {
        value: CellValue::Text(trimmed.to_string()),
        original_input: Some(original),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sheet_basic_operations() {
        let mut sheet = Sheet::new("Test");

        // Set a cell
        let coord = CellCoord::new(0, 0);
        sheet.set_cell(coord, Cell::number(42.0));

        // Get the cell
        let cell = sheet.get_cell(coord).unwrap();
        assert_eq!(cell.computed_value().as_number(), Some(42.0));

        // Remove the cell
        sheet.remove_cell(coord);
        assert!(sheet.get_cell(coord).is_none());
    }

    #[test]
    fn test_parse_cell_input() {
        // Number
        let content = parse_cell_input("42");
        assert!(
            matches!(content, CellContent::Value { value: CellValue::Number(n), .. } if n == 42.0)
        );

        // Float
        let content = parse_cell_input("3.14");
        assert!(
            matches!(content, CellContent::Value { value: CellValue::Number(n), .. } if (n - 3.14).abs() < 0.001)
        );

        // Boolean
        let content = parse_cell_input("TRUE");
        assert!(matches!(
            content,
            CellContent::Value {
                value: CellValue::Boolean(true),
                ..
            }
        ));

        // Formula
        let content = parse_cell_input("=SUM(A1:A10)");
        assert!(
            matches!(content, CellContent::Formula { expression, .. } if expression == "=SUM(A1:A10)")
        );

        // Text
        let content = parse_cell_input("Hello");
        assert!(
            matches!(content, CellContent::Value { value: CellValue::Text(s), .. } if s == "Hello")
        );

        // Percentage
        let content = parse_cell_input("50%");
        assert!(
            matches!(content, CellContent::Value { value: CellValue::Number(n), .. } if (n - 0.5).abs() < 0.001)
        );
    }

    #[test]
    fn test_row_col_dimensions() {
        let mut sheet = Sheet::new("Test");

        // Default dimensions
        assert_eq!(sheet.get_row_height(0), DEFAULT_ROW_HEIGHT);
        assert_eq!(sheet.get_col_width(0), DEFAULT_COL_WIDTH);

        // Custom dimensions
        sheet.set_row_height(5, 30.0);
        sheet.set_col_width(3, 150.0);

        assert_eq!(sheet.get_row_height(5), 30.0);
        assert_eq!(sheet.get_col_width(3), 150.0);
    }

    #[test]
    fn test_used_range() {
        let mut sheet = Sheet::new("Test");

        assert!(sheet.used_range().is_none());

        sheet.set_cell(CellCoord::new(1, 1), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(5, 10), Cell::number(2.0));

        let (start, end) = sheet.used_range().unwrap();
        assert_eq!(start, CellCoord::new(1, 1));
        assert_eq!(end, CellCoord::new(5, 10));
    }

    #[test]
    fn test_spatial_index_performance() {
        let mut sheet = Sheet::new("Test");

        // Set custom row heights
        sheet.set_row_height(0, 30.0);
        sheet.set_row_height(1, 40.0);
        sheet.set_row_height(2, 50.0);

        // Test row position lookups (O(log N) with spatial index)
        assert_eq!(sheet.row_y_position(0), 0.0);
        assert_eq!(sheet.row_y_position(1), 30.0);
        assert_eq!(sheet.row_y_position(2), 70.0);
        assert_eq!(sheet.row_y_position(3), 120.0);

        // Test finding row at Y position
        assert_eq!(sheet.row_at_y(0.0), 0);
        assert_eq!(sheet.row_at_y(29.0), 0);
        assert_eq!(sheet.row_at_y(30.0), 0);
        assert_eq!(sheet.row_at_y(31.0), 1);
        assert_eq!(sheet.row_at_y(69.0), 1);
        assert_eq!(sheet.row_at_y(70.0), 1);
        assert_eq!(sheet.row_at_y(71.0), 2);

        // Set custom column widths
        sheet.set_col_width(0, 100.0);
        sheet.set_col_width(1, 150.0);
        sheet.set_col_width(2, 200.0);

        // Test column position lookups
        assert_eq!(sheet.col_x_position(0), 0.0);
        assert_eq!(sheet.col_x_position(1), 100.0);
        assert_eq!(sheet.col_x_position(2), 250.0);
        assert_eq!(sheet.col_x_position(3), 450.0);

        // Test finding column at X position
        assert_eq!(sheet.col_at_x(0.0), 0);
        assert_eq!(sheet.col_at_x(99.0), 0);
        assert_eq!(sheet.col_at_x(100.0), 0);
        assert_eq!(sheet.col_at_x(101.0), 1);
        assert_eq!(sheet.col_at_x(249.0), 1);
        assert_eq!(sheet.col_at_x(250.0), 1);
        assert_eq!(sheet.col_at_x(251.0), 2);
    }

    #[test]
    fn test_chunked_grid_integration() {
        let mut sheet = Sheet::new("Test");

        // Insert cells in different chunks
        for row in 0..100 {
            for col in 0..100 {
                if (row + col) % 10 == 0 {
                    sheet.set_cell(
                        CellCoord::new(row, col),
                        Cell::number(row as f64 + col as f64),
                    );
                }
            }
        }

        // Verify cell count
        let expected_count = (0..100)
            .flat_map(|r| (0..100).map(move |c| (r, c)))
            .filter(|(r, c)| (r + c) % 10 == 0)
            .count();
        assert_eq!(sheet.cell_count(), expected_count);

        // Test range queries (should be efficient with chunked storage)
        let cells = sheet.get_cells_in_range(CellCoord::new(10, 10), CellCoord::new(30, 30));
        assert!(!cells.is_empty());

        // Verify cell retrieval
        assert_eq!(
            sheet
                .get_cell(CellCoord::new(10, 10))
                .unwrap()
                .computed_value()
                .as_number(),
            Some(20.0)
        );

        // Test removal
        sheet.remove_cell(CellCoord::new(10, 10));
        assert!(sheet.get_cell(CellCoord::new(10, 10)).is_none());
    }

    #[test]
    fn test_spatial_index_rebuild() {
        // Test that spatial index is properly rebuilt with custom dimensions
        let mut sheet = Sheet::new("Test");

        sheet.set_row_height(0, 30.0);
        sheet.set_row_height(5, 50.0);
        sheet.set_col_width(0, 150.0);
        sheet.set_col_width(3, 200.0);

        // Verify spatial index works correctly with custom dimensions
        assert_eq!(sheet.row_y_position(0), 0.0);
        assert_eq!(sheet.row_y_position(1), 30.0);
        assert_eq!(sheet.row_y_position(5), 30.0 + 24.0 * 4.0); // 30 + 4 default rows
        assert_eq!(sheet.row_y_position(6), 30.0 + 24.0 * 4.0 + 50.0);

        assert_eq!(sheet.col_x_position(0), 0.0);
        assert_eq!(sheet.col_x_position(1), 150.0);
        assert_eq!(sheet.col_x_position(3), 150.0 + 100.0 * 2.0); // 150 + 2 default cols
        assert_eq!(sheet.col_x_position(4), 150.0 + 100.0 * 2.0 + 200.0);

        // Test row/col finding
        assert_eq!(sheet.row_at_y(0.0), 0);
        assert_eq!(sheet.row_at_y(30.0), 0);
        assert_eq!(sheet.row_at_y(31.0), 1);

        assert_eq!(sheet.col_at_x(0.0), 0);
        assert_eq!(sheet.col_at_x(150.0), 0);
        assert_eq!(sheet.col_at_x(151.0), 1);
    }

    #[test]
    fn test_insert_rows_shifts_cells() {
        let mut sheet = Sheet::new("Test");

        // Insert some cells
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(1, 0), Cell::number(2.0));
        sheet.set_cell(CellCoord::new(2, 0), Cell::number(3.0));
        sheet.set_cell(CellCoord::new(1, 1), Cell::text("B2"));

        // Insert 2 rows at row 1
        let shifts = sheet.insert_rows(1, 2);

        // Row 0 should stay in place
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );

        // Rows 1 and 2 should be empty (gap)
        assert!(sheet.get_cell(CellCoord::new(1, 0)).is_none());
        assert!(sheet.get_cell(CellCoord::new(2, 0)).is_none());

        // Row 1 content should shift to row 3
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(3, 0)).as_number(),
            Some(2.0)
        );
        assert_eq!(sheet.get_cell_value(CellCoord::new(3, 1)).as_text(), "B2");

        // Row 2 content should shift to row 4
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(4, 0)).as_number(),
            Some(3.0)
        );

        // Check shifts return value
        assert_eq!(shifts.len(), 3); // 3 cells were shifted
    }

    #[test]
    fn test_delete_rows_removes_and_shifts() {
        let mut sheet = Sheet::new("Test");

        // Insert some cells
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(1, 0), Cell::number(2.0));
        sheet.set_cell(CellCoord::new(2, 0), Cell::number(3.0));
        sheet.set_cell(CellCoord::new(3, 0), Cell::number(4.0));
        sheet.set_cell(CellCoord::new(4, 0), Cell::number(5.0));

        // Delete 2 rows at row 1 (delete rows 1 and 2)
        let deleted = sheet.delete_rows(1, 2);

        // Row 0 should stay in place
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );

        // Row 1 should now contain what was row 3
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(1, 0)).as_number(),
            Some(4.0)
        );

        // Row 2 should now contain what was row 4
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(2, 0)).as_number(),
            Some(5.0)
        );

        // Rows 3 and 4 should be empty
        assert!(sheet.get_cell(CellCoord::new(3, 0)).is_none());
        assert!(sheet.get_cell(CellCoord::new(4, 0)).is_none());

        // Check deleted cells
        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted[0].0, CellCoord::new(1, 0));
        assert_eq!(deleted[0].1.computed_value().as_number(), Some(2.0));
        assert_eq!(deleted[1].0, CellCoord::new(2, 0));
        assert_eq!(deleted[1].1.computed_value().as_number(), Some(3.0));
    }

    #[test]
    fn test_insert_rows_shifts_row_heights() {
        let mut sheet = Sheet::new("Test");

        // Set some custom row heights
        sheet.set_row_height(0, 30.0);
        sheet.set_row_height(1, 40.0);
        sheet.set_row_height(2, 50.0);

        // Insert 2 rows at row 1
        sheet.insert_rows(1, 2);

        // Row 0 height should stay the same
        assert_eq!(sheet.get_row_height(0), 30.0);

        // Rows 1 and 2 should have default height
        assert_eq!(sheet.get_row_height(1), DEFAULT_ROW_HEIGHT);
        assert_eq!(sheet.get_row_height(2), DEFAULT_ROW_HEIGHT);

        // Original row 1 (40.0) should now be at row 3
        assert_eq!(sheet.get_row_height(3), 40.0);

        // Original row 2 (50.0) should now be at row 4
        assert_eq!(sheet.get_row_height(4), 50.0);
    }

    #[test]
    fn test_delete_rows_shifts_row_heights() {
        let mut sheet = Sheet::new("Test");

        // Set some custom row heights
        sheet.set_row_height(0, 30.0);
        sheet.set_row_height(1, 40.0);
        sheet.set_row_height(2, 50.0);
        sheet.set_row_height(3, 60.0);
        sheet.set_row_height(4, 70.0);

        // Delete 2 rows at row 1 (delete rows 1 and 2)
        sheet.delete_rows(1, 2);

        // Row 0 height should stay the same
        assert_eq!(sheet.get_row_height(0), 30.0);

        // Row 1 should now have the height that was at row 3
        assert_eq!(sheet.get_row_height(1), 60.0);

        // Row 2 should now have the height that was at row 4
        assert_eq!(sheet.get_row_height(2), 70.0);

        // Rows 3 and 4 should have default height
        assert_eq!(sheet.get_row_height(3), DEFAULT_ROW_HEIGHT);
        assert_eq!(sheet.get_row_height(4), DEFAULT_ROW_HEIGHT);
    }

    #[test]
    fn test_insert_cols_shifts_cells() {
        let mut sheet = Sheet::new("Test");

        // Insert some cells
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(0, 1), Cell::number(2.0));
        sheet.set_cell(CellCoord::new(0, 2), Cell::number(3.0));
        sheet.set_cell(CellCoord::new(1, 1), Cell::text("B2"));

        // Insert 2 columns at column 1
        let shifts = sheet.insert_cols(1, 2);

        // Column 0 should stay in place
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );

        // Columns 1 and 2 should be empty (gap)
        assert!(sheet.get_cell(CellCoord::new(0, 1)).is_none());
        assert!(sheet.get_cell(CellCoord::new(0, 2)).is_none());

        // Column 1 content should shift to column 3
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 3)).as_number(),
            Some(2.0)
        );
        assert_eq!(sheet.get_cell_value(CellCoord::new(1, 3)).as_text(), "B2");

        // Column 2 content should shift to column 4
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 4)).as_number(),
            Some(3.0)
        );

        // Check shifts return value
        assert_eq!(shifts.len(), 3); // 3 cells were shifted
    }

    #[test]
    fn test_delete_cols_removes_and_shifts() {
        let mut sheet = Sheet::new("Test");

        // Insert some cells
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(0, 1), Cell::number(2.0));
        sheet.set_cell(CellCoord::new(0, 2), Cell::number(3.0));
        sheet.set_cell(CellCoord::new(0, 3), Cell::number(4.0));
        sheet.set_cell(CellCoord::new(0, 4), Cell::number(5.0));

        // Delete 2 columns at column 1 (delete columns 1 and 2)
        let deleted = sheet.delete_cols(1, 2);

        // Column 0 should stay in place
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );

        // Column 1 should now contain what was column 3
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 1)).as_number(),
            Some(4.0)
        );

        // Column 2 should now contain what was column 4
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 2)).as_number(),
            Some(5.0)
        );

        // Columns 3 and 4 should be empty
        assert!(sheet.get_cell(CellCoord::new(0, 3)).is_none());
        assert!(sheet.get_cell(CellCoord::new(0, 4)).is_none());

        // Check deleted cells
        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted[0].0, CellCoord::new(0, 1));
        assert_eq!(deleted[0].1.computed_value().as_number(), Some(2.0));
        assert_eq!(deleted[1].0, CellCoord::new(0, 2));
        assert_eq!(deleted[1].1.computed_value().as_number(), Some(3.0));
    }

    #[test]
    fn test_insert_cols_shifts_col_widths() {
        let mut sheet = Sheet::new("Test");

        // Set some custom column widths
        sheet.set_col_width(0, 150.0);
        sheet.set_col_width(1, 200.0);
        sheet.set_col_width(2, 250.0);

        // Insert 2 columns at column 1
        sheet.insert_cols(1, 2);

        // Column 0 width should stay the same
        assert_eq!(sheet.get_col_width(0), 150.0);

        // Columns 1 and 2 should have default width
        assert_eq!(sheet.get_col_width(1), DEFAULT_COL_WIDTH);
        assert_eq!(sheet.get_col_width(2), DEFAULT_COL_WIDTH);

        // Original column 1 (200.0) should now be at column 3
        assert_eq!(sheet.get_col_width(3), 200.0);

        // Original column 2 (250.0) should now be at column 4
        assert_eq!(sheet.get_col_width(4), 250.0);
    }

    #[test]
    fn test_delete_cols_shifts_col_widths() {
        let mut sheet = Sheet::new("Test");

        // Set some custom column widths
        sheet.set_col_width(0, 150.0);
        sheet.set_col_width(1, 200.0);
        sheet.set_col_width(2, 250.0);
        sheet.set_col_width(3, 300.0);
        sheet.set_col_width(4, 350.0);

        // Delete 2 columns at column 1 (delete columns 1 and 2)
        sheet.delete_cols(1, 2);

        // Column 0 width should stay the same
        assert_eq!(sheet.get_col_width(0), 150.0);

        // Column 1 should now have the width that was at column 3
        assert_eq!(sheet.get_col_width(1), 300.0);

        // Column 2 should now have the width that was at column 4
        assert_eq!(sheet.get_col_width(2), 350.0);

        // Columns 3 and 4 should have default width
        assert_eq!(sheet.get_col_width(3), DEFAULT_COL_WIDTH);
        assert_eq!(sheet.get_col_width(4), DEFAULT_COL_WIDTH);
    }

    #[test]
    fn test_insert_delete_rows_empty_operations() {
        let mut sheet = Sheet::new("Test");
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));

        // Insert 0 rows should do nothing
        let shifts = sheet.insert_rows(1, 0);
        assert_eq!(shifts.len(), 0);
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );

        // Delete 0 rows should do nothing
        let deleted = sheet.delete_rows(1, 0);
        assert_eq!(deleted.len(), 0);
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );
    }

    #[test]
    fn test_insert_delete_cols_empty_operations() {
        let mut sheet = Sheet::new("Test");
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));

        // Insert 0 columns should do nothing
        let shifts = sheet.insert_cols(1, 0);
        assert_eq!(shifts.len(), 0);
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );

        // Delete 0 columns should do nothing
        let deleted = sheet.delete_cols(1, 0);
        assert_eq!(deleted.len(), 0);
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );
    }

    #[test]
    fn test_conditional_formatting_add_remove() {
        use crate::core::{
            Color, ComparisonOperator, ConditionalFormat, ConditionalFormattingRule,
            ConditionalRule,
        };

        let mut sheet = Sheet::new("Test");

        // Add a rule
        let rule = ConditionalFormattingRule::new(
            "rule1".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(5, 5)),
            ConditionalRule::ValueBased {
                operator: ComparisonOperator::GreaterThan,
                value1: 50.0,
                value2: None,
                format: ConditionalFormat {
                    background_color: Some(Color::RED),
                    ..Default::default()
                },
            },
        );

        sheet.add_conditional_formatting(rule);
        assert_eq!(sheet.conditional_formatting.len(), 1);

        // Remove the rule
        assert!(sheet.remove_conditional_formatting("rule1"));
        assert_eq!(sheet.conditional_formatting.len(), 0);

        // Try to remove non-existent rule
        assert!(!sheet.remove_conditional_formatting("rule2"));
    }

    #[test]
    fn test_conditional_formatting_effective_format() {
        use crate::core::{
            Color, ComparisonOperator, ConditionalFormat, ConditionalFormattingRule,
            ConditionalRule,
        };

        let mut sheet = Sheet::new("Test");

        // Add cells with values
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(60.0));
        sheet.set_cell(CellCoord::new(0, 1), Cell::number(30.0));

        // Add conditional formatting rule
        let rule = ConditionalFormattingRule::new(
            "rule1".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(5, 5)),
            ConditionalRule::ValueBased {
                operator: ComparisonOperator::GreaterThan,
                value1: 50.0,
                value2: None,
                format: ConditionalFormat {
                    background_color: Some(Color::RED),
                    bold: Some(true),
                    ..Default::default()
                },
            },
        );

        sheet.add_conditional_formatting(rule);

        // Get effective format for cell with value > 50
        let base_format = CellFormat::default();
        let value = sheet.get_cell_value(CellCoord::new(0, 0));
        let effective = sheet.get_effective_format(0, 0, &base_format, value);

        assert_eq!(effective.background_color, Some(Color::RED));
        assert!(effective.bold);

        // Get effective format for cell with value <= 50
        let value = sheet.get_cell_value(CellCoord::new(0, 1));
        let effective = sheet.get_effective_format(0, 1, &base_format, value);

        // Should not have conditional formatting applied
        assert_eq!(effective.background_color, None);
        assert!(!effective.bold);
    }

    #[test]
    fn test_conditional_formatting_priority() {
        use crate::core::{
            Color, ComparisonOperator, ConditionalFormat, ConditionalFormattingRule,
            ConditionalRule,
        };

        let mut sheet = Sheet::new("Test");
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(60.0));

        // Add two overlapping rules with different priorities
        let mut rule1 = ConditionalFormattingRule::new(
            "rule1".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(5, 5)),
            ConditionalRule::ValueBased {
                operator: ComparisonOperator::GreaterThan,
                value1: 50.0,
                value2: None,
                format: ConditionalFormat {
                    background_color: Some(Color::RED),
                    ..Default::default()
                },
            },
        );
        rule1.priority = 1;

        let mut rule2 = ConditionalFormattingRule::new(
            "rule2".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(5, 5)),
            ConditionalRule::ValueBased {
                operator: ComparisonOperator::GreaterThan,
                value1: 40.0,
                value2: None,
                format: ConditionalFormat {
                    background_color: Some(Color::GREEN),
                    ..Default::default()
                },
            },
        );
        rule2.priority = 2;

        sheet.add_conditional_formatting(rule1);
        sheet.add_conditional_formatting(rule2);

        // Both rules match, but rule2 has higher priority
        let base_format = CellFormat::default();
        let value = sheet.get_cell_value(CellCoord::new(0, 0));
        let effective = sheet.get_effective_format(0, 0, &base_format, value);

        // Should have GREEN background from rule2 (higher priority)
        assert_eq!(effective.background_color, Some(Color::GREEN));
    }

    #[test]
    fn test_conditional_formatting_color_scale() {
        use crate::core::{Color, ConditionalFormattingRule, ConditionalRule};

        let mut sheet = Sheet::new("Test");

        // Add cells with values for color scale
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(0.0));
        sheet.set_cell(CellCoord::new(0, 1), Cell::number(50.0));
        sheet.set_cell(CellCoord::new(0, 2), Cell::number(100.0));

        let rule = ConditionalFormattingRule::new(
            "color_scale".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(0, 2)),
            ConditionalRule::ColorScale {
                min_color: Color::rgb(255, 0, 0),
                max_color: Color::rgb(0, 255, 0),
                mid_color: None,
            },
        );

        sheet.add_conditional_formatting(rule);

        let base_format = CellFormat::default();

        // Check min value (should be red)
        let value = sheet.get_cell_value(CellCoord::new(0, 0));
        let effective = sheet.get_effective_format(0, 0, &base_format, value);
        let color = effective.background_color.unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);

        // Check max value (should be green)
        let value = sheet.get_cell_value(CellCoord::new(0, 2));
        let effective = sheet.get_effective_format(0, 2, &base_format, value);
        let color = effective.background_color.unwrap();
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 255);

        // Check mid value (should be yellowish)
        let value = sheet.get_cell_value(CellCoord::new(0, 1));
        let effective = sheet.get_effective_format(0, 1, &base_format, value);
        let color = effective.background_color.unwrap();
        assert!(color.r > 100 && color.r < 150);
        assert!(color.g > 100 && color.g < 150);
    }

    #[test]
    fn test_data_validation_add_remove() {
        use crate::core::{DataValidationRule, ValidationCriteria, ValidationOperator};

        let mut sheet = Sheet::new("Test");

        // Add a validation rule
        let rule = DataValidationRule::new(
            "rule1".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(5, 5)),
            ValidationCriteria::WholeNumber {
                operator: ValidationOperator::Between,
                value1: 1,
                value2: Some(100),
            },
        );

        sheet.add_data_validation(rule);
        assert_eq!(sheet.data_validation.len(), 1);

        // Remove the rule
        assert!(sheet.remove_data_validation("rule1"));
        assert_eq!(sheet.data_validation.len(), 0);

        // Try to remove non-existent rule
        assert!(!sheet.remove_data_validation("rule2"));
    }

    #[test]
    fn test_data_validation_cell_validation() {
        use crate::core::{
            DataValidationRule, ValidationCriteria, ValidationOperator, ValidationResult,
        };

        let mut sheet = Sheet::new("Test");

        // Add validation rule for a range
        let rule = DataValidationRule::new(
            "rule1".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(5, 5)),
            ValidationCriteria::WholeNumber {
                operator: ValidationOperator::Between,
                value1: 1,
                value2: Some(100),
            },
        );

        sheet.add_data_validation(rule);

        // Test valid value
        let result = sheet.validate_cell_value(0, 0, &CellValue::Number(50.0));
        assert_eq!(result, ValidationResult::Valid);

        // Test invalid value
        let result = sheet.validate_cell_value(0, 0, &CellValue::Number(150.0));
        assert!(matches!(result, ValidationResult::Invalid(_)));

        // Test cell outside validation range
        let result = sheet.validate_cell_value(10, 10, &CellValue::Number(150.0));
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_data_validation_dropdown_items() {
        use crate::core::{DataValidationRule, ListSource, ValidationCriteria};

        let mut sheet = Sheet::new("Test");

        // Add dropdown validation
        let rule = DataValidationRule::new(
            "rule1".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(5, 0)),
            ValidationCriteria::List {
                source: ListSource::Values {
                    items: vec!["Option A".into(), "Option B".into(), "Option C".into()],
                },
                show_dropdown: true,
            },
        );

        sheet.add_data_validation(rule);

        // Get dropdown items for a cell in the range
        let items = sheet.get_cell_dropdown_items(0, 0);
        assert_eq!(
            items,
            Some(vec![
                "Option A".into(),
                "Option B".into(),
                "Option C".into()
            ])
        );

        // Get dropdown items for a cell outside the range
        let items = sheet.get_cell_dropdown_items(10, 0);
        assert_eq!(items, None);
    }

    #[test]
    fn test_data_validation_replace_rule() {
        use crate::core::{DataValidationRule, ValidationCriteria, ValidationOperator};

        let mut sheet = Sheet::new("Test");

        // Add first rule
        let rule1 = DataValidationRule::new(
            "rule1".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(5, 5)),
            ValidationCriteria::WholeNumber {
                operator: ValidationOperator::GreaterThan,
                value1: 0,
                value2: None,
            },
        );

        sheet.add_data_validation(rule1);
        assert_eq!(sheet.data_validation.len(), 1);

        // Add another rule with same ID (should replace)
        let rule2 = DataValidationRule::new(
            "rule1".to_string(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 10)),
            ValidationCriteria::WholeNumber {
                operator: ValidationOperator::LessThan,
                value1: 100,
                value2: None,
            },
        );

        sheet.add_data_validation(rule2);
        assert_eq!(sheet.data_validation.len(), 1);

        // Verify the rule was replaced by checking the range
        let rule = &sheet.data_validation[0];
        assert_eq!(rule.range.end, CellCoord::new(10, 10));
    }
}
