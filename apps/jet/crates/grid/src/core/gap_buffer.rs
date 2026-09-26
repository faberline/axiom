//! Gap Buffer Index Mapping
//!
//! This module provides an efficient index mapping structure for row/column insertion and deletion.
//! Instead of physically moving data when rows/columns are inserted or deleted, we maintain a mapping
//! between "logical" indices (what the user sees) and "physical" indices (where data is actually stored).
//!
//! # Example
//!
//! ```
//! use jet_grid::core::GapBuffer;
//!
//! let mut buffer = GapBuffer::with_size(5);
//! assert_eq!(buffer.logical_to_physical(2), Some(2)); // Initially 1:1 mapping
//!
//! buffer.insert_at(2, 2); // Insert 2 rows at position 2
//! assert_eq!(buffer.len(), 7); // Now we have 7 logical rows
//! assert_eq!(buffer.logical_to_physical(0), Some(0)); // Row 0 unchanged
//! assert_eq!(buffer.logical_to_physical(1), Some(1)); // Row 1 unchanged
//! assert_eq!(buffer.logical_to_physical(2), Some(5)); // New row 2 gets physical index 5
//! assert_eq!(buffer.logical_to_physical(3), Some(6)); // New row 3 gets physical index 6
//! assert_eq!(buffer.logical_to_physical(4), Some(2)); // Old row 2 shifted to logical 4
//! ```

use std::collections::BTreeMap;

/// Gap Buffer for efficient index mapping during insertions and deletions.
///
/// This structure maintains bidirectional mappings between logical indices (user-visible)
/// and physical indices (actual storage location). When rows or columns are inserted or deleted,
/// only the mappings are updated, avoiding expensive data movement operations.
#[derive(Debug, Clone)]
pub struct GapBuffer {
    /// Maps logical index to physical storage index
    logical_to_physical_map: BTreeMap<usize, usize>,
    /// Maps physical storage index to logical index (reverse mapping)
    physical_to_logical_map: BTreeMap<usize, usize>,
    /// Next available physical index for new insertions
    next_physical: usize,
    /// Current logical size (number of visible rows/cols)
    logical_size: usize,
    /// Pool of deleted physical indices available for reuse
    deleted_physical: Vec<usize>,
}

impl GapBuffer {
    /// Creates an empty gap buffer.
    pub fn new() -> Self {
        Self {
            logical_to_physical_map: BTreeMap::new(),
            physical_to_logical_map: BTreeMap::new(),
            next_physical: 0,
            logical_size: 0,
            deleted_physical: Vec::new(),
        }
    }

    /// Creates a gap buffer initialized with a 1:1 mapping for `size` elements.
    ///
    /// # Example
    ///
    /// ```
    /// use jet_grid::core::GapBuffer;
    ///
    /// let buffer = GapBuffer::with_size(5);
    /// assert_eq!(buffer.len(), 5);
    /// assert_eq!(buffer.logical_to_physical(0), Some(0));
    /// assert_eq!(buffer.logical_to_physical(4), Some(4));
    /// ```
    pub fn with_size(size: usize) -> Self {
        let mut buffer = Self::new();
        for i in 0..size {
            buffer.logical_to_physical_map.insert(i, i);
            buffer.physical_to_logical_map.insert(i, i);
        }
        buffer.next_physical = size;
        buffer.logical_size = size;
        buffer
    }

    /// Inserts `count` new logical indices at the specified position.
    ///
    /// All logical indices >= `logical_index` are shifted up by `count`.
    /// New physical indices are allocated for the inserted logical indices.
    ///
    /// # Panics
    ///
    /// Panics if `logical_index > self.len()`.
    ///
    /// # Example
    ///
    /// ```
    /// use jet_grid::core::GapBuffer;
    ///
    /// let mut buffer = GapBuffer::with_size(3);
    /// buffer.insert_at(1, 2); // Insert 2 elements at position 1
    /// assert_eq!(buffer.len(), 5);
    /// assert_eq!(buffer.logical_to_physical(0), Some(0)); // Unchanged
    /// assert_eq!(buffer.logical_to_physical(1), Some(3)); // New element
    /// assert_eq!(buffer.logical_to_physical(2), Some(4)); // New element
    /// assert_eq!(buffer.logical_to_physical(3), Some(1)); // Shifted from 1
    /// assert_eq!(buffer.logical_to_physical(4), Some(2)); // Shifted from 2
    /// ```
    pub fn insert_at(&mut self, logical_index: usize, count: usize) {
        assert!(
            logical_index <= self.logical_size,
            "insert_at: logical_index {} out of bounds (size: {})",
            logical_index,
            self.logical_size
        );

        if count == 0 {
            return;
        }

        // Step 1: Shift existing mappings up
        // We need to iterate from highest to lowest to avoid conflicts
        let indices_to_shift: Vec<usize> = self
            .logical_to_physical_map
            .range(logical_index..)
            .map(|(&k, _)| k)
            .collect();

        for old_logical in indices_to_shift.iter().rev() {
            let physical = self.logical_to_physical_map.remove(old_logical).unwrap();
            let new_logical = old_logical + count;
            self.logical_to_physical_map.insert(new_logical, physical);
            self.physical_to_logical_map.insert(physical, new_logical);
        }

        // Step 2: Allocate new physical indices for inserted logical indices
        for i in 0..count {
            let new_logical = logical_index + i;
            let new_physical = self.allocate_physical_index();
            self.logical_to_physical_map
                .insert(new_logical, new_physical);
            self.physical_to_logical_map
                .insert(new_physical, new_logical);
        }

        self.logical_size += count;
    }

    /// Deletes `count` logical indices starting at the specified position.
    ///
    /// Physical indices are reclaimed for reuse.
    /// All logical indices > `logical_index + count` are shifted down by `count`.
    ///
    /// # Panics
    ///
    /// Panics if the deletion range is out of bounds.
    ///
    /// # Example
    ///
    /// ```
    /// use jet_grid::core::GapBuffer;
    ///
    /// let mut buffer = GapBuffer::with_size(5);
    /// buffer.delete_at(1, 2); // Delete 2 elements starting at position 1
    /// assert_eq!(buffer.len(), 3);
    /// assert_eq!(buffer.logical_to_physical(0), Some(0)); // Unchanged
    /// assert_eq!(buffer.logical_to_physical(1), Some(3)); // Was at 3, shifted down
    /// assert_eq!(buffer.logical_to_physical(2), Some(4)); // Was at 4, shifted down
    /// ```
    pub fn delete_at(&mut self, logical_index: usize, count: usize) {
        assert!(
            logical_index < self.logical_size,
            "delete_at: logical_index {} out of bounds (size: {})",
            logical_index,
            self.logical_size
        );
        assert!(
            logical_index + count <= self.logical_size,
            "delete_at: deletion range [{}, {}) out of bounds (size: {})",
            logical_index,
            logical_index + count,
            self.logical_size
        );

        if count == 0 {
            return;
        }

        // Step 1: Remove mappings and reclaim physical indices
        for i in 0..count {
            let logical = logical_index + i;
            if let Some(physical) = self.logical_to_physical_map.remove(&logical) {
                self.physical_to_logical_map.remove(&physical);
                self.deleted_physical.push(physical);
            }
        }

        // Step 2: Shift remaining mappings down
        let indices_to_shift: Vec<usize> = self
            .logical_to_physical_map
            .range((logical_index + count)..)
            .map(|(&k, _)| k)
            .collect();

        for old_logical in indices_to_shift {
            let physical = self.logical_to_physical_map.remove(&old_logical).unwrap();
            let new_logical = old_logical - count;
            self.logical_to_physical_map.insert(new_logical, physical);
            self.physical_to_logical_map.insert(physical, new_logical);
        }

        self.logical_size -= count;
    }

    /// Maps a logical index to its corresponding physical index.
    ///
    /// Returns `None` if the logical index is out of bounds.
    ///
    /// # Example
    ///
    /// ```
    /// use jet_grid::core::GapBuffer;
    ///
    /// let buffer = GapBuffer::with_size(3);
    /// assert_eq!(buffer.logical_to_physical(0), Some(0));
    /// assert_eq!(buffer.logical_to_physical(2), Some(2));
    /// assert_eq!(buffer.logical_to_physical(3), None);
    /// ```
    pub fn logical_to_physical(&self, logical: usize) -> Option<usize> {
        self.logical_to_physical_map.get(&logical).copied()
    }

    /// Maps a physical index to its corresponding logical index.
    ///
    /// Returns `None` if the physical index is not currently in use.
    ///
    /// # Example
    ///
    /// ```
    /// use jet_grid::core::GapBuffer;
    ///
    /// let buffer = GapBuffer::with_size(3);
    /// assert_eq!(buffer.physical_to_logical(0), Some(0));
    /// assert_eq!(buffer.physical_to_logical(2), Some(2));
    /// assert_eq!(buffer.physical_to_logical(5), None);
    /// ```
    pub fn physical_to_logical(&self, physical: usize) -> Option<usize> {
        self.physical_to_logical_map.get(&physical).copied()
    }

    /// Returns the current logical size (number of visible elements).
    pub fn len(&self) -> usize {
        self.logical_size
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.logical_size == 0
    }

    /// Returns the highest physical index + 1.
    ///
    /// This represents the capacity of the underlying physical storage.
    pub fn physical_capacity(&self) -> usize {
        self.next_physical
    }

    /// Allocates a new physical index, reusing deleted ones if available.
    fn allocate_physical_index(&mut self) -> usize {
        if let Some(physical) = self.deleted_physical.pop() {
            physical
        } else {
            let physical = self.next_physical;
            self.next_physical += 1;
            physical
        }
    }
}

impl Default for GapBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let buffer = GapBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.physical_capacity(), 0);
    }

    #[test]
    fn test_with_size_initial_mapping() {
        let buffer = GapBuffer::with_size(5);
        assert_eq!(buffer.len(), 5);
        assert!(!buffer.is_empty());

        // Initial mapping should be 1:1
        for i in 0..5 {
            assert_eq!(buffer.logical_to_physical(i), Some(i));
            assert_eq!(buffer.physical_to_logical(i), Some(i));
        }

        assert_eq!(buffer.logical_to_physical(5), None);
        assert_eq!(buffer.physical_to_logical(5), None);
    }

    #[test]
    fn test_insert_at_middle() {
        let mut buffer = GapBuffer::with_size(5);
        buffer.insert_at(2, 2);

        assert_eq!(buffer.len(), 7);

        // Elements before insertion point should be unchanged
        assert_eq!(buffer.logical_to_physical(0), Some(0));
        assert_eq!(buffer.logical_to_physical(1), Some(1));

        // New elements get physical indices 5, 6
        assert_eq!(buffer.logical_to_physical(2), Some(5));
        assert_eq!(buffer.logical_to_physical(3), Some(6));

        // Elements after insertion point are shifted up logically
        assert_eq!(buffer.logical_to_physical(4), Some(2));
        assert_eq!(buffer.logical_to_physical(5), Some(3));
        assert_eq!(buffer.logical_to_physical(6), Some(4));
    }

    #[test]
    fn test_insert_at_beginning() {
        let mut buffer = GapBuffer::with_size(3);
        buffer.insert_at(0, 2);

        assert_eq!(buffer.len(), 5);

        // New elements at beginning
        assert_eq!(buffer.logical_to_physical(0), Some(3));
        assert_eq!(buffer.logical_to_physical(1), Some(4));

        // Original elements shifted up
        assert_eq!(buffer.logical_to_physical(2), Some(0));
        assert_eq!(buffer.logical_to_physical(3), Some(1));
        assert_eq!(buffer.logical_to_physical(4), Some(2));
    }

    #[test]
    fn test_insert_at_end() {
        let mut buffer = GapBuffer::with_size(3);
        buffer.insert_at(3, 2);

        assert_eq!(buffer.len(), 5);

        // Original elements unchanged
        assert_eq!(buffer.logical_to_physical(0), Some(0));
        assert_eq!(buffer.logical_to_physical(1), Some(1));
        assert_eq!(buffer.logical_to_physical(2), Some(2));

        // New elements at end
        assert_eq!(buffer.logical_to_physical(3), Some(3));
        assert_eq!(buffer.logical_to_physical(4), Some(4));
    }

    #[test]
    fn test_delete_at_middle() {
        let mut buffer = GapBuffer::with_size(5);
        buffer.delete_at(1, 2);

        assert_eq!(buffer.len(), 3);

        // Element before deletion point unchanged
        assert_eq!(buffer.logical_to_physical(0), Some(0));

        // Elements after deletion point shifted down
        assert_eq!(buffer.logical_to_physical(1), Some(3));
        assert_eq!(buffer.logical_to_physical(2), Some(4));

        // Deleted indices should not be mapped
        assert_eq!(buffer.physical_to_logical(1), None);
        assert_eq!(buffer.physical_to_logical(2), None);
    }

    #[test]
    fn test_delete_at_beginning() {
        let mut buffer = GapBuffer::with_size(5);
        buffer.delete_at(0, 2);

        assert_eq!(buffer.len(), 3);

        // Remaining elements shifted down
        assert_eq!(buffer.logical_to_physical(0), Some(2));
        assert_eq!(buffer.logical_to_physical(1), Some(3));
        assert_eq!(buffer.logical_to_physical(2), Some(4));
    }

    #[test]
    fn test_delete_at_end() {
        let mut buffer = GapBuffer::with_size(5);
        buffer.delete_at(3, 2);

        assert_eq!(buffer.len(), 3);

        // Remaining elements unchanged
        assert_eq!(buffer.logical_to_physical(0), Some(0));
        assert_eq!(buffer.logical_to_physical(1), Some(1));
        assert_eq!(buffer.logical_to_physical(2), Some(2));
    }

    #[test]
    fn test_multiple_operations() {
        let mut buffer = GapBuffer::with_size(5);

        // Insert 2 at position 2
        buffer.insert_at(2, 2);
        assert_eq!(buffer.len(), 7);

        // Delete 1 at position 3
        buffer.delete_at(3, 1);
        assert_eq!(buffer.len(), 6);

        // Insert 1 at position 0
        buffer.insert_at(0, 1);
        assert_eq!(buffer.len(), 7);

        // Verify some mappings
        assert!(buffer.logical_to_physical(0).is_some());
        assert!(buffer.logical_to_physical(6).is_some());
        assert!(buffer.logical_to_physical(7).is_none());
    }

    #[test]
    fn test_physical_index_reuse() {
        let mut buffer = GapBuffer::with_size(5);

        // Delete some elements
        buffer.delete_at(1, 2);
        assert_eq!(buffer.len(), 3);

        // Physical indices 1 and 2 should be in deleted pool
        assert_eq!(buffer.deleted_physical.len(), 2);

        // Insert new elements - should reuse deleted physical indices
        buffer.insert_at(1, 1);
        assert_eq!(buffer.len(), 4);

        // One deleted physical index should be reused
        assert_eq!(buffer.deleted_physical.len(), 1);

        // The new element should have one of the deleted physical indices
        let physical = buffer.logical_to_physical(1).unwrap();
        assert!(physical == 1 || physical == 2);
    }

    #[test]
    fn test_bidirectional_mapping() {
        let mut buffer = GapBuffer::with_size(5);
        buffer.insert_at(2, 2);

        // Check that logical->physical and physical->logical are consistent
        for logical in 0..buffer.len() {
            let physical = buffer.logical_to_physical(logical).unwrap();
            assert_eq!(buffer.physical_to_logical(physical), Some(logical));
        }
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_insert_at_out_of_bounds() {
        let mut buffer = GapBuffer::with_size(3);
        buffer.insert_at(4, 1); // Should panic
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_delete_at_out_of_bounds() {
        let mut buffer = GapBuffer::with_size(3);
        buffer.delete_at(3, 1); // Should panic
    }

    #[test]
    #[should_panic(expected = "out of bounds")]
    fn test_delete_range_out_of_bounds() {
        let mut buffer = GapBuffer::with_size(3);
        buffer.delete_at(2, 2); // Range [2, 4) is out of bounds
    }

    #[test]
    fn test_insert_zero_count() {
        let mut buffer = GapBuffer::with_size(3);
        buffer.insert_at(1, 0);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.logical_to_physical(0), Some(0));
        assert_eq!(buffer.logical_to_physical(1), Some(1));
        assert_eq!(buffer.logical_to_physical(2), Some(2));
    }

    #[test]
    fn test_delete_zero_count() {
        let mut buffer = GapBuffer::with_size(3);
        buffer.delete_at(1, 0);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.logical_to_physical(0), Some(0));
        assert_eq!(buffer.logical_to_physical(1), Some(1));
        assert_eq!(buffer.logical_to_physical(2), Some(2));
    }
}
