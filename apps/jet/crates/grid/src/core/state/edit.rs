use serde::{Deserialize, Serialize};

use super::selection::CellPosition;

/// Edit mode determines what the user is currently editing
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EditMode {
    /// Not editing, just viewing the spreadsheet
    #[default]
    Viewing,
    /// Editing a cell directly in the grid
    CellEditing {
        position: CellPosition,
        content: String,
    },
    /// Editing in the formula bar
    FormulaBarEditing {
        position: CellPosition,
        content: String,
    },
}

impl EditMode {
    pub fn is_viewing(&self) -> bool {
        matches!(self, EditMode::Viewing)
    }

    pub fn is_editing(&self) -> bool {
        !self.is_viewing()
    }

    pub fn is_cell_editing(&self) -> bool {
        matches!(self, EditMode::CellEditing { .. })
    }

    pub fn is_formula_bar_editing(&self) -> bool {
        matches!(self, EditMode::FormulaBarEditing { .. })
    }

    pub fn get_content(&self) -> Option<&str> {
        match self {
            EditMode::Viewing => None,
            EditMode::CellEditing { content, .. } => Some(content),
            EditMode::FormulaBarEditing { content, .. } => Some(content),
        }
    }

    pub fn get_position(&self) -> Option<CellPosition> {
        match self {
            EditMode::Viewing => None,
            EditMode::CellEditing { position, .. } => Some(*position),
            EditMode::FormulaBarEditing { position, .. } => Some(*position),
        }
    }
}

/// Manages the edit state of the spreadsheet
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditState {
    mode: EditMode,
    /// Original content before editing (for cancel operation)
    original_content: Option<String>,
}

impl Default for EditState {
    fn default() -> Self {
        Self::new()
    }
}

impl EditState {
    pub fn new() -> Self {
        Self {
            mode: EditMode::Viewing,
            original_content: None,
        }
    }

    /// Start editing a cell directly in the grid
    pub fn start_cell_edit(&mut self, position: CellPosition, initial_content: String) {
        self.original_content = Some(initial_content.clone());
        self.mode = EditMode::CellEditing {
            position,
            content: initial_content,
        };
    }

    /// Start editing in the formula bar
    pub fn start_formula_edit(&mut self, position: CellPosition, initial_content: String) {
        self.original_content = Some(initial_content.clone());
        self.mode = EditMode::FormulaBarEditing {
            position,
            content: initial_content,
        };
    }

    /// Update the content being edited
    pub fn update_value(&mut self, new_content: String) -> Result<(), String> {
        match &mut self.mode {
            EditMode::Viewing => Err("Cannot update value while in viewing mode".to_string()),
            EditMode::CellEditing { content, .. } => {
                *content = new_content;
                Ok(())
            }
            EditMode::FormulaBarEditing { content, .. } => {
                *content = new_content;
                Ok(())
            }
        }
    }

    /// Commit the current edit and return the new content
    pub fn commit(&mut self) -> Option<(CellPosition, String)> {
        match &self.mode {
            EditMode::Viewing => None,
            EditMode::CellEditing { position, content } => {
                let result = Some((*position, content.clone()));
                self.mode = EditMode::Viewing;
                self.original_content = None;
                result
            }
            EditMode::FormulaBarEditing { position, content } => {
                let result = Some((*position, content.clone()));
                self.mode = EditMode::Viewing;
                self.original_content = None;
                result
            }
        }
    }

    /// Cancel the current edit and return the original content
    pub fn cancel(&mut self) -> Option<String> {
        if self.mode.is_editing() {
            self.mode = EditMode::Viewing;
            self.original_content.take()
        } else {
            None
        }
    }

    /// Check if currently editing
    pub fn is_editing(&self) -> bool {
        self.mode.is_editing()
    }

    /// Get the current edit mode
    pub fn mode(&self) -> &EditMode {
        &self.mode
    }

    /// Get the current content being edited
    pub fn current_content(&self) -> Option<&str> {
        self.mode.get_content()
    }

    /// Get the position being edited
    pub fn editing_position(&self) -> Option<CellPosition> {
        self.mode.get_position()
    }

    /// Get the original content before editing
    pub fn original_content(&self) -> Option<&str> {
        self.original_content.as_deref()
    }

    /// Switch from cell editing to formula bar editing
    pub fn switch_to_formula_bar(&mut self) {
        if let EditMode::CellEditing { position, content } = &self.mode {
            self.mode = EditMode::FormulaBarEditing {
                position: *position,
                content: content.clone(),
            };
        }
    }

    /// Switch from formula bar editing to cell editing
    pub fn switch_to_cell(&mut self) {
        if let EditMode::FormulaBarEditing { position, content } = &self.mode {
            self.mode = EditMode::CellEditing {
                position: *position,
                content: content.clone(),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EditMode tests
    #[test]
    fn test_edit_mode_default() {
        let mode = EditMode::default();
        assert_eq!(mode, EditMode::Viewing);
    }

    #[test]
    fn test_edit_mode_is_viewing() {
        assert!(EditMode::Viewing.is_viewing());
        assert!(!EditMode::CellEditing {
            position: CellPosition::new(0, 0),
            content: String::new()
        }
        .is_viewing());
        assert!(!EditMode::FormulaBarEditing {
            position: CellPosition::new(0, 0),
            content: String::new()
        }
        .is_viewing());
    }

    #[test]
    fn test_edit_mode_is_editing() {
        assert!(!EditMode::Viewing.is_editing());
        assert!(EditMode::CellEditing {
            position: CellPosition::new(0, 0),
            content: String::new()
        }
        .is_editing());
        assert!(EditMode::FormulaBarEditing {
            position: CellPosition::new(0, 0),
            content: String::new()
        }
        .is_editing());
    }

    #[test]
    fn test_edit_mode_is_cell_editing() {
        assert!(!EditMode::Viewing.is_cell_editing());
        assert!(EditMode::CellEditing {
            position: CellPosition::new(0, 0),
            content: String::new()
        }
        .is_cell_editing());
        assert!(!EditMode::FormulaBarEditing {
            position: CellPosition::new(0, 0),
            content: String::new()
        }
        .is_cell_editing());
    }

    #[test]
    fn test_edit_mode_is_formula_bar_editing() {
        assert!(!EditMode::Viewing.is_formula_bar_editing());
        assert!(!EditMode::CellEditing {
            position: CellPosition::new(0, 0),
            content: String::new()
        }
        .is_formula_bar_editing());
        assert!(EditMode::FormulaBarEditing {
            position: CellPosition::new(0, 0),
            content: String::new()
        }
        .is_formula_bar_editing());
    }

    #[test]
    fn test_edit_mode_get_content() {
        assert_eq!(EditMode::Viewing.get_content(), None);
        assert_eq!(
            EditMode::CellEditing {
                position: CellPosition::new(0, 0),
                content: "test".to_string()
            }
            .get_content(),
            Some("test")
        );
        assert_eq!(
            EditMode::FormulaBarEditing {
                position: CellPosition::new(0, 0),
                content: "=SUM(A1:A10)".to_string()
            }
            .get_content(),
            Some("=SUM(A1:A10)")
        );
    }

    #[test]
    fn test_edit_mode_get_position() {
        assert_eq!(EditMode::Viewing.get_position(), None);
        let pos = CellPosition::new(5, 10);
        assert_eq!(
            EditMode::CellEditing {
                position: pos,
                content: String::new()
            }
            .get_position(),
            Some(pos)
        );
        assert_eq!(
            EditMode::FormulaBarEditing {
                position: pos,
                content: String::new()
            }
            .get_position(),
            Some(pos)
        );
    }

    // EditState tests
    #[test]
    fn test_edit_state_new() {
        let state = EditState::new();
        assert!(!state.is_editing());
        assert_eq!(state.mode(), &EditMode::Viewing);
        assert_eq!(state.current_content(), None);
        assert_eq!(state.editing_position(), None);
    }

    #[test]
    fn test_edit_state_default() {
        let state = EditState::default();
        assert_eq!(state, EditState::new());
    }

    #[test]
    fn test_start_cell_edit() {
        let mut state = EditState::new();
        let pos = CellPosition::new(3, 5);
        state.start_cell_edit(pos, "initial".to_string());

        assert!(state.is_editing());
        assert!(state.mode().is_cell_editing());
        assert_eq!(state.current_content(), Some("initial"));
        assert_eq!(state.editing_position(), Some(pos));
        assert_eq!(state.original_content(), Some("initial"));
    }

    #[test]
    fn test_start_formula_edit() {
        let mut state = EditState::new();
        let pos = CellPosition::new(3, 5);
        state.start_formula_edit(pos, "=A1+B1".to_string());

        assert!(state.is_editing());
        assert!(state.mode().is_formula_bar_editing());
        assert_eq!(state.current_content(), Some("=A1+B1"));
        assert_eq!(state.editing_position(), Some(pos));
        assert_eq!(state.original_content(), Some("=A1+B1"));
    }

    #[test]
    fn test_update_value_while_editing() {
        let mut state = EditState::new();
        state.start_cell_edit(CellPosition::new(0, 0), "old".to_string());

        let result = state.update_value("new".to_string());
        assert!(result.is_ok());
        assert_eq!(state.current_content(), Some("new"));
        assert_eq!(state.original_content(), Some("old"));
    }

    #[test]
    fn test_update_value_while_viewing() {
        let mut state = EditState::new();
        let result = state.update_value("new".to_string());
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Cannot update value while in viewing mode"
        );
    }

    #[test]
    fn test_commit_cell_edit() {
        let mut state = EditState::new();
        let pos = CellPosition::new(5, 10);
        state.start_cell_edit(pos, "initial".to_string());
        state.update_value("modified".to_string()).unwrap();

        let result = state.commit();
        assert_eq!(result, Some((pos, "modified".to_string())));
        assert!(!state.is_editing());
        assert_eq!(state.original_content(), None);
    }

    #[test]
    fn test_commit_formula_edit() {
        let mut state = EditState::new();
        let pos = CellPosition::new(5, 10);
        state.start_formula_edit(pos, "=A1".to_string());
        state.update_value("=A1+B1".to_string()).unwrap();

        let result = state.commit();
        assert_eq!(result, Some((pos, "=A1+B1".to_string())));
        assert!(!state.is_editing());
    }

    #[test]
    fn test_commit_while_viewing() {
        let mut state = EditState::new();
        let result = state.commit();
        assert_eq!(result, None);
    }

    #[test]
    fn test_cancel_edit() {
        let mut state = EditState::new();
        state.start_cell_edit(CellPosition::new(0, 0), "original".to_string());
        state.update_value("modified".to_string()).unwrap();

        let original = state.cancel();
        assert_eq!(original, Some("original".to_string()));
        assert!(!state.is_editing());
        assert_eq!(state.original_content(), None);
    }

    #[test]
    fn test_cancel_while_viewing() {
        let mut state = EditState::new();
        let result = state.cancel();
        assert_eq!(result, None);
    }

    #[test]
    fn test_switch_to_formula_bar() {
        let mut state = EditState::new();
        let pos = CellPosition::new(3, 5);
        state.start_cell_edit(pos, "content".to_string());

        state.switch_to_formula_bar();
        assert!(state.mode().is_formula_bar_editing());
        assert_eq!(state.current_content(), Some("content"));
        assert_eq!(state.editing_position(), Some(pos));
    }

    #[test]
    fn test_switch_to_formula_bar_no_op_when_viewing() {
        let mut state = EditState::new();
        state.switch_to_formula_bar();
        assert!(state.mode().is_viewing());
    }

    #[test]
    fn test_switch_to_formula_bar_no_op_when_already_in_formula_bar() {
        let mut state = EditState::new();
        state.start_formula_edit(CellPosition::new(0, 0), "test".to_string());
        state.switch_to_formula_bar();
        assert!(state.mode().is_formula_bar_editing());
    }

    #[test]
    fn test_switch_to_cell() {
        let mut state = EditState::new();
        let pos = CellPosition::new(3, 5);
        state.start_formula_edit(pos, "content".to_string());

        state.switch_to_cell();
        assert!(state.mode().is_cell_editing());
        assert_eq!(state.current_content(), Some("content"));
        assert_eq!(state.editing_position(), Some(pos));
    }

    #[test]
    fn test_switch_to_cell_no_op_when_viewing() {
        let mut state = EditState::new();
        state.switch_to_cell();
        assert!(state.mode().is_viewing());
    }

    #[test]
    fn test_switch_to_cell_no_op_when_already_in_cell() {
        let mut state = EditState::new();
        state.start_cell_edit(CellPosition::new(0, 0), "test".to_string());
        state.switch_to_cell();
        assert!(state.mode().is_cell_editing());
    }

    #[test]
    fn test_edit_state_preserves_original_during_updates() {
        let mut state = EditState::new();
        state.start_cell_edit(CellPosition::new(0, 0), "original".to_string());

        state.update_value("update1".to_string()).unwrap();
        assert_eq!(state.original_content(), Some("original"));

        state.update_value("update2".to_string()).unwrap();
        assert_eq!(state.original_content(), Some("original"));

        state.update_value("update3".to_string()).unwrap();
        assert_eq!(state.original_content(), Some("original"));
    }

    #[test]
    fn test_multiple_edit_sessions() {
        let mut state = EditState::new();

        // First edit session
        state.start_cell_edit(CellPosition::new(0, 0), "first".to_string());
        state.commit();

        // Second edit session
        state.start_cell_edit(CellPosition::new(1, 1), "second".to_string());
        assert_eq!(state.original_content(), Some("second"));
        state.commit();

        // Third edit session
        state.start_formula_edit(CellPosition::new(2, 2), "third".to_string());
        assert_eq!(state.original_content(), Some("third"));
    }

    #[test]
    fn test_serialization() {
        let mut state = EditState::new();
        state.start_cell_edit(CellPosition::new(5, 10), "test content".to_string());

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: EditState = serde_json::from_str(&serialized).unwrap();

        assert_eq!(state, deserialized);
    }
}
