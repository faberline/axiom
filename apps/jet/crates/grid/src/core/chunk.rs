//! Chunked sparse storage for efficient spreadsheet grid operations.
//!
//! This module implements a two-level sparse data structure optimized for spreadsheet grids:
//! - Top level: HashMap of chunk coordinates
//! - Bottom level: Cache-friendly 64x64 chunk with Morton-encoded flat storage
//!
//! Uses Z-order (Morton) curve indexing for improved cache locality during
//! range queries common in spreadsheet rendering.

use std::collections::HashMap;

use bitvec::prelude::*;

use crate::core::spatial::morton_encode;

/// Size of each chunk in both dimensions (64x64 cells per chunk).
pub const CHUNK_SIZE: usize = 64;

/// Total number of cells per chunk (64 * 64 = 4096).
pub const CHUNK_AREA: usize = CHUNK_SIZE * CHUNK_SIZE;

/// Type alias for cell coordinates (row, col).
pub type CellCoord = (usize, usize);

/// Type alias for a shift mapping from old coordinate to new coordinate.
pub type ShiftMapping = (CellCoord, CellCoord);

/// Coordinate of a chunk in the grid.
///
/// Represents which 64x64 block a cell belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    /// Which 64-row block (row >> 6)
    pub block_row: usize,
    /// Which 64-column block (col >> 6)
    pub block_col: usize,
}

impl ChunkCoord {
    /// Create a ChunkCoord from a cell's global coordinates.
    ///
    /// Uses bit shift for fast division by 64.
    ///
    /// # Examples
    ///
    /// ```
    /// use jet_grid::core::ChunkCoord;
    ///
    /// let coord = ChunkCoord::from_cell(64, 128);
    /// assert_eq!(coord.block_row, 1);
    /// assert_eq!(coord.block_col, 2);
    /// ```
    #[inline]
    pub fn from_cell(row: usize, col: usize) -> Self {
        Self {
            block_row: row >> 6, // row / 64
            block_col: col >> 6, // col / 64
        }
    }
}

/// A single 64x64 chunk of cells using Morton-encoded flat storage.
///
/// Uses Structure-of-Arrays layout with BitVec for O(1) existence checks
/// and cache-friendly memory access patterns via Z-order curve indexing.
#[derive(Clone, Debug)]
pub struct Chunk<T> {
    /// Bitmap indicating which cells are occupied (4096 bits = 512 bytes)
    occupied: BitVec,
    /// Flat storage indexed by Morton code, None for empty cells
    cells: Vec<Option<T>>,
    /// Count of occupied cells for O(1) len()
    count: usize,
}

impl<T> Chunk<T> {
    /// Create a new empty chunk.
    pub fn new() -> Self {
        Self {
            occupied: bitvec![0; CHUNK_AREA],
            cells: (0..CHUNK_AREA).map(|_| None).collect(),
            count: 0,
        }
    }

    /// Get a reference to a cell at the given local coordinates.
    ///
    /// # Arguments
    ///
    /// * `local_row` - Row within this chunk (0-63)
    /// * `local_col` - Column within this chunk (0-63)
    #[inline]
    pub fn get(&self, local_row: u8, local_col: u8) -> Option<&T> {
        let idx = morton_encode(local_row, local_col) as usize;
        if self.occupied[idx] {
            self.cells[idx].as_ref()
        } else {
            None
        }
    }

    /// Get a mutable reference to a cell at the given local coordinates.
    ///
    /// # Arguments
    ///
    /// * `local_row` - Row within this chunk (0-63)
    /// * `local_col` - Column within this chunk (0-63)
    #[inline]
    pub fn get_mut(&mut self, local_row: u8, local_col: u8) -> Option<&mut T> {
        let idx = morton_encode(local_row, local_col) as usize;
        if self.occupied[idx] {
            self.cells[idx].as_mut()
        } else {
            None
        }
    }

    /// Insert a value at the given local coordinates.
    ///
    /// Returns the previous value if one existed.
    ///
    /// # Arguments
    ///
    /// * `local_row` - Row within this chunk (0-63)
    /// * `local_col` - Column within this chunk (0-63)
    /// * `value` - Value to insert
    #[inline]
    pub fn insert(&mut self, local_row: u8, local_col: u8, value: T) -> Option<T> {
        let idx = morton_encode(local_row, local_col) as usize;
        let was_occupied = self.occupied[idx];
        self.occupied.set(idx, true);

        if !was_occupied {
            self.count += 1;
        }

        self.cells[idx].replace(value)
    }

    /// Remove a cell at the given local coordinates.
    ///
    /// Returns the value if one existed.
    ///
    /// # Arguments
    ///
    /// * `local_row` - Row within this chunk (0-63)
    /// * `local_col` - Column within this chunk (0-63)
    #[inline]
    pub fn remove(&mut self, local_row: u8, local_col: u8) -> Option<T> {
        let idx = morton_encode(local_row, local_col) as usize;
        if self.occupied[idx] {
            self.occupied.set(idx, false);
            self.count -= 1;
            self.cells[idx].take()
        } else {
            None
        }
    }

    /// Check if this chunk is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get the number of cells in this chunk.
    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Iterate over all cells in this chunk.
    ///
    /// Returns an iterator of ((local_row, local_col), &value) tuples.
    pub fn iter(&self) -> impl Iterator<Item = ((u8, u8), &T)> {
        self.occupied.iter_ones().filter_map(move |idx| {
            let (row, col) = crate::core::spatial::morton_decode(idx as u16);
            self.cells[idx].as_ref().map(|v| ((row, col), v))
        })
    }

    /// Iterate mutably over all cells in this chunk.
    ///
    /// Returns an iterator of ((local_row, local_col), &mut value) tuples.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = ((u8, u8), &mut T)> {
        let cells = &mut self.cells;
        self.occupied.iter_ones().filter_map(move |idx| {
            let (row, col) = crate::core::spatial::morton_decode(idx as u16);
            // SAFETY: We only iterate over occupied indices
            let cell_ptr = cells.as_mut_ptr();
            unsafe { (*cell_ptr.add(idx)).as_mut().map(|v| ((row, col), v)) }
        })
    }
}

impl<T> Default for Chunk<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// A chunked sparse grid for efficient spreadsheet storage.
///
/// This data structure divides the grid into 64x64 chunks and only allocates
/// storage for chunks that contain data. Provides O(1) random access while
/// minimizing memory usage for sparse data. Uses Morton encoding within chunks
/// for cache-friendly access patterns.
#[derive(Clone, Debug)]
pub struct ChunkedGrid<T> {
    /// Map of chunk coordinates to chunks.
    chunks: HashMap<ChunkCoord, Chunk<T>>,
}

impl<T: Clone> ChunkedGrid<T> {
    /// Create a new empty chunked grid.
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    /// Get a reference to a cell at the given global coordinates.
    ///
    /// # Arguments
    ///
    /// * `row` - Global row coordinate
    /// * `col` - Global column coordinate
    pub fn get(&self, row: usize, col: usize) -> Option<&T> {
        let chunk_coord = ChunkCoord::from_cell(row, col);
        let (local_row, local_col) = to_local_coords(row, col);
        self.chunks.get(&chunk_coord)?.get(local_row, local_col)
    }

    /// Get a mutable reference to a cell at the given global coordinates.
    ///
    /// # Arguments
    ///
    /// * `row` - Global row coordinate
    /// * `col` - Global column coordinate
    pub fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut T> {
        let chunk_coord = ChunkCoord::from_cell(row, col);
        let (local_row, local_col) = to_local_coords(row, col);
        self.chunks
            .get_mut(&chunk_coord)?
            .get_mut(local_row, local_col)
    }

    /// Insert a value at the given global coordinates.
    ///
    /// Creates a new chunk if necessary. Returns the previous value if one existed.
    ///
    /// # Arguments
    ///
    /// * `row` - Global row coordinate
    /// * `col` - Global column coordinate
    /// * `value` - Value to insert
    pub fn insert(&mut self, row: usize, col: usize, value: T) -> Option<T> {
        let chunk_coord = ChunkCoord::from_cell(row, col);
        let (local_row, local_col) = to_local_coords(row, col);
        let chunk = self.chunks.entry(chunk_coord).or_default();
        chunk.insert(local_row, local_col, value)
    }

    /// Remove a cell at the given global coordinates.
    ///
    /// Removes the chunk if it becomes empty. Returns the value if one existed.
    ///
    /// # Arguments
    ///
    /// * `row` - Global row coordinate
    /// * `col` - Global column coordinate
    pub fn remove(&mut self, row: usize, col: usize) -> Option<T> {
        let chunk_coord = ChunkCoord::from_cell(row, col);
        let (local_row, local_col) = to_local_coords(row, col);

        let chunk = self.chunks.get_mut(&chunk_coord)?;
        let value = chunk.remove(local_row, local_col);

        // Clean up empty chunk
        if chunk.is_empty() {
            self.chunks.remove(&chunk_coord);
        }

        value
    }

    /// Check if a cell exists at the given global coordinates.
    ///
    /// # Arguments
    ///
    /// * `row` - Global row coordinate
    /// * `col` - Global column coordinate
    pub fn contains(&self, row: usize, col: usize) -> bool {
        self.get(row, col).is_some()
    }

    /// Get the total number of cells across all chunks.
    pub fn len(&self) -> usize {
        self.chunks.values().map(|chunk| chunk.len()).sum()
    }

    /// Check if the grid is empty.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Get a reference to a chunk containing the given cell.
    ///
    /// Useful for batch operations or rendering.
    ///
    /// # Arguments
    ///
    /// * `row` - Global row coordinate
    /// * `col` - Global column coordinate
    pub fn get_chunk(&self, row: usize, col: usize) -> Option<&Chunk<T>> {
        let chunk_coord = ChunkCoord::from_cell(row, col);
        self.chunks.get(&chunk_coord)
    }

    /// Get or create a mutable reference to a chunk containing the given cell.
    ///
    /// Creates the chunk if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `row` - Global row coordinate
    /// * `col` - Global column coordinate
    pub fn get_or_create_chunk(&mut self, row: usize, col: usize) -> &mut Chunk<T> {
        let chunk_coord = ChunkCoord::from_cell(row, col);
        self.chunks.entry(chunk_coord).or_default()
    }

    /// Iterate over all cells in the grid.
    ///
    /// Returns an iterator of ((row, col), &value) tuples with global coordinates.
    pub fn iter(&self) -> impl Iterator<Item = ((usize, usize), &T)> {
        self.chunks.iter().flat_map(|(chunk_coord, chunk)| {
            chunk.iter().map(move |((local_row, local_col), value)| {
                let (row, col) = to_global_coords(chunk_coord, local_row, local_col);
                ((row, col), value)
            })
        })
    }

    /// Iterate mutably over all cells in the grid.
    ///
    /// Returns an iterator of ((row, col), &mut value) tuples with global coordinates.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = ((usize, usize), &mut T)> {
        self.chunks.iter_mut().flat_map(|(chunk_coord, chunk)| {
            let chunk_coord = *chunk_coord;
            chunk
                .iter_mut()
                .map(move |((local_row, local_col), value)| {
                    let (row, col) = to_global_coords(&chunk_coord, local_row, local_col);
                    ((row, col), value)
                })
        })
    }

    /// Get all cells within a given range.
    ///
    /// Efficiently queries only the chunks that overlap with the range.
    /// Useful for rendering visible cells.
    ///
    /// # Arguments
    ///
    /// * `start_row` - Starting row (inclusive)
    /// * `start_col` - Starting column (inclusive)
    /// * `end_row` - Ending row (inclusive)
    /// * `end_col` - Ending column (inclusive)
    pub fn cells_in_range(
        &self,
        start_row: usize,
        start_col: usize,
        end_row: usize,
        end_col: usize,
    ) -> Vec<((usize, usize), &T)> {
        let start_chunk = ChunkCoord::from_cell(start_row, start_col);
        let end_chunk = ChunkCoord::from_cell(end_row, end_col);

        let mut result = Vec::new();

        for block_row in start_chunk.block_row..=end_chunk.block_row {
            for block_col in start_chunk.block_col..=end_chunk.block_col {
                let chunk_coord = ChunkCoord {
                    block_row,
                    block_col,
                };

                if let Some(chunk) = self.chunks.get(&chunk_coord) {
                    for ((local_row, local_col), value) in chunk.iter() {
                        let (row, col) = to_global_coords(&chunk_coord, local_row, local_col);

                        if row >= start_row && row <= end_row && col >= start_col && col <= end_col
                        {
                            result.push(((row, col), value));
                        }
                    }
                }
            }
        }

        result
    }

    /// Shift all rows >= start_row down by `count` positions.
    /// Used for inserting rows - creates a gap at start_row.
    ///
    /// Returns a list of (old_coord, new_coord) for affected cells.
    pub fn shift_rows_down(&mut self, start_row: usize, count: usize) -> Vec<ShiftMapping> {
        if count == 0 {
            return Vec::new();
        }

        // Collect all cells that need to shift (row >= start_row)
        let cells_to_shift: Vec<((usize, usize), T)> = self
            .iter()
            .filter(|((row, _), _)| *row >= start_row)
            .map(|((row, col), value)| ((row, col), value.clone()))
            .collect();

        // Remove them from the grid
        for ((row, col), _) in &cells_to_shift {
            self.remove(*row, *col);
        }

        // Re-insert at new positions
        let mut shifts = Vec::with_capacity(cells_to_shift.len());
        for ((old_row, col), value) in cells_to_shift {
            let new_row = old_row + count;
            self.insert(new_row, col, value);
            shifts.push(((old_row, col), (new_row, col)));
        }

        shifts
    }

    /// Shift all rows > deleted_row up by `count` positions.
    /// Used for deleting rows - removes rows [start_row, start_row + count) and shifts rest up.
    ///
    /// Returns a list of removed cells and a list of (old_coord, new_coord) for shifted cells.
    pub fn shift_rows_up(
        &mut self,
        start_row: usize,
        count: usize,
    ) -> (Vec<(CellCoord, T)>, Vec<ShiftMapping>) {
        if count == 0 {
            return (Vec::new(), Vec::new());
        }

        let end_row = start_row + count;

        // Collect cells in the deleted range
        let deleted_cells: Vec<((usize, usize), T)> = self
            .iter()
            .filter(|((row, _), _)| *row >= start_row && *row < end_row)
            .map(|((row, col), value)| ((row, col), value.clone()))
            .collect();

        // Collect cells that need to shift (row >= end_row)
        let cells_to_shift: Vec<((usize, usize), T)> = self
            .iter()
            .filter(|((row, _), _)| *row >= end_row)
            .map(|((row, col), value)| ((row, col), value.clone()))
            .collect();

        // Remove deleted cells
        for ((row, col), _) in &deleted_cells {
            self.remove(*row, *col);
        }

        // Remove cells that will shift
        for ((row, col), _) in &cells_to_shift {
            self.remove(*row, *col);
        }

        // Re-insert shifted cells at new positions
        let mut shifts = Vec::with_capacity(cells_to_shift.len());
        for ((old_row, col), value) in cells_to_shift {
            let new_row = old_row - count;
            self.insert(new_row, col, value);
            shifts.push(((old_row, col), (new_row, col)));
        }

        (deleted_cells, shifts)
    }

    /// Shift all columns >= start_col right by `count` positions.
    /// Used for inserting columns - creates a gap at start_col.
    ///
    /// Returns a list of (old_coord, new_coord) for affected cells.
    pub fn shift_cols_right(&mut self, start_col: usize, count: usize) -> Vec<ShiftMapping> {
        if count == 0 {
            return Vec::new();
        }

        // Collect all cells that need to shift (col >= start_col)
        let cells_to_shift: Vec<((usize, usize), T)> = self
            .iter()
            .filter(|((_, col), _)| *col >= start_col)
            .map(|((row, col), value)| ((row, col), value.clone()))
            .collect();

        // Remove them from the grid
        for ((row, col), _) in &cells_to_shift {
            self.remove(*row, *col);
        }

        // Re-insert at new positions
        let mut shifts = Vec::with_capacity(cells_to_shift.len());
        for ((row, old_col), value) in cells_to_shift {
            let new_col = old_col + count;
            self.insert(row, new_col, value);
            shifts.push(((row, old_col), (row, new_col)));
        }

        shifts
    }

    /// Shift all columns > deleted_col left by `count` positions.
    /// Used for deleting columns - removes cols [start_col, start_col + count) and shifts rest left.
    ///
    /// Returns a list of removed cells and a list of (old_coord, new_coord) for shifted cells.
    pub fn shift_cols_left(
        &mut self,
        start_col: usize,
        count: usize,
    ) -> (Vec<(CellCoord, T)>, Vec<ShiftMapping>) {
        if count == 0 {
            return (Vec::new(), Vec::new());
        }

        let end_col = start_col + count;

        // Collect cells in the deleted range
        let deleted_cells: Vec<((usize, usize), T)> = self
            .iter()
            .filter(|((_, col), _)| *col >= start_col && *col < end_col)
            .map(|((row, col), value)| ((row, col), value.clone()))
            .collect();

        // Collect cells that need to shift (col >= end_col)
        let cells_to_shift: Vec<((usize, usize), T)> = self
            .iter()
            .filter(|((_, col), _)| *col >= end_col)
            .map(|((row, col), value)| ((row, col), value.clone()))
            .collect();

        // Remove deleted cells
        for ((row, col), _) in &deleted_cells {
            self.remove(*row, *col);
        }

        // Remove cells that will shift
        for ((row, col), _) in &cells_to_shift {
            self.remove(*row, *col);
        }

        // Re-insert shifted cells at new positions
        let mut shifts = Vec::with_capacity(cells_to_shift.len());
        for ((row, old_col), value) in cells_to_shift {
            let new_col = old_col - count;
            self.insert(row, new_col, value);
            shifts.push(((row, old_col), (row, new_col)));
        }

        (deleted_cells, shifts)
    }
}

impl<T: Clone> Default for ChunkedGrid<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert global coordinates to local coordinates within a chunk.
///
/// Uses bitwise AND for fast modulo by 64.
///
/// # Arguments
///
/// * `row` - Global row coordinate
/// * `col` - Global column coordinate
///
/// # Returns
///
/// A tuple of (local_row, local_col) within the chunk (0-63 for each).
#[inline]
pub fn to_local_coords(row: usize, col: usize) -> (u8, u8) {
    let local_row = (row & 0x3F) as u8; // row % 64
    let local_col = (col & 0x3F) as u8; // col % 64
    (local_row, local_col)
}

/// Convert chunk coordinate and local coordinates to global coordinates.
///
/// Uses bit shift for fast multiplication by 64.
///
/// # Arguments
///
/// * `chunk` - The chunk coordinate
/// * `local_row` - Row within the chunk (0-63)
/// * `local_col` - Column within the chunk (0-63)
///
/// # Returns
///
/// A tuple of (global_row, global_col).
#[inline]
pub fn to_global_coords(chunk: &ChunkCoord, local_row: u8, local_col: u8) -> (usize, usize) {
    let row = (chunk.block_row << 6) + local_row as usize; // block_row * 64 + local
    let col = (chunk.block_col << 6) + local_col as usize; // block_col * 64 + local
    (row, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_coord_from_cell() {
        // Within first chunk (0-63)
        assert_eq!(
            ChunkCoord::from_cell(0, 0),
            ChunkCoord {
                block_row: 0,
                block_col: 0
            }
        );
        assert_eq!(
            ChunkCoord::from_cell(63, 63),
            ChunkCoord {
                block_row: 0,
                block_col: 0
            }
        );
        // Second chunk starts at 64
        assert_eq!(
            ChunkCoord::from_cell(64, 64),
            ChunkCoord {
                block_row: 1,
                block_col: 1
            }
        );
        assert_eq!(
            ChunkCoord::from_cell(65, 129),
            ChunkCoord {
                block_row: 1,
                block_col: 2
            }
        );
    }

    #[test]
    fn test_to_local_coords() {
        assert_eq!(to_local_coords(0, 0), (0, 0));
        assert_eq!(to_local_coords(63, 63), (63, 63));
        assert_eq!(to_local_coords(64, 64), (0, 0));
        assert_eq!(to_local_coords(65, 66), (1, 2));
        assert_eq!(to_local_coords(127, 127), (63, 63));
    }

    #[test]
    fn test_to_global_coords() {
        let chunk = ChunkCoord {
            block_row: 0,
            block_col: 0,
        };
        assert_eq!(to_global_coords(&chunk, 0, 0), (0, 0));
        assert_eq!(to_global_coords(&chunk, 63, 63), (63, 63));

        let chunk = ChunkCoord {
            block_row: 1,
            block_col: 2,
        };
        assert_eq!(to_global_coords(&chunk, 0, 0), (64, 128));
        assert_eq!(to_global_coords(&chunk, 1, 2), (65, 130));
        assert_eq!(to_global_coords(&chunk, 63, 63), (127, 191));
    }

    #[test]
    fn test_chunk_basic_operations() {
        let mut chunk = Chunk::new();
        assert!(chunk.is_empty());
        assert_eq!(chunk.len(), 0);

        // Insert
        assert_eq!(chunk.insert(0, 0, "A1"), None);
        assert_eq!(chunk.len(), 1);
        assert!(!chunk.is_empty());

        // Get
        assert_eq!(chunk.get(0, 0), Some(&"A1"));
        assert_eq!(chunk.get(1, 1), None);

        // Insert duplicate
        assert_eq!(chunk.insert(0, 0, "A1_updated"), Some("A1"));
        assert_eq!(chunk.len(), 1);

        // Get mut
        if let Some(val) = chunk.get_mut(0, 0) {
            *val = "A1_mutated";
        }
        assert_eq!(chunk.get(0, 0), Some(&"A1_mutated"));

        // Remove
        assert_eq!(chunk.remove(0, 0), Some("A1_mutated"));
        assert_eq!(chunk.len(), 0);
        assert!(chunk.is_empty());
        assert_eq!(chunk.remove(0, 0), None);
    }

    #[test]
    fn test_chunk_iteration() {
        let mut chunk = Chunk::new();
        chunk.insert(0, 0, 1);
        chunk.insert(5, 10, 2);
        chunk.insert(15, 15, 3);

        let mut items: Vec<_> = chunk.iter().collect();
        items.sort_by_key(|((r, c), _)| (*r, *c));

        assert_eq!(items.len(), 3);
        assert_eq!(items[0], ((0, 0), &1));
        assert_eq!(items[1], ((5, 10), &2));
        assert_eq!(items[2], ((15, 15), &3));
    }

    #[test]
    fn test_chunked_grid_basic_operations() {
        let mut grid = ChunkedGrid::new();
        assert!(grid.is_empty());
        assert_eq!(grid.len(), 0);

        // Insert
        assert_eq!(grid.insert(0, 0, "A1"), None);
        assert_eq!(grid.len(), 1);
        assert!(!grid.is_empty());
        assert!(grid.contains(0, 0));

        // Get
        assert_eq!(grid.get(0, 0), Some(&"A1"));
        assert_eq!(grid.get(1, 1), None);
        assert!(!grid.contains(1, 1));

        // Insert in different chunk
        assert_eq!(grid.insert(17, 33, "B2"), None);
        assert_eq!(grid.len(), 2);

        // Get mut
        if let Some(val) = grid.get_mut(0, 0) {
            *val = "A1_updated";
        }
        assert_eq!(grid.get(0, 0), Some(&"A1_updated"));

        // Remove
        assert_eq!(grid.remove(0, 0), Some("A1_updated"));
        assert_eq!(grid.len(), 1);
        assert!(!grid.contains(0, 0));
    }

    #[test]
    fn test_chunked_grid_chunk_creation() {
        let mut grid = ChunkedGrid::new();

        // Insert in different chunks (64x64 each)
        grid.insert(0, 0, 1); // Chunk (0, 0)
        grid.insert(64, 0, 2); // Chunk (1, 0)
        grid.insert(0, 64, 3); // Chunk (0, 1)
        grid.insert(64, 64, 4); // Chunk (1, 1)

        assert_eq!(grid.chunks.len(), 4);
        assert_eq!(grid.len(), 4);

        assert_eq!(grid.get(0, 0), Some(&1));
        assert_eq!(grid.get(64, 0), Some(&2));
        assert_eq!(grid.get(0, 64), Some(&3));
        assert_eq!(grid.get(64, 64), Some(&4));
    }

    #[test]
    fn test_chunked_grid_chunk_cleanup() {
        let mut grid = ChunkedGrid::new();

        // Insert multiple cells in same chunk
        grid.insert(0, 0, 1);
        grid.insert(0, 1, 2);
        grid.insert(1, 0, 3);

        assert_eq!(grid.chunks.len(), 1);
        assert_eq!(grid.len(), 3);

        // Remove cells one by one
        grid.remove(0, 0);
        assert_eq!(grid.chunks.len(), 1); // Chunk still exists
        assert_eq!(grid.len(), 2);

        grid.remove(0, 1);
        assert_eq!(grid.chunks.len(), 1); // Chunk still exists
        assert_eq!(grid.len(), 1);

        // Remove last cell - chunk should be cleaned up
        grid.remove(1, 0);
        assert_eq!(grid.chunks.len(), 0); // Chunk removed
        assert_eq!(grid.len(), 0);
        assert!(grid.is_empty());
    }

    #[test]
    fn test_chunked_grid_iteration() {
        let mut grid = ChunkedGrid::new();
        grid.insert(0, 0, 1);
        grid.insert(17, 33, 2);
        grid.insert(100, 200, 3);

        let mut items: Vec<_> = grid.iter().map(|(coord, val)| (coord, *val)).collect();
        items.sort_by_key(|((r, c), _)| (*r, *c));

        assert_eq!(items.len(), 3);
        assert_eq!(items[0], ((0, 0), 1));
        assert_eq!(items[1], ((17, 33), 2));
        assert_eq!(items[2], ((100, 200), 3));
    }

    #[test]
    fn test_chunked_grid_iter_mut() {
        let mut grid = ChunkedGrid::new();
        grid.insert(0, 0, 1);
        grid.insert(17, 33, 2);
        grid.insert(100, 200, 3);

        // Multiply all values by 10
        for (_, value) in grid.iter_mut() {
            *value *= 10;
        }

        assert_eq!(grid.get(0, 0), Some(&10));
        assert_eq!(grid.get(17, 33), Some(&20));
        assert_eq!(grid.get(100, 200), Some(&30));
    }

    #[test]
    fn test_cells_in_range() {
        let mut grid = ChunkedGrid::new();

        // Insert cells in various positions
        grid.insert(0, 0, "A1");
        grid.insert(5, 5, "F6");
        grid.insert(10, 10, "K11");
        grid.insert(15, 15, "P16");
        grid.insert(20, 20, "U21");
        grid.insert(50, 50, "AY51");

        // Query range (0, 0) to (15, 15)
        let mut cells = grid.cells_in_range(0, 0, 15, 15);
        cells.sort_by_key(|((r, c), _)| (*r, *c));

        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0], ((0, 0), &"A1"));
        assert_eq!(cells[1], ((5, 5), &"F6"));
        assert_eq!(cells[2], ((10, 10), &"K11"));
        assert_eq!(cells[3], ((15, 15), &"P16"));

        // Query range (10, 10) to (25, 25)
        let mut cells = grid.cells_in_range(10, 10, 25, 25);
        cells.sort_by_key(|((r, c), _)| (*r, *c));

        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0], ((10, 10), &"K11"));
        assert_eq!(cells[1], ((15, 15), &"P16"));
        assert_eq!(cells[2], ((20, 20), &"U21"));

        // Query range that includes nothing
        let cells = grid.cells_in_range(30, 30, 40, 40);
        assert_eq!(cells.len(), 0);
    }

    #[test]
    fn test_cells_in_range_across_chunks() {
        let mut grid = ChunkedGrid::new();

        // Insert cells across multiple chunks
        for row in 0..64 {
            for col in 0..64 {
                if (row + col) % 10 == 0 {
                    grid.insert(row, col, row * 1000 + col);
                }
            }
        }

        // Query a range that spans multiple chunks
        let cells = grid.cells_in_range(10, 10, 30, 30);

        // Verify all cells in range are returned
        for ((row, col), value) in &cells {
            assert!(*row >= 10 && *row <= 30);
            assert!(*col >= 10 && *col <= 30);
            assert_eq!(**value, row * 1000 + col);
        }

        // Verify the count is correct
        let expected_count = (10..=30)
            .flat_map(|r| (10..=30).map(move |c| (r, c)))
            .filter(|(r, c)| (r + c) % 10 == 0)
            .count();
        assert_eq!(cells.len(), expected_count);
    }

    #[test]
    fn test_get_chunk() {
        let mut grid = ChunkedGrid::new();
        grid.insert(0, 0, 1);
        grid.insert(5, 5, 2);

        let chunk = grid.get_chunk(0, 0);
        assert!(chunk.is_some());
        assert_eq!(chunk.unwrap().len(), 2);

        // 64, 64 is in a different chunk
        let chunk = grid.get_chunk(64, 64);
        assert!(chunk.is_none());
    }

    #[test]
    fn test_get_or_create_chunk() {
        let mut grid = ChunkedGrid::new();

        // Get or create a chunk
        let chunk = grid.get_or_create_chunk(0, 0);
        chunk.insert(0, 0, 42);

        assert_eq!(grid.get(0, 0), Some(&42));
        assert_eq!(grid.chunks.len(), 1);

        // Get existing chunk
        let chunk = grid.get_or_create_chunk(5, 5);
        chunk.insert(5, 5, 99);

        assert_eq!(grid.get(5, 5), Some(&99));
        assert_eq!(grid.chunks.len(), 1); // Still same chunk
    }

    #[test]
    fn test_shift_rows_down() {
        let mut grid = ChunkedGrid::new();
        grid.insert(0, 0, "A1");
        grid.insert(1, 0, "A2");
        grid.insert(2, 0, "A3");
        grid.insert(0, 1, "B1");

        // Insert 2 rows at row 1
        let shifts = grid.shift_rows_down(1, 2);

        // A1 stays, A2 becomes A4, A3 becomes A5, B1 stays
        assert_eq!(grid.get(0, 0), Some(&"A1"));
        assert_eq!(grid.get(0, 1), Some(&"B1"));
        assert_eq!(grid.get(1, 0), None); // Gap
        assert_eq!(grid.get(2, 0), None); // Gap
        assert_eq!(grid.get(3, 0), Some(&"A2")); // Shifted from row 1
        assert_eq!(grid.get(4, 0), Some(&"A3")); // Shifted from row 2

        assert_eq!(shifts.len(), 2);
    }

    #[test]
    fn test_shift_rows_up() {
        let mut grid = ChunkedGrid::new();
        grid.insert(0, 0, "A1");
        grid.insert(1, 0, "A2");
        grid.insert(2, 0, "A3");
        grid.insert(3, 0, "A4");
        grid.insert(4, 0, "A5");

        // Delete rows 1 and 2
        let (deleted, shifts) = grid.shift_rows_up(1, 2);

        // A1 stays, A2 and A3 deleted, A4 becomes A2, A5 becomes A3
        assert_eq!(grid.get(0, 0), Some(&"A1"));
        assert_eq!(grid.get(1, 0), Some(&"A4")); // Shifted from row 3
        assert_eq!(grid.get(2, 0), Some(&"A5")); // Shifted from row 4
        assert_eq!(grid.get(3, 0), None);
        assert_eq!(grid.get(4, 0), None);

        assert_eq!(deleted.len(), 2);
        assert_eq!(shifts.len(), 2);
    }

    #[test]
    fn test_shift_cols_right() {
        let mut grid = ChunkedGrid::new();
        grid.insert(0, 0, "A1");
        grid.insert(0, 1, "B1");
        grid.insert(0, 2, "C1");

        // Insert 2 columns at col 1
        let shifts = grid.shift_cols_right(1, 2);

        assert_eq!(grid.get(0, 0), Some(&"A1"));
        assert_eq!(grid.get(0, 1), None); // Gap
        assert_eq!(grid.get(0, 2), None); // Gap
        assert_eq!(grid.get(0, 3), Some(&"B1")); // Shifted from col 1
        assert_eq!(grid.get(0, 4), Some(&"C1")); // Shifted from col 2

        assert_eq!(shifts.len(), 2);
    }

    #[test]
    fn test_shift_cols_left() {
        let mut grid = ChunkedGrid::new();
        grid.insert(0, 0, "A1");
        grid.insert(0, 1, "B1");
        grid.insert(0, 2, "C1");
        grid.insert(0, 3, "D1");
        grid.insert(0, 4, "E1");

        // Delete cols 1 and 2
        let (deleted, shifts) = grid.shift_cols_left(1, 2);

        assert_eq!(grid.get(0, 0), Some(&"A1"));
        assert_eq!(grid.get(0, 1), Some(&"D1")); // Shifted from col 3
        assert_eq!(grid.get(0, 2), Some(&"E1")); // Shifted from col 4
        assert_eq!(grid.get(0, 3), None);
        assert_eq!(grid.get(0, 4), None);

        assert_eq!(deleted.len(), 2);
        assert_eq!(shifts.len(), 2);
    }

    #[test]
    fn test_shift_across_chunks() {
        let mut grid = ChunkedGrid::new();
        // Insert cells in different chunks (chunks are 64x64)
        grid.insert(0, 0, 1);
        grid.insert(63, 0, 2); // Last row of first chunk
        grid.insert(64, 0, 3); // First row of second chunk
        grid.insert(100, 0, 4);

        // Insert 10 rows at row 50
        grid.shift_rows_down(50, 10);

        // Cells before row 50 stay
        assert_eq!(grid.get(0, 0), Some(&1));

        // Cells at row 63 shifted to 73
        assert_eq!(grid.get(63, 0), None);
        assert_eq!(grid.get(73, 0), Some(&2));

        // Cells at row 64 shifted to 74
        assert_eq!(grid.get(64, 0), None);
        assert_eq!(grid.get(74, 0), Some(&3));

        // Cells at row 100 shifted to 110
        assert_eq!(grid.get(100, 0), None);
        assert_eq!(grid.get(110, 0), Some(&4));
    }
}
