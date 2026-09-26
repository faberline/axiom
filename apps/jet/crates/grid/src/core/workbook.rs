use serde::{Deserialize, Serialize};

use crate::core::error::RusheetError;
use crate::core::sheet::Sheet;

/// Metadata about the workbook
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkbookMetadata {
    /// ISO 8601 timestamp of creation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// ISO 8601 timestamp of last modification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    /// Author name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Application version that created/modified the workbook
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
}

/// A workbook containing multiple sheets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workbook {
    /// Workbook name (usually the file name)
    pub name: String,
    /// List of sheets in the workbook
    pub sheets: Vec<Sheet>,
    /// Index of the currently active sheet
    #[serde(default)]
    pub active_sheet_index: usize,
    /// Workbook metadata
    #[serde(default)]
    pub metadata: WorkbookMetadata,
}

impl Default for Workbook {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl Workbook {
    /// Create a new workbook with a default sheet
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sheets: vec![Sheet::new("Sheet1")],
            active_sheet_index: 0,
            metadata: WorkbookMetadata::default(),
        }
    }

    /// Get a reference to the active sheet
    pub fn active_sheet(&self) -> &Sheet {
        &self.sheets[self.active_sheet_index]
    }

    /// Get a mutable reference to the active sheet
    pub fn active_sheet_mut(&mut self) -> &mut Sheet {
        &mut self.sheets[self.active_sheet_index]
    }

    /// Get a sheet by index
    pub fn get_sheet(&self, index: usize) -> Option<&Sheet> {
        self.sheets.get(index)
    }

    /// Get a mutable sheet by index
    pub fn get_sheet_mut(&mut self, index: usize) -> Option<&mut Sheet> {
        self.sheets.get_mut(index)
    }

    /// Get a sheet by name
    pub fn get_sheet_by_name(&self, name: &str) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.name == name)
    }

    /// Get the index of a sheet by name
    pub fn get_sheet_index(&self, name: &str) -> Option<usize> {
        self.sheets.iter().position(|s| s.name == name)
    }

    /// Set the active sheet by index
    pub fn set_active_sheet(&mut self, index: usize) -> bool {
        if index < self.sheets.len() {
            self.active_sheet_index = index;
            true
        } else {
            false
        }
    }

    /// Set the active sheet by name
    pub fn set_active_sheet_by_name(&mut self, name: &str) -> bool {
        if let Some(index) = self.get_sheet_index(name) {
            self.active_sheet_index = index;
            true
        } else {
            false
        }
    }

    /// Add a new sheet with the given name
    pub fn add_sheet(&mut self, name: impl Into<String>) -> Result<usize, RusheetError> {
        let name = name.into();

        if name.trim().is_empty() {
            return Err(RusheetError::InvalidSheetName(
                "Name cannot be empty".to_string(),
            ));
        }

        if self.sheets.iter().any(|s| s.name == name) {
            return Err(RusheetError::SheetNameExists(name));
        }

        let index = self.sheets.len();
        self.sheets.push(Sheet::new(name));
        Ok(index)
    }

    /// Add a new sheet with an auto-generated name (Sheet2, Sheet3, etc.)
    pub fn add_sheet_auto(&mut self) -> usize {
        let mut num = self.sheets.len() + 1;
        loop {
            let name = format!("Sheet{}", num);
            if !self.sheets.iter().any(|s| s.name == name) {
                // We know this is safe because we just checked existence
                return self.add_sheet(name).unwrap();
            }
            num += 1;
        }
    }

    /// Remove a sheet by index
    pub fn remove_sheet(&mut self, index: usize) -> Result<Sheet, RusheetError> {
        if self.sheets.len() <= 1 {
            return Err(RusheetError::CannotDeleteLastSheet);
        }

        if index >= self.sheets.len() {
            return Err(RusheetError::SheetNotFound(index));
        }

        let sheet = self.sheets.remove(index);

        // Adjust active sheet index if needed
        if self.active_sheet_index >= self.sheets.len() {
            self.active_sheet_index = self.sheets.len() - 1;
        } else if self.active_sheet_index > index {
            self.active_sheet_index -= 1;
        }

        Ok(sheet)
    }

    /// Rename a sheet
    pub fn rename_sheet(
        &mut self,
        index: usize,
        new_name: impl Into<String>,
    ) -> Result<(), RusheetError> {
        let new_name = new_name.into();

        if new_name.trim().is_empty() {
            return Err(RusheetError::InvalidSheetName(
                "Name cannot be empty".to_string(),
            ));
        }

        // Check if name is already used by another sheet
        for (i, sheet) in self.sheets.iter().enumerate() {
            if i != index && sheet.name == new_name {
                return Err(RusheetError::SheetNameExists(new_name));
            }
        }

        if let Some(sheet) = self.sheets.get_mut(index) {
            sheet.name = new_name;
            Ok(())
        } else {
            Err(RusheetError::SheetNotFound(index))
        }
    }

    /// Move a sheet from one position to another
    pub fn move_sheet(&mut self, from: usize, to: usize) -> bool {
        if from >= self.sheets.len() || to >= self.sheets.len() {
            return false;
        }

        let sheet = self.sheets.remove(from);
        self.sheets.insert(to, sheet);

        // Adjust active sheet index
        if self.active_sheet_index == from {
            self.active_sheet_index = to;
        } else if from < self.active_sheet_index && to >= self.active_sheet_index {
            self.active_sheet_index -= 1;
        } else if from > self.active_sheet_index && to <= self.active_sheet_index {
            self.active_sheet_index += 1;
        }

        true
    }

    /// Duplicate a sheet
    pub fn duplicate_sheet(&mut self, index: usize) -> Option<usize> {
        let sheet = self.sheets.get(index)?.clone();

        // Generate a unique name
        let base_name = &sheet.name;
        let mut num = 2;
        let new_name = loop {
            let name = format!("{} ({})", base_name, num);
            if !self.sheets.iter().any(|s| s.name == name) {
                break name;
            }
            num += 1;
        };

        let mut new_sheet = sheet;
        new_sheet.name = new_name;

        let new_index = index + 1;
        self.sheets.insert(new_index, new_sheet);
        Some(new_index)
    }

    /// Get the number of sheets
    pub fn sheet_count(&self) -> usize {
        self.sheets.len()
    }

    /// Get all sheet names
    pub fn sheet_names(&self) -> Vec<&str> {
        self.sheets.iter().map(|s| s.name.as_str()).collect()
    }

    /// Serialize the workbook to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serialize the workbook to pretty JSON
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a workbook from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workbook_creation() {
        let wb = Workbook::new("Test");
        assert_eq!(wb.name, "Test");
        assert_eq!(wb.sheet_count(), 1);
        assert_eq!(wb.active_sheet().name, "Sheet1");
    }

    #[test]
    fn test_add_remove_sheets() {
        let mut wb = Workbook::new("Test");

        // Add sheets
        let idx = wb.add_sheet("Sheet2").unwrap();
        assert_eq!(idx, 1);
        assert_eq!(wb.sheet_count(), 2);

        let idx = wb.add_sheet_auto();
        assert_eq!(wb.sheets[idx].name, "Sheet3");

        // Remove sheet
        wb.remove_sheet(1).unwrap();
        assert_eq!(wb.sheet_count(), 2);

        // Cannot remove last sheet
        assert!(wb.remove_sheet(0).is_ok());
        assert!(wb.remove_sheet(0).is_err()); // Only 1 left now
        assert_eq!(wb.sheet_count(), 1);
    }

    #[test]
    fn test_sheet_navigation() {
        let mut wb = Workbook::new("Test");
        wb.add_sheet("Sheet2").unwrap();
        wb.add_sheet("Sheet3").unwrap();

        assert_eq!(wb.active_sheet_index, 0);

        wb.set_active_sheet(2);
        assert_eq!(wb.active_sheet().name, "Sheet3");

        wb.set_active_sheet_by_name("Sheet2");
        assert_eq!(wb.active_sheet().name, "Sheet2");
    }

    #[test]
    fn test_rename_sheet() {
        let mut wb = Workbook::new("Test");
        wb.add_sheet("Sheet2").unwrap();

        assert!(wb.rename_sheet(0, "Main").is_ok());
        assert_eq!(wb.sheets[0].name, "Main");

        // Cannot rename to existing name
        assert!(wb.rename_sheet(1, "Main").is_err());
    }

    #[test]
    fn test_serialization() {
        let mut wb = Workbook::new("Test");
        wb.add_sheet("Sheet2").unwrap();

        let json = wb.to_json().unwrap();
        let wb2 = Workbook::from_json(&json).unwrap();

        assert_eq!(wb2.name, "Test");
        assert_eq!(wb2.sheet_count(), 2);
    }
}
