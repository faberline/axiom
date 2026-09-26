pub mod clipboard;
pub mod edit;
pub mod input;
pub mod selection;
pub mod viewport;

pub use clipboard::{ClipboardData, ClipboardMode, ClipboardState};
pub use edit::{EditMode, EditState};
pub use input::{
    is_clipboard_action, is_editing_action, is_navigation_action, is_selection_action,
    key_to_action, start_edit_with_char, InputAction, Key, Modifiers,
};
pub use selection::{CellPosition, Selection, SelectionMode, SelectionRange};
pub use viewport::{ViewportState, VisibleRange};

use serde::{Deserialize, Serialize};

/// Main spreadsheet state combining all sub-states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpreadsheetState {
    pub selection: Selection,
    pub edit: EditState,
    pub clipboard: ClipboardState,
    pub viewport: ViewportState,
}

impl Default for SpreadsheetState {
    fn default() -> Self {
        Self::new()
    }
}

impl SpreadsheetState {
    pub fn new() -> Self {
        Self {
            selection: Selection::new(),
            edit: EditState::new(),
            clipboard: ClipboardState::new(),
            viewport: ViewportState::new(20, 10),
        }
    }

    pub fn with_viewport_size(visible_rows: usize, visible_cols: usize) -> Self {
        Self {
            selection: Selection::new(),
            edit: EditState::new(),
            clipboard: ClipboardState::new(),
            viewport: ViewportState::new(visible_rows, visible_cols),
        }
    }

    /// Handle a user input action and update state accordingly
    pub fn handle_action(&mut self, action: InputAction, max_row: usize, max_col: usize) {
        match action {
            // Navigation actions
            InputAction::MoveUp => self.move_selection(-1, 0, max_row, max_col),
            InputAction::MoveDown => self.move_selection(1, 0, max_row, max_col),
            InputAction::MoveLeft => self.move_selection(0, -1, max_row, max_col),
            InputAction::MoveRight => self.move_selection(0, 1, max_row, max_col),
            InputAction::MoveToStart => {
                let row = self.selection.active_cell().row;
                self.selection.select_cell(CellPosition::new(row, 0));
                self.viewport
                    .ensure_cell_visible(self.selection.active_cell());
            }
            InputAction::MoveToEnd => {
                let row = self.selection.active_cell().row;
                self.selection.select_cell(CellPosition::new(row, max_col));
                self.viewport
                    .ensure_cell_visible(self.selection.active_cell());
            }
            InputAction::MoveToTop => {
                let col = self.selection.active_cell().col;
                self.selection.select_cell(CellPosition::new(0, col));
                self.viewport
                    .ensure_cell_visible(self.selection.active_cell());
            }
            InputAction::MoveToBottom => {
                let col = self.selection.active_cell().col;
                self.selection.select_cell(CellPosition::new(max_row, col));
                self.viewport
                    .ensure_cell_visible(self.selection.active_cell());
            }
            InputAction::PageUp => {
                self.viewport.page_up();
                let (row, col) = self.viewport.scroll_position();
                self.selection.select_cell(CellPosition::new(row, col));
            }
            InputAction::PageDown => {
                self.viewport.page_down();
                let (row, col) = self.viewport.scroll_position();
                self.selection.select_cell(CellPosition::new(row, col));
            }

            // Selection extension actions
            InputAction::ExtendUp => self.extend_selection(-1, 0, max_row, max_col),
            InputAction::ExtendDown => self.extend_selection(1, 0, max_row, max_col),
            InputAction::ExtendLeft => self.extend_selection(0, -1, max_row, max_col),
            InputAction::ExtendRight => self.extend_selection(0, 1, max_row, max_col),
            InputAction::ExtendToStart => {
                let row = self.selection.active_cell().row;
                self.selection.extend_to(CellPosition::new(row, 0));
                self.viewport
                    .ensure_cell_visible(self.selection.active_cell());
            }
            InputAction::ExtendToEnd => {
                let row = self.selection.active_cell().row;
                self.selection.extend_to(CellPosition::new(row, max_col));
                self.viewport
                    .ensure_cell_visible(self.selection.active_cell());
            }
            InputAction::ExtendToTop => {
                let col = self.selection.active_cell().col;
                self.selection.extend_to(CellPosition::new(0, col));
                self.viewport
                    .ensure_cell_visible(self.selection.active_cell());
            }
            InputAction::ExtendToBottom => {
                let col = self.selection.active_cell().col;
                self.selection.extend_to(CellPosition::new(max_row, col));
                self.viewport
                    .ensure_cell_visible(self.selection.active_cell());
            }
            InputAction::SelectAll => {
                self.selection.select_all(max_row, max_col);
            }

            // Clipboard actions
            InputAction::Copy => {
                self.clipboard.copy(self.selection.primary_range());
            }
            InputAction::Cut => {
                self.clipboard.cut(self.selection.primary_range());
            }
            InputAction::Paste => {
                // Paste handling would need access to actual cell data
                // This is just state management
                self.clipboard.clear_if_cut();
            }

            // Other actions are handled elsewhere or not implemented
            _ => {}
        }
    }

    fn move_selection(
        &mut self,
        delta_row: isize,
        delta_col: isize,
        max_row: usize,
        max_col: usize,
    ) {
        let current = self.selection.active_cell();
        let new_row = ((current.row as isize + delta_row).max(0) as usize).min(max_row);
        let new_col = ((current.col as isize + delta_col).max(0) as usize).min(max_col);
        let new_pos = CellPosition::new(new_row, new_col);

        self.selection.select_cell(new_pos);
        self.viewport.ensure_cell_visible(new_pos);
    }

    fn extend_selection(
        &mut self,
        delta_row: isize,
        delta_col: isize,
        max_row: usize,
        max_col: usize,
    ) {
        let current = self.selection.active_cell();
        let new_row = ((current.row as isize + delta_row).max(0) as usize).min(max_row);
        let new_col = ((current.col as isize + delta_col).max(0) as usize).min(max_col);
        let new_pos = CellPosition::new(new_row, new_col);

        self.selection.extend_to(new_pos);
        self.viewport.ensure_cell_visible(new_pos);
    }

    /// Get the active cell position
    pub fn active_cell(&self) -> CellPosition {
        self.selection.active_cell()
    }

    /// Check if a cell is selected
    pub fn is_selected(&self, pos: CellPosition) -> bool {
        self.selection.is_selected(pos)
    }

    /// Check if currently editing
    pub fn is_editing(&self) -> bool {
        self.edit.is_editing()
    }

    /// Get the visible range
    pub fn visible_range(&self) -> VisibleRange {
        self.viewport.get_visible_range()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spreadsheet_state_new() {
        let state = SpreadsheetState::new();
        assert_eq!(state.active_cell(), CellPosition::origin());
        assert!(!state.is_editing());
        assert_eq!(state.viewport.dimensions(), (20, 10));
    }

    #[test]
    fn test_spreadsheet_state_default() {
        let state = SpreadsheetState::default();
        assert_eq!(state, SpreadsheetState::new());
    }

    #[test]
    fn test_spreadsheet_state_with_viewport_size() {
        let state = SpreadsheetState::with_viewport_size(30, 15);
        assert_eq!(state.viewport.dimensions(), (30, 15));
    }

    #[test]
    fn test_handle_move_down() {
        let mut state = SpreadsheetState::new();
        state.handle_action(InputAction::MoveDown, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(1, 0));
    }

    #[test]
    fn test_handle_move_right() {
        let mut state = SpreadsheetState::new();
        state.handle_action(InputAction::MoveRight, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(0, 1));
    }

    #[test]
    fn test_handle_move_up_at_top() {
        let mut state = SpreadsheetState::new();
        state.handle_action(InputAction::MoveUp, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(0, 0));
    }

    #[test]
    fn test_handle_move_left_at_left() {
        let mut state = SpreadsheetState::new();
        state.handle_action(InputAction::MoveLeft, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(0, 0));
    }

    #[test]
    fn test_handle_move_to_start() {
        let mut state = SpreadsheetState::new();
        state.selection.select_cell(CellPosition::new(5, 10));
        state.handle_action(InputAction::MoveToStart, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(5, 0));
    }

    #[test]
    fn test_handle_move_to_end() {
        let mut state = SpreadsheetState::new();
        state.selection.select_cell(CellPosition::new(5, 10));
        state.handle_action(InputAction::MoveToEnd, 100, 50);
        assert_eq!(state.active_cell(), CellPosition::new(5, 50));
    }

    #[test]
    fn test_handle_move_to_top() {
        let mut state = SpreadsheetState::new();
        state.selection.select_cell(CellPosition::new(20, 10));
        state.handle_action(InputAction::MoveToTop, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(0, 10));
    }

    #[test]
    fn test_handle_move_to_bottom() {
        let mut state = SpreadsheetState::new();
        state.selection.select_cell(CellPosition::new(5, 10));
        state.handle_action(InputAction::MoveToBottom, 100, 50);
        assert_eq!(state.active_cell(), CellPosition::new(100, 10));
    }

    #[test]
    fn test_handle_extend_down() {
        let mut state = SpreadsheetState::new();
        state.handle_action(InputAction::ExtendDown, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(1, 0));
        assert_eq!(
            state.selection.primary_range(),
            SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(1, 0))
        );
    }

    #[test]
    fn test_handle_extend_right() {
        let mut state = SpreadsheetState::new();
        state.handle_action(InputAction::ExtendRight, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(0, 1));
        assert_eq!(
            state.selection.primary_range(),
            SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(0, 1))
        );
    }

    #[test]
    fn test_handle_select_all() {
        let mut state = SpreadsheetState::new();
        state.handle_action(InputAction::SelectAll, 99, 25);
        assert_eq!(state.selection.primary_range(), SelectionRange::all(99, 25));
    }

    #[test]
    fn test_handle_copy() {
        let mut state = SpreadsheetState::new();
        state.selection.select_cell(CellPosition::new(5, 10));
        state.handle_action(InputAction::Copy, 100, 100);

        assert!(state.clipboard.has_content());
        assert!(state.clipboard.is_copy());
    }

    #[test]
    fn test_handle_cut() {
        let mut state = SpreadsheetState::new();
        state.selection.select_cell(CellPosition::new(5, 10));
        state.handle_action(InputAction::Cut, 100, 100);

        assert!(state.clipboard.has_content());
        assert!(state.clipboard.is_cut());
    }

    #[test]
    fn test_handle_paste_clears_cut() {
        let mut state = SpreadsheetState::new();
        state
            .clipboard
            .cut(SelectionRange::single_cell(CellPosition::new(5, 10)));

        state.handle_action(InputAction::Paste, 100, 100);

        assert!(!state.clipboard.has_content());
    }

    #[test]
    fn test_handle_paste_keeps_copy() {
        let mut state = SpreadsheetState::new();
        state
            .clipboard
            .copy(SelectionRange::single_cell(CellPosition::new(5, 10)));

        state.handle_action(InputAction::Paste, 100, 100);

        assert!(state.clipboard.has_content());
    }

    #[test]
    fn test_move_with_viewport_update() {
        let mut state = SpreadsheetState::new();

        // Move to a position outside the initial viewport
        state.selection.select_cell(CellPosition::new(50, 50));
        state.handle_action(InputAction::MoveDown, 100, 100);

        // Check that viewport was updated to keep cell visible
        assert!(state.viewport.is_visible(state.active_cell()));
    }

    #[test]
    fn test_is_selected() {
        let mut state = SpreadsheetState::new();
        state.selection.select_cell(CellPosition::new(5, 10));
        state.selection.extend_to(CellPosition::new(8, 12));

        assert!(state.is_selected(CellPosition::new(5, 10)));
        assert!(state.is_selected(CellPosition::new(7, 11)));
        assert!(!state.is_selected(CellPosition::new(4, 10)));
    }

    #[test]
    fn test_visible_range() {
        let state = SpreadsheetState::new();
        let range = state.visible_range();

        assert_eq!(range.first_row, 0);
        assert_eq!(range.first_col, 0);
    }

    #[test]
    fn test_page_down() {
        let mut state = SpreadsheetState::new();
        state.handle_action(InputAction::PageDown, 100, 100);

        let (row, col) = state.viewport.scroll_position();
        assert_eq!(row, 20); // Moved by viewport height
        assert_eq!(state.active_cell(), CellPosition::new(row, col));
    }

    #[test]
    fn test_page_up() {
        let mut state = SpreadsheetState::new();
        state.viewport.scroll_to(40, 0);
        state.selection.select_cell(CellPosition::new(40, 0));

        state.handle_action(InputAction::PageUp, 100, 100);

        let (row, col) = state.viewport.scroll_position();
        assert_eq!(row, 20);
        assert_eq!(state.active_cell(), CellPosition::new(row, col));
    }

    #[test]
    fn test_serialization() {
        let state = SpreadsheetState::new();

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: SpreadsheetState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_complex_navigation_sequence() {
        let mut state = SpreadsheetState::new();

        // Move right 5 times
        for _ in 0..5 {
            state.handle_action(InputAction::MoveRight, 100, 100);
        }
        assert_eq!(state.active_cell(), CellPosition::new(0, 5));

        // Move down 3 times
        for _ in 0..3 {
            state.handle_action(InputAction::MoveDown, 100, 100);
        }
        assert_eq!(state.active_cell(), CellPosition::new(3, 5));

        // Move to start of row
        state.handle_action(InputAction::MoveToStart, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(3, 0));

        // Move to top
        state.handle_action(InputAction::MoveToTop, 100, 100);
        assert_eq!(state.active_cell(), CellPosition::new(0, 0));
    }

    #[test]
    fn test_boundary_conditions() {
        let mut state = SpreadsheetState::new();
        let max_row = 10;
        let max_col = 10;

        // Try to move beyond bottom-right corner
        state
            .selection
            .select_cell(CellPosition::new(max_row, max_col));
        state.handle_action(InputAction::MoveDown, max_row, max_col);
        assert_eq!(state.active_cell(), CellPosition::new(max_row, max_col));

        state.handle_action(InputAction::MoveRight, max_row, max_col);
        assert_eq!(state.active_cell(), CellPosition::new(max_row, max_col));
    }
}
