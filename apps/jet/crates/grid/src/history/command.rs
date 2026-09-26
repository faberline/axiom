use crate::core::sheet::FilterState;
use crate::core::{Cell, CellContent, CellCoord, CellFormat, CellRange, CellValue, Sheet};
use crate::formula::{shift_formula_cols, shift_formula_rows};
use std::collections::HashSet;

/// Type alias for boxed commands
pub type CommandBox = Box<dyn Command>;

/// Trait for undoable commands
pub trait Command: std::fmt::Debug + Send + Sync {
    /// Execute the command, returning affected cell coordinates
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord>;

    /// Undo the command, returning affected cell coordinates
    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord>;

    /// Get a description of this command (for UI display)
    fn description(&self) -> &str;

    /// Try to merge with another command (for typing sequences)
    /// Returns true if merge was successful
    fn merge(&mut self, _other: &dyn Command) -> bool {
        false
    }

    /// Check if this command can be merged with another
    fn can_merge(&self, _other: &dyn Command) -> bool {
        false
    }
}

/// Set a single cell's value
#[derive(Debug)]
pub struct SetCellValueCommand {
    coord: CellCoord,
    new_content: CellContent,
    old_content: Option<CellContent>,
}

impl SetCellValueCommand {
    pub fn new(coord: CellCoord, new_content: CellContent) -> Self {
        Self {
            coord,
            new_content,
            old_content: None,
        }
    }

    pub fn from_value(coord: CellCoord, value: CellValue) -> Self {
        Self::new(
            coord,
            CellContent::Value {
                value,
                original_input: None,
            },
        )
    }

    pub fn from_input(coord: CellCoord, input: &str) -> Self {
        let content = crate::core::sheet::parse_cell_input(input);
        Self::new(coord, content)
    }
}

impl Command for SetCellValueCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        // Capture old state for undo
        self.old_content = sheet.get_cell(self.coord).map(|c| c.content.clone());

        // Apply new value
        let cell = sheet.get_cell_mut(self.coord);
        cell.content = self.new_content.clone();

        vec![self.coord]
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let cell = sheet.get_cell_mut(self.coord);
        cell.content = self.old_content.clone().unwrap_or(CellContent::Value {
            value: CellValue::Empty,
            original_input: None,
        });

        vec![self.coord]
    }

    fn description(&self) -> &str {
        "Set cell value"
    }
}

/// Set cell format
#[derive(Debug)]
pub struct SetCellFormatCommand {
    coord: CellCoord,
    new_format: CellFormat,
    old_format: Option<CellFormat>,
}

impl SetCellFormatCommand {
    pub fn new(coord: CellCoord, new_format: CellFormat) -> Self {
        Self {
            coord,
            new_format,
            old_format: None,
        }
    }
}

impl Command for SetCellFormatCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        // Capture old format first (immutable borrow)
        self.old_format = sheet.get_cell(self.coord).map(|c| c.format.clone());

        // Then apply new format (mutable borrow)
        let cell = sheet.get_cell_mut(self.coord);
        cell.format = self.new_format.clone();

        vec![self.coord]
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let cell = sheet.get_cell_mut(self.coord);
        cell.format = self.old_format.clone().unwrap_or_default();

        vec![self.coord]
    }

    fn description(&self) -> &str {
        "Set cell format"
    }
}

/// Set formatting for a range of cells
#[derive(Debug)]
pub struct SetRangeFormatCommand {
    start: CellCoord,
    end: CellCoord,
    new_format: CellFormat,
    old_formats: Vec<(CellCoord, CellFormat)>,
}

impl SetRangeFormatCommand {
    pub fn new(start: CellCoord, end: CellCoord, new_format: CellFormat) -> Self {
        Self {
            start,
            end,
            new_format,
            old_formats: Vec::new(),
        }
    }
}

impl Command for SetRangeFormatCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();
        self.old_formats.clear();

        let min_row = self.start.row.min(self.end.row);
        let max_row = self.start.row.max(self.end.row);
        let min_col = self.start.col.min(self.end.col);
        let max_col = self.start.col.max(self.end.col);

        for row in min_row..=max_row {
            for col in min_col..=max_col {
                let coord = CellCoord::new(row, col);

                // Capture old format (immutable borrow)
                let old_format = sheet
                    .get_cell(coord)
                    .map(|c| c.format.clone())
                    .unwrap_or_default();
                self.old_formats.push((coord, old_format));

                // Apply new format (mutable borrow)
                let cell = sheet.get_cell_mut(coord);
                cell.format = self.new_format.clone();

                affected.push(coord);
            }
        }

        affected
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();

        for (coord, format) in &self.old_formats {
            let cell = sheet.get_cell_mut(*coord);
            cell.format = format.clone();
            affected.push(*coord);
        }

        affected
    }

    fn description(&self) -> &str {
        "Set range format"
    }
}

/// Clear cell content (but keep format)
#[derive(Debug)]
pub struct ClearCellCommand {
    coord: CellCoord,
    old_content: Option<CellContent>,
}

impl ClearCellCommand {
    pub fn new(coord: CellCoord) -> Self {
        Self {
            coord,
            old_content: None,
        }
    }
}

impl Command for ClearCellCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        self.old_content = sheet.get_cell(self.coord).map(|c| c.content.clone());

        let cell = sheet.get_cell_mut(self.coord);
        cell.content = CellContent::Value {
            value: CellValue::Empty,
            original_input: None,
        };

        vec![self.coord]
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        if let Some(content) = &self.old_content {
            let cell = sheet.get_cell_mut(self.coord);
            cell.content = content.clone();
        }

        vec![self.coord]
    }

    fn description(&self) -> &str {
        "Clear cell"
    }
}

/// Clear a range of cells
#[derive(Debug)]
pub struct ClearRangeCommand {
    start: CellCoord,
    end: CellCoord,
    old_cells: Vec<(CellCoord, Cell)>,
}

impl ClearRangeCommand {
    pub fn new(start: CellCoord, end: CellCoord) -> Self {
        Self {
            start,
            end,
            old_cells: Vec::new(),
        }
    }
}

impl Command for ClearRangeCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();
        self.old_cells.clear();

        let min_row = self.start.row.min(self.end.row);
        let max_row = self.start.row.max(self.end.row);
        let min_col = self.start.col.min(self.end.col);
        let max_col = self.start.col.max(self.end.col);

        for row in min_row..=max_row {
            for col in min_col..=max_col {
                let coord = CellCoord::new(row, col);

                // Capture old cell
                if let Some(cell) = sheet.get_cell(coord) {
                    self.old_cells.push((coord, cell.clone()));
                }

                // Clear the cell
                sheet.remove_cell(coord);
                affected.push(coord);
            }
        }

        affected
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();

        for (coord, cell) in &self.old_cells {
            sheet.set_cell(*coord, cell.clone());
            affected.push(*coord);
        }

        affected
    }

    fn description(&self) -> &str {
        "Clear range"
    }
}

/// Composite command for batch operations
#[derive(Debug)]
pub struct CompositeCommand {
    commands: Vec<CommandBox>,
    description: String,
}

impl CompositeCommand {
    pub fn new(commands: Vec<CommandBox>, description: impl Into<String>) -> Self {
        Self {
            commands,
            description: description.into(),
        }
    }
}

impl Command for CompositeCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();
        for cmd in &mut self.commands {
            affected.extend(cmd.execute(sheet));
        }
        affected
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();
        // Undo in reverse order
        for cmd in self.commands.iter_mut().rev() {
            affected.extend(cmd.undo(sheet));
        }
        affected
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// Insert rows at the given position
#[derive(Debug)]
pub struct InsertRowsCommand {
    at_row: u32,
    count: u32,
    // For undo: track which cells were shifted and their old formulas
    shifted_cells: Vec<(CellCoord, CellCoord)>, // (old_coord, new_coord)
    formula_updates: Vec<(CellCoord, String, String)>, // (coord, old_formula, new_formula)
}

impl InsertRowsCommand {
    pub fn new(at_row: u32, count: u32) -> Self {
        Self {
            at_row,
            count,
            shifted_cells: Vec::new(),
            formula_updates: Vec::new(),
        }
    }
}

impl Command for InsertRowsCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        if self.count == 0 {
            return Vec::new();
        }

        let mut affected = Vec::new();

        // Step 1: Collect all formulas that need to be updated
        self.formula_updates.clear();
        for coord in sheet.non_empty_coords().collect::<Vec<_>>() {
            if let Some(cell) = sheet.get_cell(coord) {
                if let CellContent::Formula { expression, .. } = &cell.content {
                    // Try to shift the formula
                    if let Some(new_formula) =
                        shift_formula_rows(&expression, self.at_row, self.count as i32)
                    {
                        if new_formula != *expression {
                            self.formula_updates
                                .push((coord, expression.clone(), new_formula));
                        }
                    }
                }
            }
        }

        // Step 2: Insert rows and shift cells
        self.shifted_cells = sheet.insert_rows(self.at_row, self.count);

        // Step 3: Update formulas (cells may have been shifted)
        for (orig_coord, _old_formula, new_formula) in &self.formula_updates {
            // If the cell was shifted, find its new coordinate
            let current_coord = if orig_coord.row >= self.at_row {
                CellCoord::new(orig_coord.row + self.count, orig_coord.col)
            } else {
                *orig_coord
            };

            if let Some(cell) = sheet.get_cell(current_coord) {
                if let CellContent::Formula { cached_value, .. } = &cell.content {
                    let cached = cached_value.clone();
                    let cell_mut = sheet.get_cell_mut(current_coord);
                    cell_mut.content = CellContent::Formula {
                        expression: new_formula.clone(),
                        cached_value: cached,
                    };
                    affected.push(current_coord);
                }
            }
        }

        // Collect all affected cells from shifted cells
        for (old_coord, new_coord) in &self.shifted_cells {
            affected.push(*old_coord);
            affected.push(*new_coord);
        }

        affected
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();

        // Step 1: Restore old formulas
        for (coord, old_formula, _new_formula) in &self.formula_updates {
            if let Some(cell) = sheet.get_cell(*coord) {
                if let CellContent::Formula { cached_value, .. } = &cell.content {
                    let cached = cached_value.clone();
                    let cell_mut = sheet.get_cell_mut(*coord);
                    cell_mut.content = CellContent::Formula {
                        expression: old_formula.clone(),
                        cached_value: cached,
                    };
                    affected.push(*coord);
                }
            }
        }

        // Step 2: Delete the inserted rows to shift cells back
        sheet.delete_rows(self.at_row, self.count);

        // Collect all affected cells
        for (old_coord, new_coord) in &self.shifted_cells {
            affected.push(*old_coord);
            affected.push(*new_coord);
        }

        affected
    }

    fn description(&self) -> &str {
        "Insert rows"
    }
}

/// Delete rows at the given position
#[derive(Debug)]
pub struct DeleteRowsCommand {
    at_row: u32,
    count: u32,
    // For undo: store deleted cells and their coordinates
    deleted_cells: Vec<(CellCoord, Cell)>,
    // Track formula updates
    formula_updates: Vec<(CellCoord, String, String)>, // (coord, old_formula, new_formula)
}

impl DeleteRowsCommand {
    pub fn new(at_row: u32, count: u32) -> Self {
        Self {
            at_row,
            count,
            deleted_cells: Vec::new(),
            formula_updates: Vec::new(),
        }
    }
}

impl Command for DeleteRowsCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        if self.count == 0 {
            return Vec::new();
        }

        let mut affected = Vec::new();

        // Step 1: Collect all formulas that need to be updated (or become invalid)
        self.formula_updates.clear();
        for coord in sheet.non_empty_coords().collect::<Vec<_>>() {
            if let Some(cell) = sheet.get_cell(coord) {
                if let CellContent::Formula { expression, .. } = &cell.content {
                    // Try to shift the formula (negative delta for deletion)
                    if let Some(new_formula) =
                        shift_formula_rows(&expression, self.at_row, -(self.count as i32))
                    {
                        if new_formula != *expression {
                            self.formula_updates
                                .push((coord, expression.clone(), new_formula));
                        }
                    } else {
                        // Formula becomes invalid - convert to error
                        let coord_in_deleted_range =
                            coord.row >= self.at_row && coord.row < self.at_row + self.count;
                        if !coord_in_deleted_range {
                            self.formula_updates.push((
                                coord,
                                expression.clone(),
                                "=#REF!".to_string(),
                            ));
                        }
                    }
                }
            }
        }

        // Step 2: Delete rows and capture deleted cells
        self.deleted_cells = sheet.delete_rows(self.at_row, self.count);

        // Step 3: Update formulas
        for (coord, _old_formula, new_formula) in &self.formula_updates {
            // Coord may have shifted, need to find the shifted coordinate
            let shifted_coord = if coord.row >= self.at_row + self.count {
                CellCoord::new(coord.row - self.count, coord.col)
            } else {
                *coord
            };

            if let Some(cell) = sheet.get_cell(shifted_coord) {
                if let CellContent::Formula { cached_value, .. } = &cell.content {
                    let cached = cached_value.clone();
                    let cell_mut = sheet.get_cell_mut(shifted_coord);
                    cell_mut.content = CellContent::Formula {
                        expression: new_formula.clone(),
                        cached_value: cached,
                    };
                    affected.push(shifted_coord);
                }
            }
        }

        // Collect all affected cells
        for (coord, _) in &self.deleted_cells {
            affected.push(*coord);
        }

        affected
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();

        // Step 1: Restore old formulas (before re-inserting rows)
        for (coord, old_formula, _new_formula) in &self.formula_updates {
            // Coord is stored as the original coordinate before deletion
            let current_coord = if coord.row >= self.at_row + self.count {
                CellCoord::new(coord.row - self.count, coord.col)
            } else if coord.row >= self.at_row {
                // This was a deleted cell, skip it (will be restored below)
                continue;
            } else {
                *coord
            };

            if let Some(cell) = sheet.get_cell(current_coord) {
                if let CellContent::Formula { cached_value, .. } = &cell.content {
                    let cached = cached_value.clone();
                    let cell_mut = sheet.get_cell_mut(current_coord);
                    cell_mut.content = CellContent::Formula {
                        expression: old_formula.clone(),
                        cached_value: cached,
                    };
                }
            }
        }

        // Step 2: Insert rows back to make space
        sheet.insert_rows(self.at_row, self.count);

        // Step 3: Restore deleted cells
        for (coord, cell) in &self.deleted_cells {
            sheet.set_cell(*coord, cell.clone());
            affected.push(*coord);
        }

        // Step 4: Restore formulas that were in cells above deleted range
        for (coord, old_formula, _new_formula) in &self.formula_updates {
            if coord.row >= self.at_row + self.count {
                // This cell has been shifted back to its original position
                if let Some(cell) = sheet.get_cell(*coord) {
                    if let CellContent::Formula { cached_value, .. } = &cell.content {
                        let cached = cached_value.clone();
                        let cell_mut = sheet.get_cell_mut(*coord);
                        cell_mut.content = CellContent::Formula {
                            expression: old_formula.clone(),
                            cached_value: cached,
                        };
                        affected.push(*coord);
                    }
                }
            }
        }

        affected
    }

    fn description(&self) -> &str {
        "Delete rows"
    }
}

/// Insert columns at the given position
#[derive(Debug)]
pub struct InsertColsCommand {
    at_col: u32,
    count: u32,
    // For undo: track which cells were shifted and their old formulas
    shifted_cells: Vec<(CellCoord, CellCoord)>, // (old_coord, new_coord)
    formula_updates: Vec<(CellCoord, String, String)>, // (coord, old_formula, new_formula)
}

impl InsertColsCommand {
    pub fn new(at_col: u32, count: u32) -> Self {
        Self {
            at_col,
            count,
            shifted_cells: Vec::new(),
            formula_updates: Vec::new(),
        }
    }
}

impl Command for InsertColsCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        if self.count == 0 {
            return Vec::new();
        }

        let mut affected = Vec::new();

        // Step 1: Collect all formulas that need to be updated
        self.formula_updates.clear();
        for coord in sheet.non_empty_coords().collect::<Vec<_>>() {
            if let Some(cell) = sheet.get_cell(coord) {
                if let CellContent::Formula { expression, .. } = &cell.content {
                    // Try to shift the formula
                    if let Some(new_formula) =
                        shift_formula_cols(&expression, self.at_col, self.count as i32)
                    {
                        if new_formula != *expression {
                            self.formula_updates
                                .push((coord, expression.clone(), new_formula));
                        }
                    }
                }
            }
        }

        // Step 2: Insert columns and shift cells
        self.shifted_cells = sheet.insert_cols(self.at_col, self.count);

        // Step 3: Update formulas (cells may have been shifted)
        for (orig_coord, _old_formula, new_formula) in &self.formula_updates {
            // If the cell was shifted, find its new coordinate
            let current_coord = if orig_coord.col >= self.at_col {
                CellCoord::new(orig_coord.row, orig_coord.col + self.count)
            } else {
                *orig_coord
            };

            if let Some(cell) = sheet.get_cell(current_coord) {
                if let CellContent::Formula { cached_value, .. } = &cell.content {
                    let cached = cached_value.clone();
                    let cell_mut = sheet.get_cell_mut(current_coord);
                    cell_mut.content = CellContent::Formula {
                        expression: new_formula.clone(),
                        cached_value: cached,
                    };
                    affected.push(current_coord);
                }
            }
        }

        // Collect all affected cells from shifted cells
        for (old_coord, new_coord) in &self.shifted_cells {
            affected.push(*old_coord);
            affected.push(*new_coord);
        }

        affected
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();

        // Step 1: Restore old formulas
        for (coord, old_formula, _new_formula) in &self.formula_updates {
            if let Some(cell) = sheet.get_cell(*coord) {
                if let CellContent::Formula { cached_value, .. } = &cell.content {
                    let cached = cached_value.clone();
                    let cell_mut = sheet.get_cell_mut(*coord);
                    cell_mut.content = CellContent::Formula {
                        expression: old_formula.clone(),
                        cached_value: cached,
                    };
                    affected.push(*coord);
                }
            }
        }

        // Step 2: Delete the inserted columns to shift cells back
        sheet.delete_cols(self.at_col, self.count);

        // Collect all affected cells
        for (old_coord, new_coord) in &self.shifted_cells {
            affected.push(*old_coord);
            affected.push(*new_coord);
        }

        affected
    }

    fn description(&self) -> &str {
        "Insert columns"
    }
}

/// Delete columns at the given position
#[derive(Debug)]
pub struct DeleteColsCommand {
    at_col: u32,
    count: u32,
    // For undo: store deleted cells and their coordinates
    deleted_cells: Vec<(CellCoord, Cell)>,
    // Track formula updates
    formula_updates: Vec<(CellCoord, String, String)>, // (coord, old_formula, new_formula)
}

impl DeleteColsCommand {
    pub fn new(at_col: u32, count: u32) -> Self {
        Self {
            at_col,
            count,
            deleted_cells: Vec::new(),
            formula_updates: Vec::new(),
        }
    }
}

impl Command for DeleteColsCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        if self.count == 0 {
            return Vec::new();
        }

        let mut affected = Vec::new();

        // Step 1: Collect all formulas that need to be updated (or become invalid)
        self.formula_updates.clear();
        for coord in sheet.non_empty_coords().collect::<Vec<_>>() {
            if let Some(cell) = sheet.get_cell(coord) {
                if let CellContent::Formula { expression, .. } = &cell.content {
                    // Try to shift the formula (negative delta for deletion)
                    if let Some(new_formula) =
                        shift_formula_cols(&expression, self.at_col, -(self.count as i32))
                    {
                        if new_formula != *expression {
                            self.formula_updates
                                .push((coord, expression.clone(), new_formula));
                        }
                    } else {
                        // Formula becomes invalid - convert to error
                        let coord_in_deleted_range =
                            coord.col >= self.at_col && coord.col < self.at_col + self.count;
                        if !coord_in_deleted_range {
                            self.formula_updates.push((
                                coord,
                                expression.clone(),
                                "=#REF!".to_string(),
                            ));
                        }
                    }
                }
            }
        }

        // Step 2: Delete columns and capture deleted cells
        self.deleted_cells = sheet.delete_cols(self.at_col, self.count);

        // Step 3: Update formulas
        for (coord, _old_formula, new_formula) in &self.formula_updates {
            // Coord may have shifted, need to find the shifted coordinate
            let shifted_coord = if coord.col >= self.at_col + self.count {
                CellCoord::new(coord.row, coord.col - self.count)
            } else {
                *coord
            };

            if let Some(cell) = sheet.get_cell(shifted_coord) {
                if let CellContent::Formula { cached_value, .. } = &cell.content {
                    let cached = cached_value.clone();
                    let cell_mut = sheet.get_cell_mut(shifted_coord);
                    cell_mut.content = CellContent::Formula {
                        expression: new_formula.clone(),
                        cached_value: cached,
                    };
                    affected.push(shifted_coord);
                }
            }
        }

        // Collect all affected cells
        for (coord, _) in &self.deleted_cells {
            affected.push(*coord);
        }

        affected
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        let mut affected = Vec::new();

        // Step 1: Restore old formulas (before re-inserting columns)
        for (coord, old_formula, _new_formula) in &self.formula_updates {
            // Coord is stored as the original coordinate before deletion
            let current_coord = if coord.col >= self.at_col + self.count {
                CellCoord::new(coord.row, coord.col - self.count)
            } else if coord.col >= self.at_col {
                // This was a deleted cell, skip it (will be restored below)
                continue;
            } else {
                *coord
            };

            if let Some(cell) = sheet.get_cell(current_coord) {
                if let CellContent::Formula { cached_value, .. } = &cell.content {
                    let cached = cached_value.clone();
                    let cell_mut = sheet.get_cell_mut(current_coord);
                    cell_mut.content = CellContent::Formula {
                        expression: old_formula.clone(),
                        cached_value: cached,
                    };
                }
            }
        }

        // Step 2: Insert columns back to make space
        sheet.insert_cols(self.at_col, self.count);

        // Step 3: Restore deleted cells
        for (coord, cell) in &self.deleted_cells {
            sheet.set_cell(*coord, cell.clone());
            affected.push(*coord);
        }

        // Step 4: Restore formulas that were in cells to the right of deleted range
        for (coord, old_formula, _new_formula) in &self.formula_updates {
            if coord.col >= self.at_col + self.count {
                // This cell has been shifted back to its original position
                if let Some(cell) = sheet.get_cell(*coord) {
                    if let CellContent::Formula { cached_value, .. } = &cell.content {
                        let cached = cached_value.clone();
                        let cell_mut = sheet.get_cell_mut(*coord);
                        cell_mut.content = CellContent::Formula {
                            expression: old_formula.clone(),
                            cached_value: cached,
                        };
                        affected.push(*coord);
                    }
                }
            }
        }

        affected
    }

    fn description(&self) -> &str {
        "Delete columns"
    }
}

/// Sort a range of rows by a specific column
#[derive(Debug)]
pub struct SortRangeCommand {
    start_row: u32,
    end_row: u32,
    start_col: u32,
    end_col: u32,
    sort_col: u32,
    ascending: bool,
    // For undo: stores the original row order
    row_mapping: Vec<(u32, u32)>,
}

impl SortRangeCommand {
    pub fn new(
        start_row: u32,
        end_row: u32,
        start_col: u32,
        end_col: u32,
        sort_col: u32,
        ascending: bool,
    ) -> Self {
        Self {
            start_row,
            end_row,
            start_col,
            end_col,
            sort_col,
            ascending,
            row_mapping: Vec::new(),
        }
    }
}

impl Command for SortRangeCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        // Perform the sort and store the mapping for undo
        self.row_mapping = sheet.sort_range(
            self.start_row,
            self.end_row,
            self.start_col,
            self.end_col,
            self.sort_col,
            self.ascending,
        );

        // Return all cells in the sorted range as affected
        let mut affected = Vec::new();
        for row in self.start_row..=self.end_row {
            for col in self.start_col..=self.end_col {
                affected.push(CellCoord::new(row, col));
            }
        }
        affected
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        // Restore original row order using the stored mapping
        sheet.unsort_range(
            self.start_row,
            self.end_row,
            self.start_col,
            self.end_col,
            &self.row_mapping,
        );

        // Return all cells in the range as affected
        let mut affected = Vec::new();
        for row in self.start_row..=self.end_row {
            for col in self.start_col..=self.end_col {
                affected.push(CellCoord::new(row, col));
            }
        }
        affected
    }

    fn description(&self) -> &str {
        if self.ascending {
            "Sort ascending"
        } else {
            "Sort descending"
        }
    }
}

/// Merge cells in a range
#[derive(Debug)]
pub struct MergeCellsCommand {
    range: CellRange,
    // For undo: store the original cell contents that were cleared
    cleared_cells: Vec<(CellCoord, CellContent)>,
    // Store whether merge was successful
    success: bool,
}

impl MergeCellsCommand {
    pub fn new(range: CellRange) -> Self {
        Self {
            range,
            cleared_cells: Vec::new(),
            success: false,
        }
    }

    /// Create from start/end coordinates
    pub fn from_coords(start_row: u32, start_col: u32, end_row: u32, end_col: u32) -> Self {
        Self::new(CellRange::new(
            CellCoord::new(start_row, start_col),
            CellCoord::new(end_row, end_col),
        ))
    }
}

impl Command for MergeCellsCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        self.cleared_cells.clear();

        // Don't merge single cells
        if self.range.is_single_cell() {
            self.success = false;
            return Vec::new();
        }

        // Check for overlap with existing merges
        if sheet.would_overlap_merge(&self.range) {
            self.success = false;
            return Vec::new();
        }

        let master = self.range.start;

        // Collect cells to clear (non-master cells with content)
        for coord in self.range.iter() {
            if coord != master {
                if let Some(cell) = sheet.get_cell(coord) {
                    if !cell.content.is_empty() {
                        self.cleared_cells.push((coord, cell.content.clone()));
                    }
                }
            }
        }

        // Perform the merge
        self.success = sheet.merge_cells(self.range);

        if self.success {
            // Return all cells in the merged range as affected
            self.range.iter().collect()
        } else {
            Vec::new()
        }
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        if !self.success {
            return Vec::new();
        }

        // Unmerge first
        sheet.unmerge_cells(self.range);

        // Restore cleared cells
        for (coord, content) in &self.cleared_cells {
            let cell = sheet.get_cell_mut(*coord);
            cell.content = content.clone();
        }

        self.range.iter().collect()
    }

    fn description(&self) -> &str {
        "Merge cells"
    }
}

/// Unmerge cells
#[derive(Debug)]
pub struct UnmergeCellsCommand {
    range: CellRange,
    // For undo: store the original merged range
    original_merge: Option<CellRange>,
    success: bool,
}

impl UnmergeCellsCommand {
    pub fn new(range: CellRange) -> Self {
        Self {
            range,
            original_merge: None,
            success: false,
        }
    }

    /// Create from a coordinate (will unmerge the merge containing this coord)
    pub fn from_coord(coord: CellCoord) -> Self {
        Self::new(CellRange::new(coord, coord))
    }
}

impl Command for UnmergeCellsCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        // Find the merge containing the range start
        if let Some(merge) = sheet.get_merge_at(self.range.start) {
            self.original_merge = Some(*merge);
            let affected_range = *merge;

            // Unmerge
            self.success = sheet.unmerge_cells(self.range);

            if self.success {
                affected_range.iter().collect()
            } else {
                Vec::new()
            }
        } else {
            self.success = false;
            Vec::new()
        }
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        if !self.success {
            return Vec::new();
        }

        // Re-merge the original range
        if let Some(merge) = self.original_merge {
            sheet.merge_cells(merge);
            merge.iter().collect()
        } else {
            Vec::new()
        }
    }

    fn description(&self) -> &str {
        "Unmerge cells"
    }
}

/// Apply a column filter
#[derive(Debug)]
pub struct ApplyFilterCommand {
    col: u32,
    visible_values: HashSet<String>,
    max_rows: u32,
    previously_hidden_rows: Vec<u32>, // For undo - rows that were hidden before this filter
    newly_hidden_rows: Vec<u32>,      // For undo - rows hidden by this filter
}

impl ApplyFilterCommand {
    pub fn new(col: u32, visible_values: HashSet<String>, max_rows: u32) -> Self {
        Self {
            col,
            visible_values,
            max_rows,
            previously_hidden_rows: Vec::new(),
            newly_hidden_rows: Vec::new(),
        }
    }
}

impl Command for ApplyFilterCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        // Store previously hidden rows for undo
        self.previously_hidden_rows = sheet.get_hidden_rows();

        // Apply the filter
        self.newly_hidden_rows =
            sheet.apply_column_filter(self.col, &self.visible_values, self.max_rows);

        // Return affected cells (all cells in the newly hidden rows)
        self.newly_hidden_rows
            .iter()
            .map(|&row| CellCoord::new(row, self.col))
            .collect()
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        // Show all rows that were hidden by this filter
        sheet.show_rows(&self.newly_hidden_rows);

        // Remove this filter from active filters
        sheet.active_filters.retain(|f| f.col != self.col);

        // Return affected cells
        self.newly_hidden_rows
            .iter()
            .map(|&row| CellCoord::new(row, self.col))
            .collect()
    }

    fn description(&self) -> &str {
        "Apply Column Filter"
    }
}

/// Clear a column filter or all filters
#[derive(Debug)]
pub struct ClearFilterCommand {
    col: Option<u32>, // None means clear all filters
    saved_filters: Vec<FilterState>,
    hidden_rows_before: Vec<u32>,
}

impl ClearFilterCommand {
    pub fn new(col: Option<u32>) -> Self {
        Self {
            col,
            saved_filters: Vec::new(),
            hidden_rows_before: Vec::new(),
        }
    }
}

impl Command for ClearFilterCommand {
    fn execute(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        // Save current state for undo
        self.saved_filters = sheet.get_active_filters().to_vec();
        self.hidden_rows_before = sheet.get_hidden_rows();

        // Clear filter(s)
        let unhidden_rows = match self.col {
            Some(col) => sheet.clear_column_filter(col),
            None => sheet.clear_all_filters(),
        };

        // Return affected cells
        unhidden_rows
            .iter()
            .map(|&row| CellCoord::new(row, 0))
            .collect()
    }

    fn undo(&mut self, sheet: &mut Sheet) -> Vec<CellCoord> {
        // Restore filters - re-hide rows that were hidden before
        sheet.hide_rows(&self.hidden_rows_before);

        // Restore filter state
        sheet.active_filters = self.saved_filters.clone();

        // Return affected cells
        self.hidden_rows_before
            .iter()
            .map(|&row| CellCoord::new(row, 0))
            .collect()
    }

    fn description(&self) -> &str {
        match self.col {
            Some(_) => "Clear Column Filter",
            None => "Clear All Filters",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_cell_value_command() {
        let mut sheet = Sheet::new("Test");
        let coord = CellCoord::new(0, 0);

        let mut cmd = SetCellValueCommand::from_value(coord, CellValue::Number(42.0));

        // Execute
        let affected = cmd.execute(&mut sheet);
        assert_eq!(affected, vec![coord]);
        assert_eq!(sheet.get_cell_value(coord).as_number(), Some(42.0));

        // Undo
        let affected = cmd.undo(&mut sheet);
        assert_eq!(affected, vec![coord]);
        assert!(sheet.get_cell_value(coord).is_empty());
    }

    #[test]
    fn test_clear_range_command() {
        let mut sheet = Sheet::new("Test");

        // Set up some cells
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(0, 1), Cell::number(2.0));
        sheet.set_cell(CellCoord::new(1, 0), Cell::number(3.0));

        let mut cmd = ClearRangeCommand::new(CellCoord::new(0, 0), CellCoord::new(1, 1));

        // Execute
        cmd.execute(&mut sheet);
        assert!(sheet.get_cell(CellCoord::new(0, 0)).is_none());
        assert!(sheet.get_cell(CellCoord::new(0, 1)).is_none());
        assert!(sheet.get_cell(CellCoord::new(1, 0)).is_none());

        // Undo
        cmd.undo(&mut sheet);
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 1)).as_number(),
            Some(2.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(1, 0)).as_number(),
            Some(3.0)
        );
    }

    #[test]
    fn test_insert_rows_command() {
        let mut sheet = Sheet::new("Test");

        // Set up some cells
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(1, 0), Cell::number(2.0));
        sheet.set_cell(CellCoord::new(2, 0), Cell::number(3.0));

        let mut cmd = InsertRowsCommand::new(1, 2);

        // Execute
        cmd.execute(&mut sheet);

        // Row 0 should stay in place
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );

        // Rows 1 and 2 should be empty (newly inserted)
        assert!(sheet.get_cell(CellCoord::new(1, 0)).is_none());
        assert!(sheet.get_cell(CellCoord::new(2, 0)).is_none());

        // Old row 1 should shift to row 3
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(3, 0)).as_number(),
            Some(2.0)
        );

        // Old row 2 should shift to row 4
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(4, 0)).as_number(),
            Some(3.0)
        );

        // Undo
        cmd.undo(&mut sheet);

        // Should restore original state
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
        assert!(sheet.get_cell(CellCoord::new(3, 0)).is_none());
        assert!(sheet.get_cell(CellCoord::new(4, 0)).is_none());
    }

    #[test]
    fn test_insert_rows_updates_formulas() {
        let mut sheet = Sheet::new("Test");

        // Set up a formula that references rows
        let cell = Cell {
            content: CellContent::Formula {
                expression: "=A3+A4".to_string(),
                cached_value: CellValue::Empty,
            },
            format: Default::default(),
        };
        sheet.set_cell(CellCoord::new(0, 0), cell);

        let mut cmd = InsertRowsCommand::new(2, 2);

        // Execute
        cmd.execute(&mut sheet);

        // Formula should be updated: A3 (row 2) becomes A5, A4 (row 3) becomes A6
        // Both references are >= row 2, so both shift by 2
        if let Some(cell) = sheet.get_cell(CellCoord::new(0, 0)) {
            if let CellContent::Formula { expression, .. } = &cell.content {
                assert_eq!(expression, "=A5+A6");
            } else {
                panic!("Expected formula");
            }
        } else {
            panic!("Cell not found");
        }

        // Undo
        cmd.undo(&mut sheet);

        // Formula should be restored
        if let Some(cell) = sheet.get_cell(CellCoord::new(0, 0)) {
            if let CellContent::Formula { expression, .. } = &cell.content {
                assert_eq!(expression, "=A3+A4");
            } else {
                panic!("Expected formula");
            }
        } else {
            panic!("Cell not found");
        }
    }

    #[test]
    fn test_delete_rows_command() {
        let mut sheet = Sheet::new("Test");

        // Set up some cells
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(1, 0), Cell::number(2.0));
        sheet.set_cell(CellCoord::new(2, 0), Cell::number(3.0));
        sheet.set_cell(CellCoord::new(3, 0), Cell::number(4.0));
        sheet.set_cell(CellCoord::new(4, 0), Cell::number(5.0));

        let mut cmd = DeleteRowsCommand::new(1, 2);

        // Execute
        cmd.execute(&mut sheet);

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

        // Undo
        cmd.undo(&mut sheet);

        // Should restore original state
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
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(3, 0)).as_number(),
            Some(4.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(4, 0)).as_number(),
            Some(5.0)
        );
    }

    #[test]
    fn test_delete_rows_updates_formulas() {
        let mut sheet = Sheet::new("Test");

        // Set up a formula that references rows
        let cell = Cell {
            content: CellContent::Formula {
                expression: "=A1+A5".to_string(),
                cached_value: CellValue::Empty,
            },
            format: Default::default(),
        };
        sheet.set_cell(CellCoord::new(0, 0), cell);

        let mut cmd = DeleteRowsCommand::new(2, 2);

        // Execute
        cmd.execute(&mut sheet);

        // Formula should be updated: A5 becomes A3 (shifted up by 2)
        if let Some(cell) = sheet.get_cell(CellCoord::new(0, 0)) {
            if let CellContent::Formula { expression, .. } = &cell.content {
                assert_eq!(expression, "=A1+A3");
            } else {
                panic!("Expected formula");
            }
        } else {
            panic!("Cell not found");
        }

        // Undo
        cmd.undo(&mut sheet);

        // Formula should be restored
        if let Some(cell) = sheet.get_cell(CellCoord::new(0, 0)) {
            if let CellContent::Formula { expression, .. } = &cell.content {
                assert_eq!(expression, "=A1+A5");
            } else {
                panic!("Expected formula");
            }
        } else {
            panic!("Cell not found");
        }
    }

    #[test]
    fn test_insert_cols_command() {
        let mut sheet = Sheet::new("Test");

        // Set up some cells
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(0, 1), Cell::number(2.0));
        sheet.set_cell(CellCoord::new(0, 2), Cell::number(3.0));

        let mut cmd = InsertColsCommand::new(1, 2);

        // Execute
        cmd.execute(&mut sheet);

        // Column 0 should stay in place
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );

        // Columns 1 and 2 should be empty (newly inserted)
        assert!(sheet.get_cell(CellCoord::new(0, 1)).is_none());
        assert!(sheet.get_cell(CellCoord::new(0, 2)).is_none());

        // Old column 1 should shift to column 3
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 3)).as_number(),
            Some(2.0)
        );

        // Old column 2 should shift to column 4
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 4)).as_number(),
            Some(3.0)
        );

        // Undo
        cmd.undo(&mut sheet);

        // Should restore original state
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 1)).as_number(),
            Some(2.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 2)).as_number(),
            Some(3.0)
        );
        assert!(sheet.get_cell(CellCoord::new(0, 3)).is_none());
        assert!(sheet.get_cell(CellCoord::new(0, 4)).is_none());
    }

    #[test]
    fn test_insert_cols_updates_formulas() {
        let mut sheet = Sheet::new("Test");

        // Set up a formula that references columns
        let cell = Cell {
            content: CellContent::Formula {
                expression: "=C1+D1".to_string(),
                cached_value: CellValue::Empty,
            },
            format: Default::default(),
        };
        sheet.set_cell(CellCoord::new(0, 0), cell);

        let mut cmd = InsertColsCommand::new(2, 2);

        // Execute
        cmd.execute(&mut sheet);

        // Formula should be updated: C1 (col 2) becomes E1, D1 (col 3) becomes F1
        // Both references are >= col 2, so both shift by 2
        if let Some(cell) = sheet.get_cell(CellCoord::new(0, 0)) {
            if let CellContent::Formula { expression, .. } = &cell.content {
                assert_eq!(expression, "=E1+F1");
            } else {
                panic!("Expected formula");
            }
        } else {
            panic!("Cell not found");
        }

        // Undo
        cmd.undo(&mut sheet);

        // Formula should be restored
        if let Some(cell) = sheet.get_cell(CellCoord::new(0, 0)) {
            if let CellContent::Formula { expression, .. } = &cell.content {
                assert_eq!(expression, "=C1+D1");
            } else {
                panic!("Expected formula");
            }
        } else {
            panic!("Cell not found");
        }
    }

    #[test]
    fn test_delete_cols_command() {
        let mut sheet = Sheet::new("Test");

        // Set up some cells
        sheet.set_cell(CellCoord::new(0, 0), Cell::number(1.0));
        sheet.set_cell(CellCoord::new(0, 1), Cell::number(2.0));
        sheet.set_cell(CellCoord::new(0, 2), Cell::number(3.0));
        sheet.set_cell(CellCoord::new(0, 3), Cell::number(4.0));
        sheet.set_cell(CellCoord::new(0, 4), Cell::number(5.0));

        let mut cmd = DeleteColsCommand::new(1, 2);

        // Execute
        cmd.execute(&mut sheet);

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

        // Undo
        cmd.undo(&mut sheet);

        // Should restore original state
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 0)).as_number(),
            Some(1.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 1)).as_number(),
            Some(2.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 2)).as_number(),
            Some(3.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 3)).as_number(),
            Some(4.0)
        );
        assert_eq!(
            sheet.get_cell_value(CellCoord::new(0, 4)).as_number(),
            Some(5.0)
        );
    }

    #[test]
    fn test_delete_cols_updates_formulas() {
        let mut sheet = Sheet::new("Test");

        // Set up a formula that references columns
        let cell = Cell {
            content: CellContent::Formula {
                expression: "=A1+E1".to_string(),
                cached_value: CellValue::Empty,
            },
            format: Default::default(),
        };
        sheet.set_cell(CellCoord::new(0, 0), cell);

        let mut cmd = DeleteColsCommand::new(2, 2);

        // Execute
        cmd.execute(&mut sheet);

        // Formula should be updated: E1 becomes C1 (shifted left by 2)
        if let Some(cell) = sheet.get_cell(CellCoord::new(0, 0)) {
            if let CellContent::Formula { expression, .. } = &cell.content {
                assert_eq!(expression, "=A1+C1");
            } else {
                panic!("Expected formula");
            }
        } else {
            panic!("Cell not found");
        }

        // Undo
        cmd.undo(&mut sheet);

        // Formula should be restored
        if let Some(cell) = sheet.get_cell(CellCoord::new(0, 0)) {
            if let CellContent::Formula { expression, .. } = &cell.content {
                assert_eq!(expression, "=A1+E1");
            } else {
                panic!("Expected formula");
            }
        } else {
            panic!("Cell not found");
        }
    }
}
