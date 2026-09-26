use serde::{Deserialize, Serialize};

use super::selection::CellPosition;

/// Represents a rectangular viewport in the spreadsheet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibleRange {
    pub first_row: usize,
    pub last_row: usize,
    pub first_col: usize,
    pub last_col: usize,
}

impl VisibleRange {
    pub fn new(first_row: usize, last_row: usize, first_col: usize, last_col: usize) -> Self {
        Self {
            first_row,
            last_row,
            first_col,
            last_col,
        }
    }

    pub fn contains(&self, pos: CellPosition) -> bool {
        pos.row >= self.first_row
            && pos.row <= self.last_row
            && pos.col >= self.first_col
            && pos.col <= self.last_col
    }

    pub fn row_count(&self) -> usize {
        self.last_row.saturating_sub(self.first_row) + 1
    }

    pub fn col_count(&self) -> usize {
        self.last_col.saturating_sub(self.first_col) + 1
    }
}

/// Manages the viewport and scroll state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewportState {
    /// Top-left corner of the viewport
    scroll_row: usize,
    scroll_col: usize,
    /// Number of visible rows
    visible_rows: usize,
    /// Number of visible columns
    visible_cols: usize,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self::new(20, 10)
    }
}

impl ViewportState {
    pub fn new(visible_rows: usize, visible_cols: usize) -> Self {
        Self {
            scroll_row: 0,
            scroll_col: 0,
            visible_rows,
            visible_cols,
        }
    }

    /// Scroll to a specific position (top-left corner)
    pub fn scroll_to(&mut self, row: usize, col: usize) {
        self.scroll_row = row;
        self.scroll_col = col;
    }

    /// Scroll by a relative amount
    pub fn scroll_by(&mut self, delta_row: isize, delta_col: isize) {
        self.scroll_row = (self.scroll_row as isize + delta_row).max(0) as usize;
        self.scroll_col = (self.scroll_col as isize + delta_col).max(0) as usize;
    }

    /// Ensure a cell is visible, scrolling if necessary
    pub fn ensure_cell_visible(&mut self, pos: CellPosition) {
        // Scroll vertically if needed
        if pos.row < self.scroll_row {
            self.scroll_row = pos.row;
        } else if pos.row >= self.scroll_row + self.visible_rows {
            self.scroll_row = pos.row.saturating_sub(self.visible_rows - 1);
        }

        // Scroll horizontally if needed
        if pos.col < self.scroll_col {
            self.scroll_col = pos.col;
        } else if pos.col >= self.scroll_col + self.visible_cols {
            self.scroll_col = pos.col.saturating_sub(self.visible_cols - 1);
        }
    }

    /// Get the current visible range
    pub fn get_visible_range(&self) -> VisibleRange {
        VisibleRange {
            first_row: self.scroll_row,
            last_row: self.scroll_row + self.visible_rows.saturating_sub(1),
            first_col: self.scroll_col,
            last_col: self.scroll_col + self.visible_cols.saturating_sub(1),
        }
    }

    /// Get the scroll position (top-left corner)
    pub fn scroll_position(&self) -> (usize, usize) {
        (self.scroll_row, self.scroll_col)
    }

    /// Get the viewport dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        (self.visible_rows, self.visible_cols)
    }

    /// Set the viewport dimensions
    pub fn set_dimensions(&mut self, visible_rows: usize, visible_cols: usize) {
        self.visible_rows = visible_rows;
        self.visible_cols = visible_cols;
    }

    /// Check if a cell is currently visible
    pub fn is_visible(&self, pos: CellPosition) -> bool {
        self.get_visible_range().contains(pos)
    }

    /// Scroll to show a specific cell at the top-left
    pub fn scroll_to_cell(&mut self, pos: CellPosition) {
        self.scroll_row = pos.row;
        self.scroll_col = pos.col;
    }

    /// Center the viewport on a specific cell
    pub fn center_on_cell(&mut self, pos: CellPosition) {
        self.scroll_row = pos.row.saturating_sub(self.visible_rows / 2);
        self.scroll_col = pos.col.saturating_sub(self.visible_cols / 2);
    }

    /// Scroll down by one page
    pub fn page_down(&mut self) {
        self.scroll_row += self.visible_rows;
    }

    /// Scroll up by one page
    pub fn page_up(&mut self) {
        self.scroll_row = self.scroll_row.saturating_sub(self.visible_rows);
    }

    /// Scroll right by one page
    pub fn page_right(&mut self) {
        self.scroll_col += self.visible_cols;
    }

    /// Scroll left by one page
    pub fn page_left(&mut self) {
        self.scroll_col = self.scroll_col.saturating_sub(self.visible_cols);
    }

    /// Reset scroll to origin
    pub fn reset(&mut self) {
        self.scroll_row = 0;
        self.scroll_col = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // VisibleRange tests
    #[test]
    fn test_visible_range_new() {
        let range = VisibleRange::new(5, 15, 10, 20);
        assert_eq!(range.first_row, 5);
        assert_eq!(range.last_row, 15);
        assert_eq!(range.first_col, 10);
        assert_eq!(range.last_col, 20);
    }

    #[test]
    fn test_visible_range_contains() {
        let range = VisibleRange::new(5, 15, 10, 20);

        assert!(range.contains(CellPosition::new(10, 15)));
        assert!(range.contains(CellPosition::new(5, 10)));
        assert!(range.contains(CellPosition::new(15, 20)));
        assert!(!range.contains(CellPosition::new(4, 15)));
        assert!(!range.contains(CellPosition::new(16, 15)));
        assert!(!range.contains(CellPosition::new(10, 9)));
        assert!(!range.contains(CellPosition::new(10, 21)));
    }

    #[test]
    fn test_visible_range_row_count() {
        let range = VisibleRange::new(5, 15, 10, 20);
        assert_eq!(range.row_count(), 11);
    }

    #[test]
    fn test_visible_range_col_count() {
        let range = VisibleRange::new(5, 15, 10, 20);
        assert_eq!(range.col_count(), 11);
    }

    #[test]
    fn test_visible_range_single_cell() {
        let range = VisibleRange::new(5, 5, 10, 10);
        assert_eq!(range.row_count(), 1);
        assert_eq!(range.col_count(), 1);
    }

    // ViewportState tests
    #[test]
    fn test_viewport_state_new() {
        let viewport = ViewportState::new(20, 10);
        assert_eq!(viewport.scroll_position(), (0, 0));
        assert_eq!(viewport.dimensions(), (20, 10));
    }

    #[test]
    fn test_viewport_state_default() {
        let viewport = ViewportState::default();
        assert_eq!(viewport.scroll_position(), (0, 0));
        assert_eq!(viewport.dimensions(), (20, 10));
    }

    #[test]
    fn test_scroll_to() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(5, 15);

        assert_eq!(viewport.scroll_position(), (5, 15));
    }

    #[test]
    fn test_scroll_by_positive() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 20);
        viewport.scroll_by(5, 3);

        assert_eq!(viewport.scroll_position(), (15, 23));
    }

    #[test]
    fn test_scroll_by_negative() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 20);
        viewport.scroll_by(-5, -10);

        assert_eq!(viewport.scroll_position(), (5, 10));
    }

    #[test]
    fn test_scroll_by_negative_clamped() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(5, 3);
        viewport.scroll_by(-10, -10);

        assert_eq!(viewport.scroll_position(), (0, 0));
    }

    #[test]
    fn test_get_visible_range() {
        let viewport = ViewportState::new(20, 10);
        let range = viewport.get_visible_range();

        assert_eq!(range.first_row, 0);
        assert_eq!(range.last_row, 19);
        assert_eq!(range.first_col, 0);
        assert_eq!(range.last_col, 9);
    }

    #[test]
    fn test_get_visible_range_scrolled() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        let range = viewport.get_visible_range();

        assert_eq!(range.first_row, 10);
        assert_eq!(range.last_row, 29);
        assert_eq!(range.first_col, 5);
        assert_eq!(range.last_col, 14);
    }

    #[test]
    fn test_is_visible() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);

        assert!(viewport.is_visible(CellPosition::new(15, 7)));
        assert!(viewport.is_visible(CellPosition::new(10, 5)));
        assert!(viewport.is_visible(CellPosition::new(29, 14)));
        assert!(!viewport.is_visible(CellPosition::new(9, 7)));
        assert!(!viewport.is_visible(CellPosition::new(30, 7)));
        assert!(!viewport.is_visible(CellPosition::new(15, 4)));
        assert!(!viewport.is_visible(CellPosition::new(15, 15)));
    }

    #[test]
    fn test_ensure_cell_visible_already_visible() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);

        let initial_pos = viewport.scroll_position();
        viewport.ensure_cell_visible(CellPosition::new(15, 7));

        assert_eq!(viewport.scroll_position(), initial_pos);
    }

    #[test]
    fn test_ensure_cell_visible_above() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        viewport.ensure_cell_visible(CellPosition::new(5, 7));

        assert_eq!(viewport.scroll_position(), (5, 5));
    }

    #[test]
    fn test_ensure_cell_visible_below() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        viewport.ensure_cell_visible(CellPosition::new(35, 7));

        assert_eq!(viewport.scroll_position(), (16, 5));
    }

    #[test]
    fn test_ensure_cell_visible_left() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        viewport.ensure_cell_visible(CellPosition::new(15, 2));

        assert_eq!(viewport.scroll_position(), (10, 2));
    }

    #[test]
    fn test_ensure_cell_visible_right() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        viewport.ensure_cell_visible(CellPosition::new(15, 20));

        assert_eq!(viewport.scroll_position(), (10, 11));
    }

    #[test]
    fn test_ensure_cell_visible_diagonal() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        viewport.ensure_cell_visible(CellPosition::new(5, 2));

        assert_eq!(viewport.scroll_position(), (5, 2));
    }

    #[test]
    fn test_set_dimensions() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.set_dimensions(30, 15);

        assert_eq!(viewport.dimensions(), (30, 15));
    }

    #[test]
    fn test_scroll_to_cell() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to_cell(CellPosition::new(15, 25));

        assert_eq!(viewport.scroll_position(), (15, 25));
    }

    #[test]
    fn test_center_on_cell() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.center_on_cell(CellPosition::new(50, 30));

        assert_eq!(viewport.scroll_position(), (40, 25));
    }

    #[test]
    fn test_center_on_cell_near_origin() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.center_on_cell(CellPosition::new(5, 3));

        assert_eq!(viewport.scroll_position(), (0, 0));
    }

    #[test]
    fn test_page_down() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        viewport.page_down();

        assert_eq!(viewport.scroll_position(), (30, 5));
    }

    #[test]
    fn test_page_up() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(30, 5);
        viewport.page_up();

        assert_eq!(viewport.scroll_position(), (10, 5));
    }

    #[test]
    fn test_page_up_clamped() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        viewport.page_up();

        assert_eq!(viewport.scroll_position(), (0, 5));
    }

    #[test]
    fn test_page_right() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        viewport.page_right();

        assert_eq!(viewport.scroll_position(), (10, 15));
    }

    #[test]
    fn test_page_left() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 15);
        viewport.page_left();

        assert_eq!(viewport.scroll_position(), (10, 5));
    }

    #[test]
    fn test_page_left_clamped() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(10, 5);
        viewport.page_left();

        assert_eq!(viewport.scroll_position(), (10, 0));
    }

    #[test]
    fn test_reset() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(50, 30);
        viewport.reset();

        assert_eq!(viewport.scroll_position(), (0, 0));
    }

    #[test]
    fn test_multiple_operations() {
        let mut viewport = ViewportState::new(20, 10);

        viewport.scroll_to(10, 5);
        viewport.page_down();
        assert_eq!(viewport.scroll_position(), (30, 5));

        viewport.page_right();
        assert_eq!(viewport.scroll_position(), (30, 15));

        viewport.scroll_by(-10, -5);
        assert_eq!(viewport.scroll_position(), (20, 10));

        viewport.center_on_cell(CellPosition::new(50, 50));
        assert_eq!(viewport.scroll_position(), (40, 45));

        viewport.reset();
        assert_eq!(viewport.scroll_position(), (0, 0));
    }

    #[test]
    fn test_serialization() {
        let mut viewport = ViewportState::new(20, 10);
        viewport.scroll_to(15, 25);

        let serialized = serde_json::to_string(&viewport).unwrap();
        let deserialized: ViewportState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(viewport, deserialized);
    }

    #[test]
    fn test_visible_range_serialization() {
        let range = VisibleRange::new(5, 15, 10, 20);

        let serialized = serde_json::to_string(&range).unwrap();
        let deserialized: VisibleRange = serde_json::from_str(&serialized).unwrap();

        assert_eq!(range, deserialized);
    }
}
