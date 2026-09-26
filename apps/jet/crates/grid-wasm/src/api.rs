use jet_grid::core::{
    AlertStyle, CellContent, CellCoord, CellError, CellFormat, CellPosition, CellRange, CellValue,
    ConditionalFormattingRule, ConditionalRule, DataValidationRule, HorizontalAlign, RusheetError,
    Selection, ValidationAlert, ValidationCriteria, ValidationMessage, ValidationResult,
    VerticalAlign, Workbook,
};
use jet_grid::formula::{extract_references, DependencyGraph};
use jet_grid::history::{
    ApplyFilterCommand, ClearFilterCommand, ClearRangeCommand, DeleteColsCommand,
    DeleteRowsCommand, HistoryManager, InsertColsCommand, InsertRowsCommand, MergeCellsCommand,
    SetCellFormatCommand, SetCellValueCommand, SetRangeFormatCommand, SortRangeCommand,
    UnmergeCellsCommand,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use wasm_bindgen::prelude::*;

use crate::viewport::{pack_format, ViewportBuffer};

/// Main spreadsheet engine exposed to JavaScript
#[wasm_bindgen]
pub struct SpreadsheetEngine {
    workbook: Workbook,
    dep_graph: DependencyGraph,
    history: HistoryManager,
    /// Reusable buffer for viewport data (zero-copy optimization)
    viewport_buffer: ViewportBuffer,
    /// Selection state (managed in WASM for consistency)
    selection: Selection,
}

/// Structured error object for JavaScript
#[derive(Serialize)]
pub struct JsRuSheetError {
    code: String,
    message: String,
}

impl From<RusheetError> for JsRuSheetError {
    fn from(err: RusheetError) -> Self {
        Self {
            code: err.code().to_string(),
            message: err.to_string(),
        }
    }
}

impl JsRuSheetError {
    fn from_error<E: std::fmt::Display>(err: E) -> JsValue {
        let error = Self {
            code: "ERROR".to_string(),
            message: err.to_string(),
        };
        serde_wasm_bindgen::to_value(&error).unwrap_or(JsValue::NULL)
    }
}

fn to_js_error(err: RusheetError) -> JsValue {
    let js_error = JsRuSheetError::from(err);
    serde_wasm_bindgen::to_value(&js_error).unwrap_or(JsValue::NULL)
}

/// Cell data for JavaScript
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellData {
    pub value: Option<String>,
    pub display_value: String,
    pub formula: Option<String>,
    pub format: CellFormatData,
    pub row: u32,
    pub col: u32,
}

/// Cell format data for JavaScript
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct CellFormatData {
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub underline: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "fontSize")]
    pub font_size: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "textColor")]
    pub text_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "backgroundColor")]
    pub background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "horizontalAlign")]
    pub horizontal_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "verticalAlign")]
    pub vertical_align: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Merge range data for JavaScript
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRangeData {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
}

/// Merge info for a cell
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeInfo {
    pub master_row: u32,
    pub master_col: u32,
    pub row_span: u32,
    pub col_span: u32,
}

impl From<&CellFormat> for CellFormatData {
    fn from(format: &CellFormat) -> Self {
        use jet_grid::core::{HorizontalAlign, VerticalAlign};

        CellFormatData {
            bold: format.bold,
            italic: format.italic,
            underline: format.underline,
            font_size: format.font_size,
            text_color: format.text_color.map(|c| c.to_hex()),
            background_color: format.background_color.map(|c| c.to_hex()),
            horizontal_align: match format.horizontal_align {
                HorizontalAlign::Left => None,
                HorizontalAlign::Center => Some("center".to_string()),
                HorizontalAlign::Right => Some("right".to_string()),
            },
            vertical_align: match format.vertical_align {
                VerticalAlign::Middle => None,
                VerticalAlign::Top => Some("top".to_string()),
                VerticalAlign::Bottom => Some("bottom".to_string()),
            },
        }
    }
}

#[wasm_bindgen]
impl SpreadsheetEngine {
    /// Create a new spreadsheet engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            workbook: Workbook::new("Untitled"),
            dep_graph: DependencyGraph::new(),
            history: HistoryManager::new(100),
            viewport_buffer: ViewportBuffer::with_capacity(1000),
            selection: Selection::new(),
        }
    }

    /// Set cell value (handles both plain values and formulas)
    /// Returns JSON array of affected cell coordinates for re-render
    /// Or returns JSON error object if validation fails
    #[wasm_bindgen(js_name = setCellValue)]
    pub fn set_cell_value(&mut self, row: u32, col: u32, value: &str) -> String {
        // Parse input to CellValue first for validation
        let cell_value = self.parse_input_to_cell_value(value);

        // Check validation
        let validation_result =
            self.workbook
                .active_sheet()
                .validate_cell_value(row, col, &cell_value);

        if let ValidationResult::Invalid(alert) = validation_result {
            if alert.style == AlertStyle::Stop {
                // Return error - don't set the value
                return serde_json::json!({
                    "error": "validation_failed",
                    "alert": alert
                })
                .to_string();
            }
            // Warning/Information: proceed but could log
        }

        // Proceed with original logic
        let coord = CellCoord::new(row, col);
        let cmd = Box::new(SetCellValueCommand::from_input(coord, value));

        // Execute command
        let mut affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        // Update dependency graph and recalculate if this is a formula
        if value.starts_with('=') {
            let refs = extract_references(value);
            let deps: HashSet<_> = refs.into_iter().map(|(r, c)| (r, c)).collect();
            self.dep_graph.set_dependencies((row, col), deps);

            // Immediately evaluate the formula we just set
            self.recalculate_cell(row, col);

            // Then recalculate dependents
            if let Ok(order) = self.dep_graph.get_recalc_order((row, col)) {
                for (r, c) in order {
                    if (r, c) != (row, col) {
                        // Don't recalculate the formula cell twice
                        self.recalculate_cell(r, c);
                        affected.push(CellCoord::new(r, c));
                    }
                }
            }
        } else {
            // Clear dependencies for non-formula cells
            self.dep_graph.remove_cell((row, col));

            // Recalculate cells that depend on this one
            if let Ok(order) = self.dep_graph.get_recalc_order((row, col)) {
                for (r, c) in order {
                    if (r, c) != (row, col) {
                        self.recalculate_cell(r, c);
                        affected.push(CellCoord::new(r, c));
                    }
                }
            }
        }

        // Return affected cells as JSON
        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Recalculate a single cell's formula
    fn recalculate_cell(&mut self, row: u32, col: u32) {
        let coord = CellCoord::new(row, col);
        let current_sheet_name = self.workbook.active_sheet().name.to_string();

        let expression = {
            let sheet = self.workbook.active_sheet();
            if let Some(cell) = sheet.get_cell(coord) {
                if let CellContent::Formula { expression, .. } = &cell.content {
                    Some(expression.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(expression) = expression {
            // Log formula evaluation for debugging (only in WASM target)
            #[cfg(all(debug_assertions, target_arch = "wasm32"))]
            web_sys::console::log_1(
                &format!(
                    "[Formula] Evaluating cell ({}, {}): {}",
                    row, col, expression
                )
                .into(),
            );

            // Create closure to get cell values from any sheet
            let result = jet_grid::formula::evaluate_formula_cross_sheet(
                &expression,
                Some(&current_sheet_name),
                |sheet_name, r, c| {
                    let sheet = if let Some(name) = sheet_name {
                        match self.workbook.get_sheet_by_name(name) {
                            Some(s) => s,
                            None => return CellValue::Error(CellError::InvalidReference),
                        }
                    } else {
                        self.workbook.active_sheet()
                    };
                    sheet
                        .get_cell(CellCoord::new(r, c))
                        .map(|c| c.computed_value().clone())
                        .unwrap_or(CellValue::Empty)
                },
            );

            // Log result (only in WASM target)
            #[cfg(all(debug_assertions, target_arch = "wasm32"))]
            web_sys::console::log_1(
                &format!("[Formula] Result for ({}, {}): {:?}", row, col, result).into(),
            );

            // Check for errors and log them (only in WASM target)
            #[cfg(target_arch = "wasm32")]
            if matches!(result, CellValue::Error(_)) {
                web_sys::console::error_1(
                    &format!(
                        "[Formula Error] Cell ({}, {}) formula '{}' failed: {:?}",
                        row, col, expression, result
                    )
                    .into(),
                );
            }

            // Update cached value
            let sheet = self.workbook.active_sheet_mut();
            if let Some(cell) = sheet.get_cell(coord) {
                let new_content = CellContent::Formula {
                    expression,
                    cached_value: result,
                };
                let mut new_cell = cell.clone();
                new_cell.content = new_content;
                sheet.set_cell(coord, new_cell);
            }
        }
    }

    /// Helper to parse input to CellValue without setting it
    fn parse_input_to_cell_value(&self, input: &str) -> CellValue {
        if input.is_empty() {
            return CellValue::Empty;
        }

        // Check if it's a formula
        if input.starts_with('=') {
            // For validation purposes, treat formulas as text
            return CellValue::Text(input.to_string());
        }

        // Try to parse as number
        if let Ok(n) = input.parse::<f64>() {
            return CellValue::Number(n);
        }

        // Check for boolean
        match input.to_lowercase().as_str() {
            "true" => return CellValue::Boolean(true),
            "false" => return CellValue::Boolean(false),
            _ => {}
        }

        // Default to text
        CellValue::Text(input.to_string())
    }

    /// Get cell data for rendering
    #[wasm_bindgen(js_name = getCellData)]
    pub fn get_cell_data(&self, row: u32, col: u32) -> JsValue {
        let coord = CellCoord::new(row, col);
        let sheet = self.workbook.active_sheet();

        let data = if let Some(cell) = sheet.get_cell(coord) {
            // Apply conditional formatting to get effective format
            let base_format = &cell.format;
            let value = cell.content.computed_value();
            let effective_format = sheet.get_effective_format(row, col, base_format, value);

            CellData {
                value: Some(cell.content.original_input()),
                display_value: cell.content.display_value(),
                formula: cell.content.formula_expression().map(String::from),
                format: CellFormatData::from(&effective_format),
                row,
                col,
            }
        } else {
            CellData {
                value: None,
                display_value: String::new(),
                formula: None,
                format: CellFormatData::default(),
                row,
                col,
            }
        };

        serde_wasm_bindgen::to_value(&data).unwrap_or(JsValue::NULL)
    }

    /// Get visible cells for viewport (virtual scrolling)
    #[wasm_bindgen(js_name = getViewportData)]
    pub fn get_viewport_data(
        &self,
        start_row: u32,
        end_row: u32,
        start_col: u32,
        end_col: u32,
    ) -> String {
        let sheet = self.workbook.active_sheet();
        let mut cells: Vec<CellData> = Vec::new();

        for row in start_row..=end_row {
            // Skip hidden rows (filtered out)
            if sheet.is_row_hidden(row) {
                continue;
            }
            for col in start_col..=end_col {
                let coord = CellCoord::new(row, col);
                if let Some(cell) = sheet.get_cell(coord) {
                    // Apply conditional formatting to get effective format
                    let base_format = &cell.format;
                    let value = cell.content.computed_value();
                    let effective_format = sheet.get_effective_format(row, col, base_format, value);

                    cells.push(CellData {
                        value: Some(cell.content.original_input()),
                        display_value: cell.content.display_value(),
                        formula: cell.content.formula_expression().map(String::from),
                        format: CellFormatData::from(&effective_format),
                        row,
                        col,
                    });
                }
            }
        }

        serde_json::to_string(&cells).unwrap_or_else(|_| "[]".to_string())
    }

    /// Set cell format
    #[wasm_bindgen(js_name = setCellFormat)]
    pub fn set_cell_format(&mut self, row: u32, col: u32, format_json: &str) -> bool {
        let format_data: CellFormatData = match serde_json::from_str(format_json) {
            Ok(f) => f,
            Err(e) => {
                web_sys::console::log_1(
                    &format!(
                        "Error deserializing format for cell ({}, {}): {}",
                        row, col, e
                    )
                    .into(),
                );
                return false;
            }
        };

        let coord = CellCoord::new(row, col);
        let format = cell_format_from_data(&format_data);

        let cmd = Box::new(SetCellFormatCommand::new(coord, format));
        self.history.execute(cmd, self.workbook.active_sheet_mut());

        true
    }

    /// Apply format to range
    #[wasm_bindgen(js_name = setRangeFormat)]
    pub fn set_range_format(
        &mut self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
        format_json: &str,
    ) -> bool {
        let format_data: CellFormatData = match serde_json::from_str(format_json) {
            Ok(f) => f,
            Err(e) => {
                web_sys::console::log_1(
                    &format!(
                        "Error deserializing range format ({},{}) to ({},{}): {}",
                        start_row, start_col, end_row, end_col, e
                    )
                    .into(),
                );
                return false;
            }
        };

        let start = CellCoord::new(start_row, start_col);
        let end = CellCoord::new(end_row, end_col);
        let format = cell_format_from_data(&format_data);

        let cmd = Box::new(SetRangeFormatCommand::new(start, end, format));
        self.history.execute(cmd, self.workbook.active_sheet_mut());

        true
    }

    /// Clear a range of cells
    #[wasm_bindgen(js_name = clearRange)]
    pub fn clear_range(
        &mut self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> String {
        let start = CellCoord::new(start_row, start_col);
        let end = CellCoord::new(end_row, end_col);

        let cmd = Box::new(ClearRangeCommand::new(start, end));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        // Clear dependencies and recalculate dependents
        let mut all_affected: Vec<CellCoord> = affected.clone();
        for coord in &affected {
            self.dep_graph.remove_cell((coord.row, coord.col));

            if let Ok(order) = self.dep_graph.get_recalc_order((coord.row, coord.col)) {
                for (r, c) in order {
                    self.recalculate_cell(r, c);
                    all_affected.push(CellCoord::new(r, c));
                }
            }
        }

        let coords: Vec<[u32; 2]> = all_affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    // --- Undo/Redo ---

    /// Undo the last command
    #[wasm_bindgen]
    pub fn undo(&mut self) -> String {
        if let Some(affected) = self.history.undo(self.workbook.active_sheet_mut()) {
            // Recalculate all affected cells
            for coord in &affected {
                if let Ok(order) = self.dep_graph.get_recalc_order((coord.row, coord.col)) {
                    for (r, c) in order {
                        self.recalculate_cell(r, c);
                    }
                }
            }

            let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
            serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
        } else {
            "[]".to_string()
        }
    }

    /// Redo the last undone command
    #[wasm_bindgen]
    pub fn redo(&mut self) -> String {
        if let Some(affected) = self.history.redo(self.workbook.active_sheet_mut()) {
            // Recalculate all affected cells
            for coord in &affected {
                if let Ok(order) = self.dep_graph.get_recalc_order((coord.row, coord.col)) {
                    for (r, c) in order {
                        self.recalculate_cell(r, c);
                    }
                }
            }

            let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
            serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
        } else {
            "[]".to_string()
        }
    }

    #[wasm_bindgen(js_name = canUndo)]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    #[wasm_bindgen(js_name = canRedo)]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Clear undo/redo history
    #[wasm_bindgen(js_name = clearHistory)]
    pub fn clear_history(&mut self) {
        self.history = HistoryManager::new(100);
    }

    // --- Sheet Management ---

    /// Add a new sheet
    #[wasm_bindgen(js_name = addSheet)]
    pub fn add_sheet(&mut self, name: &str) -> Result<usize, JsValue> {
        self.workbook.add_sheet(name).map_err(to_js_error)
    }

    /// Set active sheet by index
    #[wasm_bindgen(js_name = setActiveSheet)]
    pub fn set_active_sheet(&mut self, index: usize) -> bool {
        self.workbook.set_active_sheet(index)
    }

    /// Get all sheet names as JSON array
    #[wasm_bindgen(js_name = getSheetNames)]
    pub fn get_sheet_names(&self) -> String {
        let names = self.workbook.sheet_names();
        serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get active sheet index
    #[wasm_bindgen(js_name = getActiveSheetIndex)]
    pub fn get_active_sheet_index(&self) -> usize {
        self.workbook.active_sheet_index
    }

    /// Rename a sheet
    #[wasm_bindgen(js_name = renameSheet)]
    pub fn rename_sheet(&mut self, index: usize, name: &str) -> Result<bool, JsValue> {
        self.workbook
            .rename_sheet(index, name)
            .map(|_| true)
            .map_err(to_js_error)
    }

    /// Delete a sheet
    #[wasm_bindgen(js_name = deleteSheet)]
    pub fn delete_sheet(&mut self, index: usize) -> Result<bool, JsValue> {
        self.workbook
            .remove_sheet(index)
            .map(|_| true)
            .map_err(to_js_error)
    }

    // --- Row/Column sizing ---

    #[wasm_bindgen(js_name = setRowHeight)]
    pub fn set_row_height(&mut self, row: u32, height: f64) {
        self.workbook.active_sheet_mut().set_row_height(row, height);
    }

    #[wasm_bindgen(js_name = setColWidth)]
    pub fn set_col_width(&mut self, col: u32, width: f64) {
        self.workbook.active_sheet_mut().set_col_width(col, width);
    }

    #[wasm_bindgen(js_name = getRowHeight)]
    pub fn get_row_height(&self, row: u32) -> f64 {
        self.workbook.active_sheet().get_row_height(row)
    }

    #[wasm_bindgen(js_name = getColWidth)]
    pub fn get_col_width(&self, col: u32) -> f64 {
        self.workbook.active_sheet().get_col_width(col)
    }

    // --- Row/Column Insert/Delete ---

    /// Insert rows at the given position
    /// Returns JSON array of affected cell coordinates
    #[wasm_bindgen(js_name = insertRows)]
    pub fn insert_rows(&mut self, at_row: u32, count: u32) -> String {
        let cmd = Box::new(InsertRowsCommand::new(at_row, count));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        // Rebuild dependency graph since cell references changed
        self.rebuild_dependency_graph();

        // Recalculate all formulas
        self.recalculate_all();

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Delete rows at the given position
    /// Returns JSON array of affected cell coordinates
    #[wasm_bindgen(js_name = deleteRows)]
    pub fn delete_rows(&mut self, at_row: u32, count: u32) -> String {
        let cmd = Box::new(DeleteRowsCommand::new(at_row, count));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        // Rebuild dependency graph since cell references changed
        self.rebuild_dependency_graph();

        // Recalculate all formulas
        self.recalculate_all();

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Insert columns at the given position
    /// Returns JSON array of affected cell coordinates
    #[wasm_bindgen(js_name = insertCols)]
    pub fn insert_cols(&mut self, at_col: u32, count: u32) -> String {
        let cmd = Box::new(InsertColsCommand::new(at_col, count));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        // Rebuild dependency graph since cell references changed
        self.rebuild_dependency_graph();

        // Recalculate all formulas
        self.recalculate_all();

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Delete columns at the given position
    /// Returns JSON array of affected cell coordinates
    #[wasm_bindgen(js_name = deleteCols)]
    pub fn delete_cols(&mut self, at_col: u32, count: u32) -> String {
        let cmd = Box::new(DeleteColsCommand::new(at_col, count));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        // Rebuild dependency graph since cell references changed
        self.rebuild_dependency_graph();

        // Recalculate all formulas
        self.recalculate_all();

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Sort a range of rows by a specific column
    /// Returns JSON array of affected cell coordinates
    #[wasm_bindgen(js_name = sortRange)]
    pub fn sort_range(
        &mut self,
        start_row: u32,
        end_row: u32,
        start_col: u32,
        end_col: u32,
        sort_col: u32,
        ascending: bool,
    ) -> String {
        let cmd = Box::new(SortRangeCommand::new(
            start_row, end_row, start_col, end_col, sort_col, ascending,
        ));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        // Recalculate formulas in the sorted range
        self.recalculate_all();

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    // --- Cell Merging ---

    /// Merge cells in a range. Returns JSON array of affected cell coordinates.
    #[wasm_bindgen(js_name = mergeCells)]
    pub fn merge_cells(
        &mut self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> String {
        let cmd = Box::new(MergeCellsCommand::from_coords(
            start_row, start_col, end_row, end_col,
        ));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Unmerge cells at a position. Returns JSON array of affected cell coordinates.
    #[wasm_bindgen(js_name = unmergeCells)]
    pub fn unmerge_cells(&mut self, row: u32, col: u32) -> String {
        let coord = CellCoord::new(row, col);
        let cmd = Box::new(UnmergeCellsCommand::from_coord(coord));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get merged ranges as JSON array of objects with start/end coordinates.
    #[wasm_bindgen(js_name = getMergedRanges)]
    pub fn get_merged_ranges(&self) -> String {
        let sheet = self.workbook.active_sheet();
        let ranges: Vec<MergeRangeData> = sheet
            .get_merged_ranges()
            .iter()
            .map(|r| MergeRangeData {
                start_row: r.start.row,
                start_col: r.start.col,
                end_row: r.end.row,
                end_col: r.end.col,
            })
            .collect();

        serde_json::to_string(&ranges).unwrap_or_else(|_| "[]".to_string())
    }

    /// Check if a cell is part of a merged range (but not the master cell).
    #[wasm_bindgen(js_name = isMergedSlave)]
    pub fn is_merged_slave(&self, row: u32, col: u32) -> bool {
        let coord = CellCoord::new(row, col);
        self.workbook.active_sheet().is_merged_slave(coord)
    }

    /// Get the merge info for a cell (returns null if not merged).
    #[wasm_bindgen(js_name = getMergeInfo)]
    pub fn get_merge_info(&self, row: u32, col: u32) -> JsValue {
        let coord = CellCoord::new(row, col);
        let sheet = self.workbook.active_sheet();

        if let Some(range) = sheet.get_merge_at(coord) {
            let info = MergeInfo {
                master_row: range.start.row,
                master_col: range.start.col,
                row_span: range.row_span(),
                col_span: range.col_span(),
            };
            serde_wasm_bindgen::to_value(&info).unwrap_or(JsValue::NULL)
        } else {
            JsValue::NULL
        }
    }

    /// Check if any merged range overlaps the given range.
    #[wasm_bindgen(js_name = hasMergeInRange)]
    pub fn has_merge_in_range(
        &self,
        start_row: u32,
        end_row: u32,
        start_col: u32,
        end_col: u32,
    ) -> bool {
        self.workbook
            .active_sheet()
            .has_merge_in_range(start_row, end_row, start_col, end_col)
    }

    // --- Filtering ---

    /// Get unique values in a column for filter dropdown
    #[wasm_bindgen(js_name = getUniqueValuesInColumn)]
    pub fn get_unique_values_in_column(&self, col: u32, max_rows: u32) -> String {
        let sheet = self.workbook.active_sheet();
        let values = sheet.get_unique_values_in_column(col, max_rows);
        serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_string())
    }

    /// Apply a column filter
    /// values_json is a JSON array of string values to show
    /// Returns JSON array of affected row indices
    #[wasm_bindgen(js_name = applyColumnFilter)]
    pub fn apply_column_filter(&mut self, col: u32, values_json: &str, max_rows: u32) -> String {
        // Parse the values
        let visible_values: HashSet<String> = match serde_json::from_str(values_json) {
            Ok(v) => v,
            Err(_) => return "[]".to_string(),
        };

        let cmd = Box::new(ApplyFilterCommand::new(col, visible_values, max_rows));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Clear filter on a specific column
    /// Returns JSON array of affected row indices
    #[wasm_bindgen(js_name = clearColumnFilter)]
    pub fn clear_column_filter(&mut self, col: u32) -> String {
        let cmd = Box::new(ClearFilterCommand::new(Some(col)));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Clear all filters
    /// Returns JSON array of affected row indices
    #[wasm_bindgen(js_name = clearAllFilters)]
    pub fn clear_all_filters(&mut self) -> String {
        let cmd = Box::new(ClearFilterCommand::new(None));
        let affected = self.history.execute(cmd, self.workbook.active_sheet_mut());

        let coords: Vec<[u32; 2]> = affected.iter().map(|c| [c.row, c.col]).collect();
        serde_json::to_string(&coords).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get active filters as JSON
    #[wasm_bindgen(js_name = getActiveFilters)]
    pub fn get_active_filters(&self) -> String {
        let sheet = self.workbook.active_sheet();
        let filters = sheet.get_active_filters();

        // Convert to a simpler format for JS
        let js_filters: Vec<serde_json::Value> = filters
            .iter()
            .map(|f| {
                serde_json::json!({
                    "col": f.col,
                    "visibleValues": f.visible_values.iter().collect::<Vec<_>>()
                })
            })
            .collect();

        serde_json::to_string(&js_filters).unwrap_or_else(|_| "[]".to_string())
    }

    /// Check if a row is hidden
    #[wasm_bindgen(js_name = isRowHidden)]
    pub fn is_row_hidden(&self, row: u32) -> bool {
        self.workbook.active_sheet().is_row_hidden(row)
    }

    /// Get all hidden rows
    #[wasm_bindgen(js_name = getHiddenRows)]
    pub fn get_hidden_rows(&self) -> String {
        let hidden = self.workbook.active_sheet().get_hidden_rows();
        serde_json::to_string(&hidden).unwrap_or_else(|_| "[]".to_string())
    }

    // --- Search and Replace ---

    /// Search for cells matching the query
    ///
    /// options_json format:
    /// {
    ///   "query": "search text",
    ///   "match_case": false,
    ///   "match_entire_cell": false,
    ///   "use_regex": false,
    ///   "search_formulas": false,
    ///   "sheet_indices": null  // or [0, 1] to search specific sheets
    /// }
    ///
    /// Returns JSON array of SearchResult
    #[wasm_bindgen]
    pub fn search(&self, options_json: &str) -> Result<String, JsValue> {
        let options: jet_grid::core::SearchOptions =
            serde_json::from_str(options_json).map_err(JsRuSheetError::from_error)?;

        let results = jet_grid::core::SearchEngine::search(&self.workbook, &options)
            .map_err(JsRuSheetError::from_error)?;

        serde_json::to_string(&results).map_err(JsRuSheetError::from_error)
    }

    /// Replace text in cells matching the query
    ///
    /// options_json format:
    /// {
    ///   "query": "search text",
    ///   "replacement": "new text",
    ///   "match_case": false,
    ///   "match_entire_cell": false,
    ///   "use_regex": false,
    ///   "search_formulas": false,
    ///   "sheet_indices": null
    /// }
    ///
    /// Returns JSON array of replaced cells (SearchResult format)
    #[wasm_bindgen]
    pub fn replace(&mut self, options_json: &str) -> Result<String, JsValue> {
        let options: jet_grid::core::ReplaceOptions =
            serde_json::from_str(options_json).map_err(JsRuSheetError::from_error)?;

        let results = jet_grid::core::SearchEngine::replace(&mut self.workbook, &options)
            .map_err(JsRuSheetError::from_error)?;

        serde_json::to_string(&results).map_err(JsRuSheetError::from_error)
    }

    /// Simple search in current sheet only
    #[wasm_bindgen(js_name = searchCurrentSheet)]
    pub fn search_current_sheet(&self, query: &str, match_case: bool) -> Result<String, JsValue> {
        let options = jet_grid::core::SearchOptions {
            query: query.to_string(),
            match_case,
            match_entire_cell: false,
            use_regex: false,
            search_formulas: false,
            sheet_indices: Some(vec![self.workbook.active_sheet_index]),
        };

        let results = jet_grid::core::SearchEngine::search(&self.workbook, &options)
            .map_err(JsRuSheetError::from_error)?;

        serde_json::to_string(&results).map_err(JsRuSheetError::from_error)
    }

    // --- Conditional Formatting ---

    /// Add a conditional formatting rule
    ///
    /// rule_json format:
    /// {
    ///   "range": { "start_row": 0, "start_col": 0, "end_row": 10, "end_col": 5 },
    ///   "rule": {
    ///     "type": "value_based",
    ///     "operator": "greater_than",
    ///     "value1": 50,
    ///     "format": { "background_color": { "r": 255, "g": 0, "b": 0, "a": 255 } }
    ///   },
    ///   "priority": 0,
    ///   "enabled": true
    /// }
    ///
    /// Returns the generated rule ID
    #[wasm_bindgen(js_name = addConditionalFormatting)]
    pub fn add_conditional_formatting(&mut self, rule_json: &str) -> Result<String, JsValue> {
        #[derive(Deserialize)]
        struct RuleInput {
            range: CellRange,
            rule: ConditionalRule,
            #[serde(default)]
            priority: i32,
            #[serde(default = "default_true")]
            enabled: bool,
        }
        fn default_true() -> bool {
            true
        }

        let input: RuleInput =
            serde_json::from_str(rule_json).map_err(JsRuSheetError::from_error)?;

        let rule_id = uuid::Uuid::new_v4().to_string();
        let rule = ConditionalFormattingRule {
            id: rule_id.clone(),
            range: input.range,
            rule: input.rule,
            priority: input.priority,
            enabled: input.enabled,
        };

        self.workbook
            .active_sheet_mut()
            .add_conditional_formatting(rule);

        Ok(rule_id)
    }

    /// Remove a conditional formatting rule by ID
    #[wasm_bindgen(js_name = removeConditionalFormatting)]
    pub fn remove_conditional_formatting(&mut self, rule_id: &str) -> bool {
        self.workbook
            .active_sheet_mut()
            .remove_conditional_formatting(rule_id)
    }

    /// Get all conditional formatting rules for the active sheet
    #[wasm_bindgen(js_name = getConditionalFormattingRules)]
    pub fn get_conditional_formatting_rules(&self) -> Result<String, JsValue> {
        let rules = &self.workbook.active_sheet().conditional_formatting;
        serde_json::to_string(rules).map_err(JsRuSheetError::from_error)
    }

    /// Update an existing conditional formatting rule
    #[wasm_bindgen(js_name = updateConditionalFormatting)]
    pub fn update_conditional_formatting(
        &mut self,
        rule_id: &str,
        rule_json: &str,
    ) -> Result<bool, JsValue> {
        #[derive(Deserialize)]
        struct UpdateInput {
            range: Option<CellRange>,
            rule: Option<ConditionalRule>,
            priority: Option<i32>,
            enabled: Option<bool>,
        }

        let input: UpdateInput =
            serde_json::from_str(rule_json).map_err(JsRuSheetError::from_error)?;

        let sheet = self.workbook.active_sheet_mut();
        if let Some(existing) = sheet
            .conditional_formatting
            .iter_mut()
            .find(|r| r.id == rule_id)
        {
            if let Some(range) = input.range {
                existing.range = range;
            }
            if let Some(rule) = input.rule {
                existing.rule = rule;
            }
            if let Some(priority) = input.priority {
                existing.priority = priority;
            }
            if let Some(enabled) = input.enabled {
                existing.enabled = enabled;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Clear all conditional formatting rules from active sheet
    #[wasm_bindgen(js_name = clearConditionalFormatting)]
    pub fn clear_conditional_formatting(&mut self) {
        self.workbook
            .active_sheet_mut()
            .conditional_formatting
            .clear();
    }

    // --- Data Validation ---

    /// Add a data validation rule
    ///
    /// rule_json format:
    /// {
    ///   "range": { "start_row": 0, "start_col": 0, "end_row": 10, "end_col": 0 },
    ///   "criteria": {
    ///     "type": "list",
    ///     "source": { "type": "values", "items": ["Yes", "No", "Maybe"] },
    ///     "show_dropdown": true
    ///   },
    ///   "allow_blank": true,
    ///   "error_alert": {
    ///     "style": "stop",
    ///     "title": "Invalid Input",
    ///     "message": "Please select from the list"
    ///   }
    /// }
    ///
    /// Returns the generated rule ID
    #[wasm_bindgen(js_name = addDataValidation)]
    pub fn add_data_validation(&mut self, rule_json: &str) -> Result<String, JsValue> {
        #[derive(Deserialize)]
        struct RuleInput {
            range: CellRange,
            criteria: ValidationCriteria,
            #[serde(default = "default_true")]
            allow_blank: bool,
            #[serde(default)]
            input_message: Option<ValidationMessage>,
            #[serde(default)]
            error_alert: ValidationAlert,
            #[serde(default = "default_true")]
            enabled: bool,
        }
        fn default_true() -> bool {
            true
        }

        let input: RuleInput =
            serde_json::from_str(rule_json).map_err(JsRuSheetError::from_error)?;

        let rule_id = uuid::Uuid::new_v4().to_string();
        let rule = DataValidationRule {
            id: rule_id.clone(),
            range: input.range,
            criteria: input.criteria,
            allow_blank: input.allow_blank,
            input_message: input.input_message,
            error_alert: input.error_alert,
            enabled: input.enabled,
        };

        self.workbook.active_sheet_mut().add_data_validation(rule);

        Ok(rule_id)
    }

    /// Remove a data validation rule by ID
    #[wasm_bindgen(js_name = removeDataValidation)]
    pub fn remove_data_validation(&mut self, rule_id: &str) -> bool {
        self.workbook
            .active_sheet_mut()
            .remove_data_validation(rule_id)
    }

    /// Get all data validation rules for the active sheet
    #[wasm_bindgen(js_name = getDataValidationRules)]
    pub fn get_data_validation_rules(&self) -> Result<String, JsValue> {
        let rules = self.workbook.active_sheet().get_data_validation_rules();
        serde_json::to_string(rules).map_err(JsRuSheetError::from_error)
    }

    /// Get dropdown items for a cell (if it has list validation)
    #[wasm_bindgen(js_name = getCellDropdownItems)]
    pub fn get_cell_dropdown_items(&self, row: u32, col: u32) -> Result<JsValue, JsValue> {
        let items = self
            .workbook
            .active_sheet()
            .get_cell_dropdown_items(row, col);
        match items {
            Some(items) => {
                let js_array = js_sys::Array::new();
                for item in items {
                    js_array.push(&JsValue::from_str(&item));
                }
                Ok(js_array.into())
            }
            None => Ok(JsValue::NULL),
        }
    }

    /// Validate a value against the cell's validation rule
    /// Returns: { "valid": true } or { "valid": false, "alert": { "style": "stop", "title": ..., "message": ... } }
    #[wasm_bindgen(js_name = validateCellValue)]
    pub fn validate_cell_value(&self, row: u32, col: u32, value: &str) -> Result<String, JsValue> {
        let cell_value = self.parse_input_to_cell_value(value);
        let result = self
            .workbook
            .active_sheet()
            .validate_cell_value(row, col, &cell_value);

        #[derive(Serialize)]
        struct ValidationResponse {
            valid: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            alert: Option<ValidationAlert>,
            #[serde(skip_serializing_if = "Option::is_none")]
            needs_context: Option<bool>,
        }

        let response = match result {
            ValidationResult::Valid => ValidationResponse {
                valid: true,
                alert: None,
                needs_context: None,
            },
            ValidationResult::Invalid(alert) => ValidationResponse {
                valid: false,
                alert: Some(alert),
                needs_context: None,
            },
            ValidationResult::NeedsContext => ValidationResponse {
                valid: true, // Allow for now, full validation needs evaluator
                alert: None,
                needs_context: Some(true),
            },
        };

        serde_json::to_string(&response).map_err(JsRuSheetError::from_error)
    }

    /// Clear all data validation rules from active sheet
    #[wasm_bindgen(js_name = clearDataValidation)]
    pub fn clear_data_validation(&mut self) {
        self.workbook.active_sheet_mut().data_validation.clear();
    }

    // --- Serialization ---

    /// Serialize workbook to JSON
    #[wasm_bindgen]
    pub fn serialize(&self) -> String {
        self.workbook.to_json().unwrap_or_else(|_| "{}".to_string())
    }

    /// Deserialize workbook from JSON
    #[wasm_bindgen]
    pub fn deserialize(&mut self, json: &str) -> bool {
        match Workbook::from_json(json) {
            Ok(wb) => {
                self.workbook = wb;
                self.rebuild_dependency_graph();
                self.history.clear();
                true
            }
            Err(_) => false,
        }
    }

    /// Rebuild dependency graph from current workbook state
    fn rebuild_dependency_graph(&mut self) {
        self.dep_graph.clear();

        for sheet in &self.workbook.sheets {
            for coord in sheet.non_empty_coords() {
                if let Some(cell) = sheet.get_cell(coord) {
                    if let Some(expression) = cell.content.formula_expression() {
                        let refs = extract_references(expression);
                        let deps: HashSet<_> = refs.into_iter().collect();
                        self.dep_graph
                            .set_dependencies((coord.row, coord.col), deps);
                    }
                }
            }
        }
    }

    /// Recalculate all formulas in the workbook
    #[wasm_bindgen(js_name = recalculateAll)]
    pub fn recalculate_all(&mut self) {
        let sheet = self.workbook.active_sheet();
        let coords: Vec<CellCoord> = sheet.non_empty_coords().collect();

        for coord in coords {
            self.recalculate_cell(coord.row, coord.col);
        }
    }

    /// Get total dimensions of the spreadsheet
    #[wasm_bindgen(js_name = getDimensions)]
    pub fn get_dimensions(&self) -> String {
        let sheet = self.workbook.active_sheet();

        // Use a reasonable limit for scrollable area for now
        let max_rows = 1000;
        let max_cols = 26;

        let width = sheet.col_x_position(max_cols);
        let height = sheet.row_y_position(max_rows);

        serde_json::to_string(&serde_json::json!({
            "width": width,
            "height": height,
            "rows": max_rows,
            "cols": max_cols
        }))
        .unwrap_or_else(|_| "{}".to_string())
    }

    /// Get cell coordinates from pixel coordinates
    #[wasm_bindgen(js_name = getCellFromPixel)]
    pub fn get_cell_from_pixel(&self, x: f64, y: f64) -> Vec<u32> {
        let sheet = self.workbook.active_sheet();
        let row = sheet.row_at_y(y);
        let col = sheet.col_at_x(x);
        vec![row, col]
    }

    // =========================================================================
    // Zero-Copy Viewport API
    // =========================================================================

    /// Populate the internal viewport buffer with cells in the given range.
    /// Call this before accessing the viewport arrays.
    #[wasm_bindgen(js_name = populateViewport)]
    pub fn populate_viewport(
        &mut self,
        start_row: u32,
        end_row: u32,
        start_col: u32,
        end_col: u32,
    ) {
        self.viewport_buffer.clear();
        let sheet = self.workbook.active_sheet();

        for row in start_row..=end_row {
            // Skip hidden rows (filtered out)
            if sheet.is_row_hidden(row) {
                continue;
            }
            for col in start_col..=end_col {
                let coord = CellCoord::new(row, col);
                if let Some(cell) = sheet.get_cell(coord) {
                    // Extract numeric value (NaN for non-numeric)
                    let numeric_value = match &cell.content {
                        CellContent::Value {
                            value: CellValue::Number(n),
                            ..
                        } => *n,
                        CellContent::Formula {
                            cached_value: CellValue::Number(n),
                            ..
                        } => *n,
                        _ => f64::NAN,
                    };

                    // Apply conditional formatting to get effective format
                    let base_format = &cell.format;
                    let value = cell.content.computed_value();
                    let effective_format = sheet.get_effective_format(row, col, base_format, value);

                    // Pack format flags using effective format
                    let h_align = match effective_format.horizontal_align {
                        HorizontalAlign::Left => 0,
                        HorizontalAlign::Center => 1,
                        HorizontalAlign::Right => 2,
                    };
                    let v_align = match effective_format.vertical_align {
                        VerticalAlign::Middle => 0,
                        VerticalAlign::Top => 1,
                        VerticalAlign::Bottom => 2,
                    };
                    let format_flags = pack_format(
                        effective_format.bold,
                        effective_format.italic,
                        effective_format.underline,
                        effective_format.font_size,
                        h_align,
                        v_align,
                    );

                    self.viewport_buffer.push(
                        row,
                        col,
                        numeric_value,
                        format_flags,
                        cell.content.display_value(),
                    );
                }
            }
        }
    }

    /// Get the number of cells in the viewport buffer
    #[wasm_bindgen(js_name = getViewportLen)]
    pub fn get_viewport_len(&self) -> usize {
        self.viewport_buffer.len()
    }

    /// Get pointer to viewport row indices (Uint32Array)
    #[wasm_bindgen(js_name = getViewportRowsPtr)]
    pub fn get_viewport_rows_ptr(&self) -> *const u32 {
        self.viewport_buffer.rows.as_ptr()
    }

    /// Get pointer to viewport column indices (Uint32Array)
    #[wasm_bindgen(js_name = getViewportColsPtr)]
    pub fn get_viewport_cols_ptr(&self) -> *const u32 {
        self.viewport_buffer.cols.as_ptr()
    }

    /// Get pointer to viewport numeric values (Float64Array)
    #[wasm_bindgen(js_name = getViewportValuesPtr)]
    pub fn get_viewport_values_ptr(&self) -> *const f64 {
        self.viewport_buffer.values.as_ptr()
    }

    /// Get pointer to viewport format flags (Uint32Array)
    #[wasm_bindgen(js_name = getViewportFormatsPtr)]
    pub fn get_viewport_formats_ptr(&self) -> *const u32 {
        self.viewport_buffer.formats.as_ptr()
    }

    /// Get viewport display values as JSON (strings still need serialization)
    #[wasm_bindgen(js_name = getViewportDisplayValues)]
    pub fn get_viewport_display_values(&self) -> String {
        serde_json::to_string(&self.viewport_buffer.display_values)
            .unwrap_or_else(|_| "[]".to_string())
    }
}

impl Default for SpreadsheetEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Selection data for JavaScript
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionData {
    pub active_cell: [u32; 2],
    pub primary: SelectionRangeData,
    pub additional: Vec<SelectionRangeData>,
}

/// Selection range data for JavaScript
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRangeData {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
}

/// Aggregation results for JavaScript
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregationData {
    pub sum: f64,
    pub average: f64,
    pub count: u32,
    pub numeric_count: u32,
    pub min: f64,
    pub max: f64,
}

#[wasm_bindgen]
impl SpreadsheetEngine {
    // =========================================================================
    // Selection API
    // =========================================================================

    /// Set selection to a single cell
    #[wasm_bindgen(js_name = setSelection)]
    pub fn set_selection(&mut self, row: u32, col: u32) {
        self.selection
            .select_cell(CellPosition::new(row as usize, col as usize));
    }

    /// Get current selection state as JSON
    #[wasm_bindgen(js_name = getSelection)]
    pub fn get_selection(&self) -> String {
        let active = self.selection.active_cell();
        let primary = self.selection.primary_range().normalize();

        let additional: Vec<SelectionRangeData> = self
            .selection
            .all_ranges()
            .skip(1) // Skip primary
            .map(|r| {
                let n = r.normalize();
                SelectionRangeData {
                    start_row: n.start.row as u32,
                    start_col: n.start.col as u32,
                    end_row: n.end.row as u32,
                    end_col: n.end.col as u32,
                }
            })
            .collect();

        let data = SelectionData {
            active_cell: [active.row as u32, active.col as u32],
            primary: SelectionRangeData {
                start_row: primary.start.row as u32,
                start_col: primary.start.col as u32,
                end_row: primary.end.row as u32,
                end_col: primary.end.col as u32,
            },
            additional,
        };

        serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
    }

    /// Extend primary selection to a new position (for Shift+Click or Shift+Arrow)
    #[wasm_bindgen(js_name = extendSelection)]
    pub fn extend_selection(&mut self, row: u32, col: u32) {
        self.selection
            .extend_to(CellPosition::new(row as usize, col as usize));
    }

    /// Add a new selection range (for Ctrl+Click multi-selection)
    #[wasm_bindgen(js_name = addSelection)]
    pub fn add_selection(&mut self, row: u32, col: u32) {
        // Finalize current primary as additional range, then start new primary
        let current_primary = self.selection.primary_range();
        self.selection.add_range(current_primary);
        self.selection
            .select_cell(CellPosition::new(row as usize, col as usize));
    }

    /// Select an entire row
    #[wasm_bindgen(js_name = selectRow)]
    pub fn select_row(&mut self, row: u32) {
        self.selection.select_row(row as usize, 999);
    }

    /// Select an entire column
    #[wasm_bindgen(js_name = selectColumn)]
    pub fn select_column(&mut self, col: u32) {
        self.selection.select_column(col as usize, 999);
    }

    /// Select all cells
    #[wasm_bindgen(js_name = selectAll)]
    pub fn select_all(&mut self) {
        self.selection.select_all(999, 999);
    }

    /// Get aggregation data for the current selection
    #[wasm_bindgen(js_name = getSelectionAggregation)]
    pub fn get_selection_aggregation(&self) -> String {
        let sheet = self.workbook.active_sheet();
        let mut sum = 0.0_f64;
        let mut count = 0_u32;
        let mut numeric_count = 0_u32;
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;

        for range in self.selection.all_ranges() {
            let normalized = range.normalize();
            for row in normalized.start.row..=normalized.end.row {
                for col in normalized.start.col..=normalized.end.col {
                    count += 1;
                    let coord = CellCoord::new(row as u32, col as u32);
                    if let Some(cell) = sheet.get_cell(coord) {
                        let value = cell.content.computed_value();
                        if let CellValue::Number(n) = value {
                            numeric_count += 1;
                            sum += n;
                            if *n < min {
                                min = *n;
                            }
                            if *n > max {
                                max = *n;
                            }
                        }
                    }
                }
            }
        }

        let average = if numeric_count > 0 {
            sum / numeric_count as f64
        } else {
            0.0
        };
        if numeric_count == 0 {
            min = 0.0;
            max = 0.0;
        }

        let data = AggregationData {
            sum,
            average,
            count,
            numeric_count,
            min,
            max,
        };

        serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Convert CellFormatData to CellFormat
fn cell_format_from_data(data: &CellFormatData) -> CellFormat {
    use jet_grid::core::{Color, HorizontalAlign, VerticalAlign};

    let mut format = CellFormat::default();
    format.bold = data.bold;
    format.italic = data.italic;
    format.underline = data.underline;
    format.font_size = data.font_size;

    if let Some(ref color) = data.text_color {
        format.text_color = Color::from_hex(color);
    }
    if let Some(ref color) = data.background_color {
        format.background_color = Color::from_hex(color);
    }

    if let Some(ref align) = data.horizontal_align {
        format.horizontal_align = match align.as_str() {
            "center" => HorizontalAlign::Center,
            "right" => HorizontalAlign::Right,
            _ => HorizontalAlign::Left,
        };
    }

    if let Some(ref align) = data.vertical_align {
        format.vertical_align = match align.as_str() {
            "top" => VerticalAlign::Top,
            "bottom" => VerticalAlign::Bottom,
            _ => VerticalAlign::Middle,
        };
    }

    format
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_format_data_deserialization() {
        // Test camelCase JSON from frontend
        let json = r##"{
            "bold": true,
            "italic": false,
            "fontSize": 12,
            "textColor": "#ff0000",
            "backgroundColor": "#00ff00",
            "horizontalAlign": "center",
            "verticalAlign": "top"
        }"##;

        let result: Result<CellFormatData, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "Failed to deserialize camelCase format data"
        );

        let format = result.unwrap();
        assert_eq!(format.bold, true);
        assert_eq!(format.font_size, Some(12));
        assert_eq!(format.text_color, Some("#ff0000".to_string()));
        assert_eq!(format.background_color, Some("#00ff00".to_string()));
        assert_eq!(format.horizontal_align, Some("center".to_string()));
        assert_eq!(format.vertical_align, Some("top".to_string()));
    }

    #[test]
    fn test_cell_format_data_round_trip() {
        let format = CellFormatData {
            bold: true,
            italic: false,
            underline: false,
            font_size: Some(14),
            text_color: Some("#ff0000".to_string()),
            background_color: Some("#00ff00".to_string()),
            horizontal_align: Some("right".to_string()),
            vertical_align: Some("bottom".to_string()),
        };

        let json = serde_json::to_string(&format).unwrap();
        let deserialized: CellFormatData = serde_json::from_str(&json).unwrap();

        assert_eq!(format.bold, deserialized.bold);
        assert_eq!(format.font_size, deserialized.font_size);
        assert_eq!(format.text_color, deserialized.text_color);
        assert_eq!(format.background_color, deserialized.background_color);
        assert_eq!(format.horizontal_align, deserialized.horizontal_align);
        assert_eq!(format.vertical_align, deserialized.vertical_align);
    }

    #[test]
    fn test_cell_format_data_partial_fields() {
        // Test that partial fields deserialize correctly
        let json = r##"{
            "bold": true,
            "textColor": "#ff0000"
        }"##;

        let format: CellFormatData = serde_json::from_str(json).unwrap();
        assert_eq!(format.bold, true);
        assert_eq!(format.italic, false); // default
        assert_eq!(format.text_color, Some("#ff0000".to_string()));
        assert_eq!(format.font_size, None); // not provided
    }
}

#[cfg(test)]
mod bug_fixes {
    use jet_grid::core::{CellCoord, Sheet};

    #[test]
    fn test_bug_1_3_number_preservation() {
        // Test at the core level without WASM
        let mut sheet = Sheet::new("Test");

        // Enter "10" in B2 (row=1, col=1)
        let coord = CellCoord::new(1, 1);
        let content = jet_grid::core::sheet::parse_cell_input("10");
        let cell = jet_grid::core::Cell::new(content);
        sheet.set_cell(coord, cell);

        // Verify original_input returns "10"
        let cell = sheet.get_cell(coord).unwrap();
        let original = cell.content.original_input();
        assert_eq!(
            original, "10",
            "Bug #1-3: Number '10' should be preserved as original input"
        );
        assert_eq!(cell.content.display_value(), "10");

        // Enter formula "=B2" in B3
        let coord_b3 = CellCoord::new(1, 2);
        let formula_content = jet_grid::core::sheet::parse_cell_input("=B2");
        let formula_cell = jet_grid::core::Cell::new(formula_content);
        sheet.set_cell(coord_b3, formula_cell);

        // Re-check B2 - should STILL be "10", not corrupted
        let cell_again = sheet.get_cell(coord).unwrap();
        let original_again = cell_again.content.original_input();
        assert_eq!(
            original_again, "10",
            "Bug #1-3: Re-reading B2 should still return '10', not 'B2C2'"
        );
    }

    #[test]
    fn test_original_input_preserved_for_all_types() {
        // Test at the core level without WASM
        let mut sheet = Sheet::new("Test");

        // Test number
        let content = jet_grid::core::sheet::parse_cell_input("42");
        sheet.set_cell(CellCoord::new(0, 0), jet_grid::core::Cell::new(content));
        let cell = sheet.get_cell(CellCoord::new(0, 0)).unwrap();
        assert_eq!(cell.content.original_input(), "42");

        // Test percentage
        let content = jet_grid::core::sheet::parse_cell_input("50%");
        sheet.set_cell(CellCoord::new(0, 1), jet_grid::core::Cell::new(content));
        let cell = sheet.get_cell(CellCoord::new(0, 1)).unwrap();
        assert_eq!(cell.content.original_input(), "50%");
        assert_eq!(cell.content.display_value(), "0.5");

        // Test boolean
        let content = jet_grid::core::sheet::parse_cell_input("TRUE");
        sheet.set_cell(CellCoord::new(0, 2), jet_grid::core::Cell::new(content));
        let cell = sheet.get_cell(CellCoord::new(0, 2)).unwrap();
        assert_eq!(cell.content.original_input(), "TRUE");

        // Test text
        let content = jet_grid::core::sheet::parse_cell_input("Hello");
        sheet.set_cell(CellCoord::new(0, 3), jet_grid::core::Cell::new(content));
        let cell = sheet.get_cell(CellCoord::new(0, 3)).unwrap();
        assert_eq!(cell.content.original_input(), "Hello");

        // Test formula
        let content = jet_grid::core::sheet::parse_cell_input("=A1+B1");
        sheet.set_cell(CellCoord::new(0, 4), jet_grid::core::Cell::new(content));
        let cell = sheet.get_cell(CellCoord::new(0, 4)).unwrap();
        assert_eq!(cell.content.original_input(), "=A1+B1");
    }

    // Helper function to get cell data with conditional formatting applied
    fn get_cell_as_data(engine: &super::SpreadsheetEngine, row: u32, col: u32) -> super::CellData {
        let coord = CellCoord::new(row, col);
        let sheet = engine.workbook.active_sheet();

        if let Some(cell) = sheet.get_cell(coord) {
            // Apply conditional formatting to get effective format
            let base_format = &cell.format;
            let value = cell.content.computed_value();
            let effective_format = sheet.get_effective_format(row, col, base_format, value);

            super::CellData {
                value: Some(cell.content.original_input()),
                display_value: cell.content.display_value(),
                formula: cell.content.formula_expression().map(String::from),
                format: super::CellFormatData::from(&effective_format),
                row,
                col,
            }
        } else {
            super::CellData {
                value: None,
                display_value: String::new(),
                formula: None,
                format: super::CellFormatData::default(),
                row,
                col,
            }
        }
    }

    #[test]
    fn test_bug_4_formula_evaluation() {
        let mut engine = super::SpreadsheetEngine::new();

        // Test case from bug report: "=5+3" should display "8"
        engine.set_cell_value(0, 0, "=5+3");

        let data = get_cell_as_data(&engine, 0, 0);

        assert_eq!(
            data.formula,
            Some("=5+3".to_string()),
            "Formula should be stored"
        );
        assert_eq!(
            data.display_value, "8",
            "Bug #4: '=5+3' should evaluate to '8', not empty"
        );
        assert_ne!(data.display_value, "", "Display value should not be empty");
    }

    #[test]
    fn test_formula_with_cell_references() {
        let mut engine = super::SpreadsheetEngine::new();

        // Set up: A1=10, A2=20
        engine.set_cell_value(0, 0, "10");
        engine.set_cell_value(1, 0, "20");

        // Formula: A3=A1+A2
        engine.set_cell_value(2, 0, "=A1+A2");

        let data = get_cell_as_data(&engine, 2, 0);
        assert_eq!(data.formula, Some("=A1+A2".to_string()));
        assert_eq!(data.display_value, "30", "A1+A2 should equal 30");
    }

    #[test]
    fn test_formula_multiplication_division() {
        let mut engine = super::SpreadsheetEngine::new();

        // Test multiplication
        engine.set_cell_value(0, 0, "=6*7");
        let data = get_cell_as_data(&engine, 0, 0);
        assert_eq!(data.display_value, "42", "6*7 should equal 42");

        // Test division
        engine.set_cell_value(0, 1, "=100/4");
        let data = get_cell_as_data(&engine, 0, 1);
        assert_eq!(data.display_value, "25", "100/4 should equal 25");

        // Test combined operations
        engine.set_cell_value(0, 2, "=(5+3)*2");
        let data = get_cell_as_data(&engine, 0, 2);
        assert_eq!(data.display_value, "16", "(5+3)*2 should equal 16");
    }

    #[test]
    fn test_formula_recalculation_on_dependency_change() {
        let mut engine = super::SpreadsheetEngine::new();

        // Set up: A1=5, B1=A1*2
        engine.set_cell_value(0, 0, "5");
        engine.set_cell_value(0, 1, "=A1*2");

        let data = get_cell_as_data(&engine, 0, 1);
        assert_eq!(data.display_value, "10", "Initial: A1*2 = 5*2 = 10");

        // Change A1 to 10
        engine.set_cell_value(0, 0, "10");

        // B1 should automatically recalculate
        let data = get_cell_as_data(&engine, 0, 1);
        assert_eq!(data.display_value, "20", "After change: A1*2 = 10*2 = 20");
    }

    #[test]
    fn test_bug_7_persistence() {
        let mut engine = super::SpreadsheetEngine::new();

        // User enters data
        engine.set_cell_value(0, 0, "Hello");
        engine.set_cell_value(1, 0, "World");
        engine.set_cell_value(2, 0, "=A1&\" \"&A2"); // Concatenation formula

        // Simulate page refresh: serialize and deserialize
        let json = engine.serialize();
        assert!(!json.is_empty(), "Serialized JSON should not be empty");

        let mut new_engine = super::SpreadsheetEngine::new();
        let success = new_engine.deserialize(&json);

        assert!(success, "Bug #7: Data should persist after refresh");

        // Verify data is still there
        let data1 = get_cell_as_data(&new_engine, 0, 0);
        assert_eq!(
            data1.value,
            Some("Hello".to_string()),
            "Bug #7: 'Hello' should persist"
        );

        let data2 = get_cell_as_data(&new_engine, 1, 0);
        assert_eq!(
            data2.value,
            Some("World".to_string()),
            "Bug #7: 'World' should persist"
        );

        // Verify formula persists and evaluates
        let data3 = get_cell_as_data(&new_engine, 2, 0);
        assert_eq!(
            data3.formula,
            Some("=A1&\" \"&A2".to_string()),
            "Formula should persist"
        );
    }

    #[test]
    fn test_serialization_preserves_formatting() {
        use serde_json::json;

        let mut engine = super::SpreadsheetEngine::new();

        // Set cell value
        engine.set_cell_value(0, 0, "Formatted");

        // Apply formatting
        let format = json!({
            "bold": true,
            "textColor": "#ff0000",
            "backgroundColor": "#ffff00"
        })
        .to_string();
        engine.set_cell_format(0, 0, &format);

        // Serialize and deserialize
        let json = engine.serialize();
        let mut new_engine = super::SpreadsheetEngine::new();
        new_engine.deserialize(&json);

        // Verify formatting persists
        let data = get_cell_as_data(&new_engine, 0, 0);
        assert_eq!(data.format.bold, true);
        assert_eq!(data.format.text_color, Some("#ff0000".to_string()));
        assert_eq!(data.format.background_color, Some("#ffff00".to_string()));
    }

    #[test]
    fn test_data_validation_integration() {
        use jet_grid::core::{
            AlertStyle, CellCoord, CellRange, DataValidationRule, ListSource, ValidationAlert,
            ValidationCriteria,
        };

        let mut engine = super::SpreadsheetEngine::new();

        // Add a list validation rule for column A (rows 0-10) directly
        let rule = DataValidationRule {
            id: "test-validation".to_string(),
            range: CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            criteria: ValidationCriteria::List {
                source: ListSource::Values {
                    items: vec!["Yes".into(), "No".into(), "Maybe".into()],
                },
                show_dropdown: true,
            },
            allow_blank: true,
            input_message: None,
            error_alert: ValidationAlert {
                style: AlertStyle::Stop,
                title: Some("Invalid Input".into()),
                message: Some("Please select from the list".into()),
            },
            enabled: true,
        };

        engine.workbook.active_sheet_mut().add_data_validation(rule);

        // Test valid value - should succeed
        let result = engine.set_cell_value(0, 0, "Yes");
        let coords: Result<Vec<[u32; 2]>, _> = serde_json::from_str(&result);
        assert!(coords.is_ok(), "Setting valid value should succeed");

        // Verify cell was actually set
        let cell_data = get_cell_as_data(&engine, 0, 0);
        assert_eq!(cell_data.value, Some("Yes".to_string()));

        // Test invalid value - should fail
        let result = engine.set_cell_value(1, 0, "Invalid");
        let error: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            error["error"], "validation_failed",
            "Invalid value should be rejected"
        );
        assert_eq!(error["alert"]["style"], "stop");

        // Verify cell was NOT set (should be empty)
        let cell_data = get_cell_as_data(&engine, 1, 0);
        assert_eq!(cell_data.value, None, "Invalid value should not be set");

        // Test that the rule exists
        let rules = engine.workbook.active_sheet().get_data_validation_rules();
        assert_eq!(rules.len(), 1, "Should have one validation rule");

        // Test removeDataValidation
        let removed = engine
            .workbook
            .active_sheet_mut()
            .remove_data_validation("test-validation");
        assert!(removed, "Rule should be removed");

        // After removal, invalid value should be accepted
        let result = engine.set_cell_value(2, 0, "Invalid");
        let coords: Result<Vec<[u32; 2]>, _> = serde_json::from_str(&result);
        assert!(
            coords.is_ok(),
            "After rule removal, any value should be accepted"
        );

        let cell_data = get_cell_as_data(&engine, 2, 0);
        assert_eq!(cell_data.value, Some("Invalid".to_string()));
    }

    #[test]
    fn test_data_validation_number_range() {
        use jet_grid::core::{
            AlertStyle, CellCoord, CellRange, DataValidationRule, ValidationAlert,
            ValidationCriteria, ValidationOperator,
        };

        let mut engine = super::SpreadsheetEngine::new();

        // Add a decimal validation rule (between 0 and 100) directly
        let rule = DataValidationRule {
            id: "test-number-validation".to_string(),
            range: CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            criteria: ValidationCriteria::Decimal {
                operator: ValidationOperator::Between,
                value1: 0.0,
                value2: Some(100.0),
            },
            allow_blank: false,
            input_message: None,
            error_alert: ValidationAlert {
                style: AlertStyle::Stop,
                title: Some("Out of Range".into()),
                message: Some("Value must be between 0 and 100".into()),
            },
            enabled: true,
        };

        engine.workbook.active_sheet_mut().add_data_validation(rule);

        // Test valid values
        let result = engine.set_cell_value(0, 0, "50");
        assert!(serde_json::from_str::<Vec<[u32; 2]>>(&result).is_ok());
        let cell_data = get_cell_as_data(&engine, 0, 0);
        assert_eq!(cell_data.display_value, "50");

        let result = engine.set_cell_value(1, 0, "0");
        assert!(serde_json::from_str::<Vec<[u32; 2]>>(&result).is_ok());

        let result = engine.set_cell_value(2, 0, "100");
        assert!(serde_json::from_str::<Vec<[u32; 2]>>(&result).is_ok());

        // Test invalid values
        let result = engine.set_cell_value(3, 0, "-1");
        let error: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(error["error"], "validation_failed");

        let result = engine.set_cell_value(4, 0, "101");
        let error: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(error["error"], "validation_failed");

        // Test blank (should fail because allow_blank is false)
        let result = engine.set_cell_value(5, 0, "");
        let error: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(error["error"], "validation_failed");
    }

    #[test]
    fn test_conditional_formatting_integration() {
        use jet_grid::core::{
            CellRange, Color, ComparisonOperator, ConditionalFormat, ConditionalFormattingRule,
            ConditionalRule,
        };

        let mut engine = super::SpreadsheetEngine::new();

        // Set up some cells with values
        engine.set_cell_value(0, 0, "100");
        engine.set_cell_value(1, 0, "50");
        engine.set_cell_value(2, 0, "25");

        // Add a conditional formatting rule directly: values > 50 get red background
        let rule = ConditionalFormattingRule {
            id: "test-rule".to_string(),
            range: CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 5)),
            rule: ConditionalRule::ValueBased {
                operator: ComparisonOperator::GreaterThan,
                value1: 50.0,
                value2: None,
                format: ConditionalFormat {
                    background_color: Some(Color {
                        r: 255,
                        g: 0,
                        b: 0,
                        a: 255,
                    }),
                    ..Default::default()
                },
            },
            priority: 0,
            enabled: true,
        };

        engine
            .workbook
            .active_sheet_mut()
            .add_conditional_formatting(rule);

        // Verify conditional formatting is applied
        let data_100 = get_cell_as_data(&engine, 0, 0);
        assert_eq!(data_100.display_value, "100");
        assert_eq!(
            data_100.format.background_color,
            Some("#ff0000".to_string()),
            "Cell with value 100 should have red background"
        );

        let data_50 = get_cell_as_data(&engine, 1, 0);
        assert_eq!(data_50.display_value, "50");
        assert_eq!(
            data_50.format.background_color, None,
            "Cell with value 50 should not have conditional formatting"
        );

        let data_25 = get_cell_as_data(&engine, 2, 0);
        assert_eq!(data_25.display_value, "25");
        assert_eq!(
            data_25.format.background_color, None,
            "Cell with value 25 should not have conditional formatting"
        );

        // Remove the rule
        let removed = engine
            .workbook
            .active_sheet_mut()
            .remove_conditional_formatting("test-rule");
        assert!(removed, "Rule should be successfully removed");

        // Verify conditional formatting is no longer applied
        let data_after_remove = get_cell_as_data(&engine, 0, 0);
        assert_eq!(
            data_after_remove.format.background_color, None,
            "After rule removal, cell should not have conditional formatting"
        );
    }
}
