use serde::{Deserialize, Serialize};

/// Represents a single cell position in the spreadsheet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellPosition {
    pub row: usize,
    pub col: usize,
}

impl CellPosition {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }

    pub fn origin() -> Self {
        Self { row: 0, col: 0 }
    }
}

/// Represents a rectangular range of cells
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionRange {
    pub start: CellPosition,
    pub end: CellPosition,
}

impl SelectionRange {
    pub fn new(start: CellPosition, end: CellPosition) -> Self {
        Self { start, end }
    }

    pub fn single_cell(pos: CellPosition) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    pub fn from_row(row: usize, max_col: usize) -> Self {
        Self {
            start: CellPosition::new(row, 0),
            end: CellPosition::new(row, max_col),
        }
    }

    pub fn from_column(col: usize, max_row: usize) -> Self {
        Self {
            start: CellPosition::new(0, col),
            end: CellPosition::new(max_row, col),
        }
    }

    pub fn all(max_row: usize, max_col: usize) -> Self {
        Self {
            start: CellPosition::origin(),
            end: CellPosition::new(max_row, max_col),
        }
    }

    /// Returns the normalized range (top-left to bottom-right)
    pub fn normalize(&self) -> Self {
        let min_row = self.start.row.min(self.end.row);
        let max_row = self.start.row.max(self.end.row);
        let min_col = self.start.col.min(self.end.col);
        let max_col = self.start.col.max(self.end.col);

        Self {
            start: CellPosition::new(min_row, min_col),
            end: CellPosition::new(max_row, max_col),
        }
    }

    /// Check if a position is within this range
    pub fn contains(&self, pos: CellPosition) -> bool {
        let normalized = self.normalize();
        pos.row >= normalized.start.row
            && pos.row <= normalized.end.row
            && pos.col >= normalized.start.col
            && pos.col <= normalized.end.col
    }

    /// Get the top-left corner of the range
    pub fn top_left(&self) -> CellPosition {
        self.normalize().start
    }

    /// Get the bottom-right corner of the range
    pub fn bottom_right(&self) -> CellPosition {
        self.normalize().end
    }

    /// Get the number of rows in the range
    pub fn row_count(&self) -> usize {
        let normalized = self.normalize();
        normalized.end.row - normalized.start.row + 1
    }

    /// Get the number of columns in the range
    pub fn col_count(&self) -> usize {
        let normalized = self.normalize();
        normalized.end.col - normalized.start.col + 1
    }

    /// Get the total number of cells in the range
    pub fn cell_count(&self) -> usize {
        self.row_count() * self.col_count()
    }
}

/// Selection mode determines how selections are modified
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    /// Normal selection (replace existing)
    Normal,
    /// Extend the current selection
    Extend,
    /// Add a new range to the selection
    Add,
}

/// Represents the current selection state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// Primary selection range (where the active cell is)
    primary: SelectionRange,
    /// Additional selection ranges (for multi-selection)
    additional: Vec<SelectionRange>,
    /// The active cell position (cursor position)
    active_cell: CellPosition,
}

impl Default for Selection {
    fn default() -> Self {
        Self::new()
    }
}

impl Selection {
    pub fn new() -> Self {
        let origin = CellPosition::origin();
        Self {
            primary: SelectionRange::single_cell(origin),
            additional: Vec::new(),
            active_cell: origin,
        }
    }

    pub fn select_cell(&mut self, pos: CellPosition) {
        self.primary = SelectionRange::single_cell(pos);
        self.additional.clear();
        self.active_cell = pos;
    }

    pub fn extend_to(&mut self, pos: CellPosition) {
        self.primary.end = pos;
        self.active_cell = pos;
    }

    pub fn add_range(&mut self, range: SelectionRange) {
        self.additional.push(range);
    }

    pub fn select_row(&mut self, row: usize, max_col: usize) {
        self.primary = SelectionRange::from_row(row, max_col);
        self.additional.clear();
        self.active_cell = CellPosition::new(row, 0);
    }

    pub fn select_column(&mut self, col: usize, max_row: usize) {
        self.primary = SelectionRange::from_column(col, max_row);
        self.additional.clear();
        self.active_cell = CellPosition::new(0, col);
    }

    pub fn select_all(&mut self, max_row: usize, max_col: usize) {
        self.primary = SelectionRange::all(max_row, max_col);
        self.additional.clear();
        self.active_cell = CellPosition::origin();
    }

    pub fn is_selected(&self, pos: CellPosition) -> bool {
        self.primary.contains(pos) || self.additional.iter().any(|r| r.contains(pos))
    }

    pub fn primary_range(&self) -> SelectionRange {
        self.primary
    }

    pub fn active_cell(&self) -> CellPosition {
        self.active_cell
    }

    pub fn all_ranges(&self) -> impl Iterator<Item = &SelectionRange> {
        std::iter::once(&self.primary).chain(self.additional.iter())
    }

    pub fn clear_additional(&mut self) {
        self.additional.clear();
    }

    pub fn has_multiple_ranges(&self) -> bool {
        !self.additional.is_empty()
    }

    pub fn total_cell_count(&self) -> usize {
        self.primary.cell_count()
            + self
                .additional
                .iter()
                .map(|r| r.cell_count())
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // CellPosition tests
    #[test]
    fn test_cell_position_new() {
        let pos = CellPosition::new(5, 10);
        assert_eq!(pos.row, 5);
        assert_eq!(pos.col, 10);
    }

    #[test]
    fn test_cell_position_origin() {
        let origin = CellPosition::origin();
        assert_eq!(origin.row, 0);
        assert_eq!(origin.col, 0);
    }

    #[test]
    fn test_cell_position_equality() {
        let pos1 = CellPosition::new(3, 4);
        let pos2 = CellPosition::new(3, 4);
        let pos3 = CellPosition::new(3, 5);
        assert_eq!(pos1, pos2);
        assert_ne!(pos1, pos3);
    }

    // SelectionRange tests
    #[test]
    fn test_selection_range_single_cell() {
        let pos = CellPosition::new(2, 3);
        let range = SelectionRange::single_cell(pos);
        assert_eq!(range.start, pos);
        assert_eq!(range.end, pos);
    }

    #[test]
    fn test_selection_range_from_row() {
        let range = SelectionRange::from_row(5, 100);
        assert_eq!(range.start.row, 5);
        assert_eq!(range.start.col, 0);
        assert_eq!(range.end.row, 5);
        assert_eq!(range.end.col, 100);
    }

    #[test]
    fn test_selection_range_from_column() {
        let range = SelectionRange::from_column(10, 200);
        assert_eq!(range.start.row, 0);
        assert_eq!(range.start.col, 10);
        assert_eq!(range.end.row, 200);
        assert_eq!(range.end.col, 10);
    }

    #[test]
    fn test_selection_range_all() {
        let range = SelectionRange::all(99, 25);
        assert_eq!(range.start, CellPosition::origin());
        assert_eq!(range.end, CellPosition::new(99, 25));
    }

    #[test]
    fn test_selection_range_normalize() {
        let range = SelectionRange::new(CellPosition::new(5, 10), CellPosition::new(2, 3));
        let normalized = range.normalize();
        assert_eq!(normalized.start, CellPosition::new(2, 3));
        assert_eq!(normalized.end, CellPosition::new(5, 10));
    }

    #[test]
    fn test_selection_range_normalize_already_normalized() {
        let range = SelectionRange::new(CellPosition::new(2, 3), CellPosition::new(5, 10));
        let normalized = range.normalize();
        assert_eq!(normalized, range);
    }

    #[test]
    fn test_selection_range_contains() {
        let range = SelectionRange::new(CellPosition::new(2, 3), CellPosition::new(5, 7));
        assert!(range.contains(CellPosition::new(3, 5)));
        assert!(range.contains(CellPosition::new(2, 3)));
        assert!(range.contains(CellPosition::new(5, 7)));
        assert!(!range.contains(CellPosition::new(1, 5)));
        assert!(!range.contains(CellPosition::new(6, 5)));
        assert!(!range.contains(CellPosition::new(3, 2)));
        assert!(!range.contains(CellPosition::new(3, 8)));
    }

    #[test]
    fn test_selection_range_contains_reversed() {
        let range = SelectionRange::new(CellPosition::new(5, 7), CellPosition::new(2, 3));
        assert!(range.contains(CellPosition::new(3, 5)));
        assert!(range.contains(CellPosition::new(2, 3)));
        assert!(range.contains(CellPosition::new(5, 7)));
    }

    #[test]
    fn test_selection_range_top_left() {
        let range = SelectionRange::new(CellPosition::new(5, 7), CellPosition::new(2, 3));
        assert_eq!(range.top_left(), CellPosition::new(2, 3));
    }

    #[test]
    fn test_selection_range_bottom_right() {
        let range = SelectionRange::new(CellPosition::new(5, 7), CellPosition::new(2, 3));
        assert_eq!(range.bottom_right(), CellPosition::new(5, 7));
    }

    #[test]
    fn test_selection_range_row_count() {
        let range = SelectionRange::new(CellPosition::new(2, 3), CellPosition::new(5, 7));
        assert_eq!(range.row_count(), 4);
    }

    #[test]
    fn test_selection_range_col_count() {
        let range = SelectionRange::new(CellPosition::new(2, 3), CellPosition::new(5, 7));
        assert_eq!(range.col_count(), 5);
    }

    #[test]
    fn test_selection_range_cell_count() {
        let range = SelectionRange::new(CellPosition::new(2, 3), CellPosition::new(5, 7));
        assert_eq!(range.cell_count(), 20);
    }

    #[test]
    fn test_selection_range_single_cell_count() {
        let range = SelectionRange::single_cell(CellPosition::new(3, 4));
        assert_eq!(range.cell_count(), 1);
    }

    // Selection tests
    #[test]
    fn test_selection_new() {
        let selection = Selection::new();
        assert_eq!(selection.active_cell(), CellPosition::origin());
        assert_eq!(
            selection.primary_range(),
            SelectionRange::single_cell(CellPosition::origin())
        );
        assert!(!selection.has_multiple_ranges());
    }

    #[test]
    fn test_selection_default() {
        let selection = Selection::default();
        assert_eq!(selection, Selection::new());
    }

    #[test]
    fn test_selection_select_cell() {
        let mut selection = Selection::new();
        let pos = CellPosition::new(5, 10);
        selection.select_cell(pos);
        assert_eq!(selection.active_cell(), pos);
        assert_eq!(selection.primary_range(), SelectionRange::single_cell(pos));
        assert!(!selection.has_multiple_ranges());
    }

    #[test]
    fn test_selection_select_cell_clears_additional() {
        let mut selection = Selection::new();
        selection.add_range(SelectionRange::new(
            CellPosition::new(10, 10),
            CellPosition::new(15, 15),
        ));
        assert!(selection.has_multiple_ranges());

        selection.select_cell(CellPosition::new(5, 5));
        assert!(!selection.has_multiple_ranges());
    }

    #[test]
    fn test_selection_extend_to() {
        let mut selection = Selection::new();
        selection.select_cell(CellPosition::new(2, 3));
        selection.extend_to(CellPosition::new(5, 7));

        assert_eq!(selection.active_cell(), CellPosition::new(5, 7));
        let expected_range = SelectionRange::new(CellPosition::new(2, 3), CellPosition::new(5, 7));
        assert_eq!(selection.primary_range(), expected_range);
    }

    #[test]
    fn test_selection_add_range() {
        let mut selection = Selection::new();
        let range = SelectionRange::new(CellPosition::new(10, 10), CellPosition::new(15, 15));
        selection.add_range(range);

        assert!(selection.has_multiple_ranges());
        assert_eq!(selection.all_ranges().count(), 2);
    }

    #[test]
    fn test_selection_select_row() {
        let mut selection = Selection::new();
        selection.select_row(5, 100);

        assert_eq!(selection.active_cell(), CellPosition::new(5, 0));
        assert_eq!(selection.primary_range(), SelectionRange::from_row(5, 100));
        assert!(!selection.has_multiple_ranges());
    }

    #[test]
    fn test_selection_select_column() {
        let mut selection = Selection::new();
        selection.select_column(10, 200);

        assert_eq!(selection.active_cell(), CellPosition::new(0, 10));
        assert_eq!(
            selection.primary_range(),
            SelectionRange::from_column(10, 200)
        );
        assert!(!selection.has_multiple_ranges());
    }

    #[test]
    fn test_selection_select_all() {
        let mut selection = Selection::new();
        selection.select_all(99, 25);

        assert_eq!(selection.active_cell(), CellPosition::origin());
        assert_eq!(selection.primary_range(), SelectionRange::all(99, 25));
        assert!(!selection.has_multiple_ranges());
    }

    #[test]
    fn test_selection_is_selected_primary() {
        let mut selection = Selection::new();
        selection.select_cell(CellPosition::new(5, 10));
        selection.extend_to(CellPosition::new(8, 12));

        assert!(selection.is_selected(CellPosition::new(6, 11)));
        assert!(selection.is_selected(CellPosition::new(5, 10)));
        assert!(selection.is_selected(CellPosition::new(8, 12)));
        assert!(!selection.is_selected(CellPosition::new(4, 10)));
        assert!(!selection.is_selected(CellPosition::new(9, 12)));
    }

    #[test]
    fn test_selection_is_selected_additional() {
        let mut selection = Selection::new();
        selection.select_cell(CellPosition::new(0, 0));
        selection.add_range(SelectionRange::new(
            CellPosition::new(10, 10),
            CellPosition::new(15, 15),
        ));

        assert!(selection.is_selected(CellPosition::new(0, 0)));
        assert!(selection.is_selected(CellPosition::new(12, 13)));
        assert!(!selection.is_selected(CellPosition::new(5, 5)));
    }

    #[test]
    fn test_selection_clear_additional() {
        let mut selection = Selection::new();
        selection.add_range(SelectionRange::new(
            CellPosition::new(10, 10),
            CellPosition::new(15, 15),
        ));
        assert!(selection.has_multiple_ranges());

        selection.clear_additional();
        assert!(!selection.has_multiple_ranges());
    }

    #[test]
    fn test_selection_all_ranges() {
        let mut selection = Selection::new();
        selection.select_cell(CellPosition::new(0, 0));
        selection.add_range(SelectionRange::new(
            CellPosition::new(5, 5),
            CellPosition::new(10, 10),
        ));
        selection.add_range(SelectionRange::new(
            CellPosition::new(20, 20),
            CellPosition::new(25, 25),
        ));

        let ranges: Vec<_> = selection.all_ranges().collect();
        assert_eq!(ranges.len(), 3);
    }

    #[test]
    fn test_selection_total_cell_count() {
        let mut selection = Selection::new();
        selection.select_cell(CellPosition::new(0, 0));
        selection.extend_to(CellPosition::new(2, 2)); // 3x3 = 9 cells
        selection.add_range(SelectionRange::new(
            CellPosition::new(5, 5),
            CellPosition::new(6, 6),
        )); // 2x2 = 4 cells

        assert_eq!(selection.total_cell_count(), 13);
    }

    #[test]
    fn test_selection_mode_variants() {
        assert_ne!(SelectionMode::Normal, SelectionMode::Extend);
        assert_ne!(SelectionMode::Normal, SelectionMode::Add);
        assert_ne!(SelectionMode::Extend, SelectionMode::Add);
    }

    #[test]
    fn test_serialization() {
        let mut selection = Selection::new();
        selection.select_cell(CellPosition::new(5, 10));

        let serialized = serde_json::to_string(&selection).unwrap();
        let deserialized: Selection = serde_json::from_str(&serialized).unwrap();

        assert_eq!(selection, deserialized);
    }
}
