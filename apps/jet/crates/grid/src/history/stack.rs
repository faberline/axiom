use crate::core::{CellCoord, Sheet};
use crate::history::command::CommandBox;

/// Manages undo/redo history for spreadsheet operations
#[derive(Default)]
pub struct HistoryManager {
    /// Stack of commands that can be undone
    undo_stack: Vec<CommandBox>,
    /// Stack of commands that can be redone
    redo_stack: Vec<CommandBox>,
    /// Maximum number of undo levels
    max_size: usize,
    /// Whether to try merging similar commands
    enable_merging: bool,
}

impl HistoryManager {
    /// Create a new history manager with the specified max undo levels
    pub fn new(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
            enable_merging: true,
        }
    }

    /// Execute a command and add it to the undo stack
    pub fn execute(&mut self, mut command: CommandBox, sheet: &mut Sheet) -> Vec<CellCoord> {
        let affected = command.execute(sheet);

        // Clear redo stack on new action
        self.redo_stack.clear();

        // Try to merge with previous command if enabled
        if self.enable_merging {
            if let Some(last) = self.undo_stack.last_mut() {
                if last.can_merge(&*command) && last.merge(&*command) {
                    return affected;
                }
            }
        }

        // Add to undo stack
        self.undo_stack.push(command);

        // Limit stack size
        while self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }

        affected
    }

    /// Undo the last command
    pub fn undo(&mut self, sheet: &mut Sheet) -> Option<Vec<CellCoord>> {
        let mut command = self.undo_stack.pop()?;
        let affected = command.undo(sheet);
        self.redo_stack.push(command);
        Some(affected)
    }

    /// Redo the last undone command
    pub fn redo(&mut self, sheet: &mut Sheet) -> Option<Vec<CellCoord>> {
        let mut command = self.redo_stack.pop()?;
        let affected = command.execute(sheet);
        self.undo_stack.push(command);
        Some(affected)
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get the description of the command that would be undone
    pub fn undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|c| c.description())
    }

    /// Get the description of the command that would be redone
    pub fn redo_description(&self) -> Option<&str> {
        self.redo_stack.last().map(|c| c.description())
    }

    /// Get the number of commands in the undo stack
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of commands in the redo stack
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Enable or disable command merging
    pub fn set_merging_enabled(&mut self, enabled: bool) {
        self.enable_merging = enabled;
    }

    /// Start a new undo group (prevents merging with previous commands)
    pub fn start_group(&mut self) {
        // Simply disable merging for the next command by setting a flag
        // This is a simplified implementation; a more sophisticated version
        // would track nested groups
        self.enable_merging = false;
    }

    /// End the current undo group
    pub fn end_group(&mut self) {
        self.enable_merging = true;
    }
}

impl std::fmt::Debug for HistoryManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryManager")
            .field("undo_count", &self.undo_stack.len())
            .field("redo_count", &self.redo_stack.len())
            .field("max_size", &self.max_size)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::CellValue;
    use crate::history::command::SetCellValueCommand;

    #[test]
    fn test_undo_redo() {
        let mut sheet = Sheet::new("Test");
        let mut history = HistoryManager::new(100);

        let coord = CellCoord::new(0, 0);

        // Execute command
        let cmd = Box::new(SetCellValueCommand::from_value(
            coord,
            CellValue::Number(42.0),
        ));
        history.execute(cmd, &mut sheet);

        assert_eq!(sheet.get_cell_value(coord).as_number(), Some(42.0));
        assert!(history.can_undo());
        assert!(!history.can_redo());

        // Undo
        history.undo(&mut sheet);
        assert!(sheet.get_cell_value(coord).is_empty());
        assert!(!history.can_undo());
        assert!(history.can_redo());

        // Redo
        history.redo(&mut sheet);
        assert_eq!(sheet.get_cell_value(coord).as_number(), Some(42.0));
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_redo_cleared_on_new_command() {
        let mut sheet = Sheet::new("Test");
        let mut history = HistoryManager::new(100);

        let coord = CellCoord::new(0, 0);

        // Execute and undo
        let cmd = Box::new(SetCellValueCommand::from_value(
            coord,
            CellValue::Number(42.0),
        ));
        history.execute(cmd, &mut sheet);
        history.undo(&mut sheet);

        assert!(history.can_redo());

        // Execute new command - should clear redo stack
        let cmd = Box::new(SetCellValueCommand::from_value(
            coord,
            CellValue::Number(100.0),
        ));
        history.execute(cmd, &mut sheet);

        assert!(!history.can_redo());
    }

    #[test]
    fn test_max_size() {
        let mut sheet = Sheet::new("Test");
        let mut history = HistoryManager::new(3);

        // Execute 5 commands
        for i in 0..5 {
            let coord = CellCoord::new(0, i);
            let cmd = Box::new(SetCellValueCommand::from_value(
                coord,
                CellValue::Number(i as f64),
            ));
            history.execute(cmd, &mut sheet);
        }

        // Should only have 3 commands (max_size)
        assert_eq!(history.undo_count(), 3);
    }

    #[test]
    fn test_multiple_undo_redo() {
        let mut sheet = Sheet::new("Test");
        let mut history = HistoryManager::new(100);

        // Set A1 = 1, A2 = 2, A3 = 3
        for i in 0..3 {
            let coord = CellCoord::new(i, 0);
            let cmd = Box::new(SetCellValueCommand::from_value(
                coord,
                CellValue::Number((i + 1) as f64),
            ));
            history.execute(cmd, &mut sheet);
        }

        // Undo all
        history.undo(&mut sheet);
        history.undo(&mut sheet);
        history.undo(&mut sheet);

        assert!(sheet.get_cell_value(CellCoord::new(0, 0)).is_empty());
        assert!(sheet.get_cell_value(CellCoord::new(1, 0)).is_empty());
        assert!(sheet.get_cell_value(CellCoord::new(2, 0)).is_empty());

        // Redo all
        history.redo(&mut sheet);
        history.redo(&mut sheet);
        history.redo(&mut sheet);

        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(1, 0)).as_number(),
            Some(2.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(2, 0)).as_number(),
            Some(3.0)
        );
    }
}
