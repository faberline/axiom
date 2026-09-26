use serde::{Deserialize, Serialize};

use super::selection::SelectionRange;

/// Clipboard mode determines what operation was last performed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ClipboardMode {
    /// Clipboard is empty
    #[default]
    Empty,
    /// Content was copied
    Copy,
    /// Content was cut
    Cut,
}

/// Represents clipboard data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardData {
    /// The range that was copied/cut
    pub range: SelectionRange,
    /// The mode (copy or cut)
    pub mode: ClipboardMode,
}

/// Manages the clipboard state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardState {
    data: Option<ClipboardData>,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardState {
    pub fn new() -> Self {
        Self { data: None }
    }

    /// Copy a range to the clipboard
    pub fn copy(&mut self, range: SelectionRange) {
        self.data = Some(ClipboardData {
            range,
            mode: ClipboardMode::Copy,
        });
    }

    /// Cut a range to the clipboard
    pub fn cut(&mut self, range: SelectionRange) {
        self.data = Some(ClipboardData {
            range,
            mode: ClipboardMode::Cut,
        });
    }

    /// Get the clipboard data for pasting
    pub fn paste(&self) -> Option<&ClipboardData> {
        self.data.as_ref()
    }

    /// Clear the clipboard
    pub fn clear(&mut self) {
        self.data = None;
    }

    /// Check if clipboard has content
    pub fn has_content(&self) -> bool {
        self.data.is_some()
    }

    /// Get the current mode
    pub fn mode(&self) -> ClipboardMode {
        self.data
            .as_ref()
            .map(|d| d.mode)
            .unwrap_or(ClipboardMode::Empty)
    }

    /// Get the current range if any
    pub fn range(&self) -> Option<SelectionRange> {
        self.data.as_ref().map(|d| d.range)
    }

    /// Check if the clipboard contains a copy operation
    pub fn is_copy(&self) -> bool {
        matches!(self.mode(), ClipboardMode::Copy)
    }

    /// Check if the clipboard contains a cut operation
    pub fn is_cut(&self) -> bool {
        matches!(self.mode(), ClipboardMode::Cut)
    }

    /// Check if the clipboard is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_none()
    }

    /// Clear the clipboard if it contains a cut operation
    /// (cut operations should only be pasted once)
    pub fn clear_if_cut(&mut self) {
        if self.is_cut() {
            self.clear();
        }
    }

    /// Get the size of the clipboard range
    pub fn size(&self) -> Option<(usize, usize)> {
        self.data
            .as_ref()
            .map(|d| (d.range.row_count(), d.range.col_count()))
    }
}

#[cfg(test)]
mod tests {
    use super::super::selection::CellPosition;
    use super::*;

    #[test]
    fn test_clipboard_mode_default() {
        assert_eq!(ClipboardMode::default(), ClipboardMode::Empty);
    }

    #[test]
    fn test_clipboard_mode_variants() {
        assert_ne!(ClipboardMode::Empty, ClipboardMode::Copy);
        assert_ne!(ClipboardMode::Empty, ClipboardMode::Cut);
        assert_ne!(ClipboardMode::Copy, ClipboardMode::Cut);
    }

    #[test]
    fn test_clipboard_state_new() {
        let state = ClipboardState::new();
        assert!(!state.has_content());
        assert!(state.is_empty());
        assert_eq!(state.mode(), ClipboardMode::Empty);
    }

    #[test]
    fn test_clipboard_state_default() {
        let state = ClipboardState::default();
        assert_eq!(state, ClipboardState::new());
    }

    #[test]
    fn test_copy() {
        let mut state = ClipboardState::new();
        let range = SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(5, 5));

        state.copy(range);

        assert!(state.has_content());
        assert!(!state.is_empty());
        assert_eq!(state.mode(), ClipboardMode::Copy);
        assert!(state.is_copy());
        assert!(!state.is_cut());
        assert_eq!(state.range(), Some(range));
    }

    #[test]
    fn test_cut() {
        let mut state = ClipboardState::new();
        let range = SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(5, 5));

        state.cut(range);

        assert!(state.has_content());
        assert!(!state.is_empty());
        assert_eq!(state.mode(), ClipboardMode::Cut);
        assert!(!state.is_copy());
        assert!(state.is_cut());
        assert_eq!(state.range(), Some(range));
    }

    #[test]
    fn test_paste() {
        let mut state = ClipboardState::new();
        let range = SelectionRange::new(CellPosition::new(1, 2), CellPosition::new(3, 4));

        state.copy(range);

        let paste_data = state.paste();
        assert!(paste_data.is_some());
        let data = paste_data.unwrap();
        assert_eq!(data.range, range);
        assert_eq!(data.mode, ClipboardMode::Copy);
    }

    #[test]
    fn test_paste_empty() {
        let state = ClipboardState::new();
        assert!(state.paste().is_none());
    }

    #[test]
    fn test_clear() {
        let mut state = ClipboardState::new();
        state.copy(SelectionRange::new(
            CellPosition::new(0, 0),
            CellPosition::new(5, 5),
        ));

        assert!(state.has_content());

        state.clear();

        assert!(!state.has_content());
        assert!(state.is_empty());
        assert_eq!(state.mode(), ClipboardMode::Empty);
        assert!(state.paste().is_none());
    }

    #[test]
    fn test_has_content() {
        let mut state = ClipboardState::new();
        assert!(!state.has_content());

        state.copy(SelectionRange::new(
            CellPosition::new(0, 0),
            CellPosition::new(1, 1),
        ));
        assert!(state.has_content());

        state.clear();
        assert!(!state.has_content());
    }

    #[test]
    fn test_copy_overwrites_previous() {
        let mut state = ClipboardState::new();

        let range1 = SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(2, 2));
        state.copy(range1);

        let range2 = SelectionRange::new(CellPosition::new(5, 5), CellPosition::new(8, 8));
        state.copy(range2);

        assert_eq!(state.range(), Some(range2));
        assert!(state.is_copy());
    }

    #[test]
    fn test_cut_overwrites_copy() {
        let mut state = ClipboardState::new();

        let range1 = SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(2, 2));
        state.copy(range1);

        let range2 = SelectionRange::new(CellPosition::new(5, 5), CellPosition::new(8, 8));
        state.cut(range2);

        assert_eq!(state.range(), Some(range2));
        assert!(state.is_cut());
        assert!(!state.is_copy());
    }

    #[test]
    fn test_copy_overwrites_cut() {
        let mut state = ClipboardState::new();

        let range1 = SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(2, 2));
        state.cut(range1);

        let range2 = SelectionRange::new(CellPosition::new(5, 5), CellPosition::new(8, 8));
        state.copy(range2);

        assert_eq!(state.range(), Some(range2));
        assert!(state.is_copy());
        assert!(!state.is_cut());
    }

    #[test]
    fn test_clear_if_cut_with_cut() {
        let mut state = ClipboardState::new();
        state.cut(SelectionRange::new(
            CellPosition::new(0, 0),
            CellPosition::new(5, 5),
        ));

        state.clear_if_cut();

        assert!(state.is_empty());
        assert!(!state.has_content());
    }

    #[test]
    fn test_clear_if_cut_with_copy() {
        let mut state = ClipboardState::new();
        let range = SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(5, 5));
        state.copy(range);

        state.clear_if_cut();

        assert!(!state.is_empty());
        assert!(state.has_content());
        assert_eq!(state.range(), Some(range));
    }

    #[test]
    fn test_clear_if_cut_when_empty() {
        let mut state = ClipboardState::new();
        state.clear_if_cut();
        assert!(state.is_empty());
    }

    #[test]
    fn test_size() {
        let mut state = ClipboardState::new();

        // Empty clipboard
        assert_eq!(state.size(), None);

        // 3x4 range
        state.copy(SelectionRange::new(
            CellPosition::new(2, 3),
            CellPosition::new(4, 6),
        ));
        assert_eq!(state.size(), Some((3, 4)));

        // Single cell
        state.copy(SelectionRange::single_cell(CellPosition::new(5, 5)));
        assert_eq!(state.size(), Some((1, 1)));
    }

    #[test]
    fn test_size_with_reversed_range() {
        let mut state = ClipboardState::new();

        // Range selected from bottom-right to top-left
        state.copy(SelectionRange::new(
            CellPosition::new(10, 10),
            CellPosition::new(5, 5),
        ));

        assert_eq!(state.size(), Some((6, 6)));
    }

    #[test]
    fn test_clipboard_data_equality() {
        let range = SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(5, 5));

        let data1 = ClipboardData {
            range,
            mode: ClipboardMode::Copy,
        };

        let data2 = ClipboardData {
            range,
            mode: ClipboardMode::Copy,
        };

        let data3 = ClipboardData {
            range,
            mode: ClipboardMode::Cut,
        };

        assert_eq!(data1, data2);
        assert_ne!(data1, data3);
    }

    #[test]
    fn test_multiple_paste_copy() {
        let mut state = ClipboardState::new();
        let range = SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(5, 5));
        state.copy(range);

        // Copy can be pasted multiple times
        assert!(state.paste().is_some());
        assert!(state.paste().is_some());
        assert!(state.paste().is_some());
        assert!(state.has_content());
    }

    #[test]
    fn test_paste_workflow() {
        let mut state = ClipboardState::new();
        let range = SelectionRange::new(CellPosition::new(0, 0), CellPosition::new(5, 5));

        // Cut and paste once
        state.cut(range);
        assert!(state.paste().is_some());
        state.clear_if_cut();
        assert!(state.is_empty());

        // Copy and paste multiple times
        state.copy(range);
        assert!(state.paste().is_some());
        assert!(state.paste().is_some());
        state.clear_if_cut(); // Should not clear
        assert!(state.has_content());
    }

    #[test]
    fn test_serialization() {
        let mut state = ClipboardState::new();
        state.copy(SelectionRange::new(
            CellPosition::new(5, 10),
            CellPosition::new(15, 20),
        ));

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: ClipboardState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_clipboard_mode_serialization() {
        let mode = ClipboardMode::Copy;
        let serialized = serde_json::to_string(&mode).unwrap();
        let deserialized: ClipboardMode = serde_json::from_str(&serialized).unwrap();
        assert_eq!(mode, deserialized);
    }
}
