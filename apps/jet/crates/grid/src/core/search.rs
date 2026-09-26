use crate::core::{CellContent, Sheet, Workbook};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Options for searching cells in a workbook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    /// The search query (literal string or regex pattern)
    pub query: String,
    /// Whether to match case-sensitively
    #[serde(default)]
    pub match_case: bool,
    /// Whether the entire cell value must match (vs. partial match)
    #[serde(default)]
    pub match_entire_cell: bool,
    /// Whether to interpret query as a regex pattern
    #[serde(default)]
    pub use_regex: bool,
    /// Whether to search formula text instead of displayed values
    #[serde(default)]
    pub search_formulas: bool,
    /// Optional list of sheet indices to search (None = all sheets)
    pub sheet_indices: Option<Vec<usize>>,
}

/// A single search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Name of the sheet containing the match
    pub sheet_name: String,
    /// Index of the sheet
    pub sheet_index: usize,
    /// Row coordinate
    pub row: u32,
    /// Column coordinate
    pub col: u32,
    /// The matched portion of text
    pub matched_text: String,
    /// The full cell value (for context)
    pub cell_value: String,
    /// Whether this cell contains a formula
    pub is_formula: bool,
}

/// Options for replacing text in cells
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceOptions {
    /// Search options
    #[serde(flatten)]
    pub search: SearchOptions,
    /// Replacement text (or regex replacement pattern if use_regex is true)
    pub replacement: String,
}

/// Errors that can occur during search/replace operations
#[derive(Debug, Clone)]
pub enum SearchError {
    /// Invalid regex pattern
    InvalidRegex(String),
    /// Attempted to replace text in a formula cell
    CannotReplaceFormula,
    /// Sheet index out of bounds
    InvalidSheetIndex(usize),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::InvalidRegex(msg) => write!(f, "Invalid regex pattern: {}", msg),
            SearchError::CannotReplaceFormula => write!(f, "Cannot replace text in formula cells"),
            SearchError::InvalidSheetIndex(idx) => write!(f, "Invalid sheet index: {}", idx),
        }
    }
}

impl std::error::Error for SearchError {}

/// Matcher trait for different matching strategies
trait Matcher {
    fn is_match(&self, text: &str) -> bool;
    fn find_match(&self, text: &str) -> Option<String>;
    fn replace_all(&self, text: &str, replacement: &str) -> String;
}

/// Literal string matcher
struct LiteralMatcher {
    query: String,
    match_case: bool,
    match_entire: bool,
}

impl Matcher for LiteralMatcher {
    fn is_match(&self, text: &str) -> bool {
        if self.match_entire {
            if self.match_case {
                text == self.query
            } else {
                text.eq_ignore_ascii_case(&self.query)
            }
        } else if self.match_case {
            text.contains(&self.query)
        } else {
            text.to_lowercase().contains(&self.query.to_lowercase())
        }
    }

    fn find_match(&self, text: &str) -> Option<String> {
        if self.is_match(text) {
            if self.match_entire {
                Some(text.to_string())
            } else {
                // Find the actual matched substring
                if self.match_case {
                    text.find(&self.query)
                        .map(|pos| text[pos..pos + self.query.len()].to_string())
                } else {
                    // Case-insensitive - need to find position
                    let lower_text = text.to_lowercase();
                    let lower_query = self.query.to_lowercase();
                    lower_text
                        .find(&lower_query)
                        .map(|pos| text[pos..pos + self.query.len()].to_string())
                }
            }
        } else {
            None
        }
    }

    fn replace_all(&self, text: &str, replacement: &str) -> String {
        if self.match_case {
            text.replace(&self.query, replacement)
        } else {
            // Case-insensitive replacement
            let lower_text = text.to_lowercase();
            let lower_query = self.query.to_lowercase();
            let mut result = String::new();
            let mut last_end = 0;

            for (idx, _) in lower_text.match_indices(&lower_query) {
                result.push_str(&text[last_end..idx]);
                result.push_str(replacement);
                last_end = idx + self.query.len();
            }
            result.push_str(&text[last_end..]);
            result
        }
    }
}

/// Regex matcher
struct RegexMatcher {
    regex: Regex,
    match_entire: bool,
}

impl Matcher for RegexMatcher {
    fn is_match(&self, text: &str) -> bool {
        if self.match_entire {
            self.regex.is_match(text) && {
                // Check if the match covers the entire text
                if let Some(mat) = self.regex.find(text) {
                    mat.start() == 0 && mat.end() == text.len()
                } else {
                    false
                }
            }
        } else {
            self.regex.is_match(text)
        }
    }

    fn find_match(&self, text: &str) -> Option<String> {
        if let Some(mat) = self.regex.find(text) {
            if self.match_entire {
                if mat.start() == 0 && mat.end() == text.len() {
                    Some(mat.as_str().to_string())
                } else {
                    None
                }
            } else {
                Some(mat.as_str().to_string())
            }
        } else {
            None
        }
    }

    fn replace_all(&self, text: &str, replacement: &str) -> String {
        self.regex.replace_all(text, replacement).to_string()
    }
}

/// Search engine for finding and replacing text in workbooks
pub struct SearchEngine;

impl SearchEngine {
    /// Search for cells matching the given options
    ///
    /// # Arguments
    ///
    /// * `workbook` - The workbook to search
    /// * `options` - Search options
    ///
    /// # Returns
    ///
    /// A vector of search results, ordered by sheet index, then row, then column
    pub fn search(
        workbook: &Workbook,
        options: &SearchOptions,
    ) -> Result<Vec<SearchResult>, SearchError> {
        // Build matcher
        let matcher = Self::build_matcher(options)?;

        // Determine which sheets to search
        let sheet_indices: Vec<usize> = match &options.sheet_indices {
            Some(indices) => {
                // Validate indices
                for &idx in indices {
                    if idx >= workbook.sheets.len() {
                        return Err(SearchError::InvalidSheetIndex(idx));
                    }
                }
                indices.clone()
            }
            None => (0..workbook.sheets.len()).collect(),
        };

        let mut results = Vec::new();

        // Search each sheet
        for &sheet_index in &sheet_indices {
            let sheet = &workbook.sheets[sheet_index];
            Self::search_sheet(sheet, sheet_index, matcher.as_ref(), options, &mut results);
        }

        Ok(results)
    }

    /// Replace text in cells matching the given options
    ///
    /// # Arguments
    ///
    /// * `workbook` - The workbook to modify
    /// * `options` - Replace options
    ///
    /// # Returns
    ///
    /// A vector of search results representing the cells that were modified
    pub fn replace(
        workbook: &mut Workbook,
        options: &ReplaceOptions,
    ) -> Result<Vec<SearchResult>, SearchError> {
        // Build matcher
        let matcher = Self::build_matcher(&options.search)?;

        // Determine which sheets to search
        let sheet_indices: Vec<usize> = match &options.search.sheet_indices {
            Some(indices) => {
                // Validate indices
                for &idx in indices {
                    if idx >= workbook.sheets.len() {
                        return Err(SearchError::InvalidSheetIndex(idx));
                    }
                }
                indices.clone()
            }
            None => (0..workbook.sheets.len()).collect(),
        };

        let mut results = Vec::new();

        // Process each sheet
        for &sheet_index in &sheet_indices {
            let sheet = &mut workbook.sheets[sheet_index];
            Self::replace_in_sheet(sheet, sheet_index, matcher.as_ref(), options, &mut results)?;
        }

        Ok(results)
    }

    /// Build a matcher from search options
    fn build_matcher(options: &SearchOptions) -> Result<Box<dyn Matcher>, SearchError> {
        if options.use_regex {
            let regex = if options.match_case {
                Regex::new(&options.query)
            } else {
                Regex::new(&format!("(?i){}", options.query))
            };

            match regex {
                Ok(r) => Ok(Box::new(RegexMatcher {
                    regex: r,
                    match_entire: options.match_entire_cell,
                })),
                Err(e) => Err(SearchError::InvalidRegex(e.to_string())),
            }
        } else {
            Ok(Box::new(LiteralMatcher {
                query: options.query.clone(),
                match_case: options.match_case,
                match_entire: options.match_entire_cell,
            }))
        }
    }

    /// Search a single sheet for matches
    fn search_sheet(
        sheet: &Sheet,
        sheet_index: usize,
        matcher: &dyn Matcher,
        options: &SearchOptions,
        results: &mut Vec<SearchResult>,
    ) {
        // Iterate through all non-empty cells
        for coord in sheet.non_empty_coords() {
            if let Some(cell) = sheet.get_cell(coord) {
                let search_text = Self::get_search_text(&cell.content, options.search_formulas);

                if let Some(matched_text) = matcher.find_match(&search_text) {
                    results.push(SearchResult {
                        sheet_name: sheet.name.clone(),
                        sheet_index,
                        row: coord.row,
                        col: coord.col,
                        matched_text,
                        cell_value: cell.content.display_value(),
                        is_formula: cell.content.is_formula(),
                    });
                }
            }
        }
    }

    /// Replace text in a single sheet
    fn replace_in_sheet(
        sheet: &mut Sheet,
        sheet_index: usize,
        matcher: &dyn Matcher,
        options: &ReplaceOptions,
        results: &mut Vec<SearchResult>,
    ) -> Result<(), SearchError> {
        // Collect coordinates to modify (can't modify while iterating)
        let mut to_modify = Vec::new();

        for coord in sheet.non_empty_coords() {
            if let Some(cell) = sheet.get_cell(coord) {
                // Don't allow replacing in formula cells
                if cell.content.is_formula() && !options.search.search_formulas {
                    continue;
                }

                // If searching formulas and this is a formula, skip
                // (we don't want to replace formula expressions, only values)
                if options.search.search_formulas && cell.content.is_formula() {
                    return Err(SearchError::CannotReplaceFormula);
                }

                let search_text =
                    Self::get_search_text(&cell.content, options.search.search_formulas);

                if let Some(matched_text) = matcher.find_match(&search_text) {
                    to_modify.push((coord, matched_text, search_text));
                }
            }
        }

        // Now perform the replacements
        for (coord, matched_text, search_text) in to_modify {
            let new_text = matcher.replace_all(&search_text, &options.replacement);

            // Update the cell
            sheet.set_cell_value(coord, &new_text);

            // Record the result
            let cell_value = sheet
                .get_cell(coord)
                .map(|c| c.content.display_value())
                .unwrap_or_default();

            results.push(SearchResult {
                sheet_name: sheet.name.clone(),
                sheet_index,
                row: coord.row,
                col: coord.col,
                matched_text,
                cell_value,
                is_formula: false, // After replacement, it's always a value
            });
        }

        Ok(())
    }

    /// Get the text to search based on options
    fn get_search_text(content: &CellContent, search_formulas: bool) -> String {
        if search_formulas {
            match content {
                CellContent::Formula { expression, .. } => expression.clone(),
                _ => content.display_value(),
            }
        } else {
            content.display_value()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Cell, CellCoord};

    fn create_test_workbook() -> Workbook {
        let mut workbook = Workbook::new("Test");

        // Add some test data to Sheet1
        let sheet = workbook.active_sheet_mut();
        sheet.set_cell(CellCoord::new(0, 0), Cell::text("Hello World"));
        sheet.set_cell(CellCoord::new(0, 1), Cell::text("hello world"));
        sheet.set_cell(CellCoord::new(1, 0), Cell::number(42.0));
        sheet.set_cell(CellCoord::new(1, 1), Cell::text("The answer is 42"));
        sheet.set_cell(CellCoord::new(2, 0), Cell::formula("=SUM(A1:A2)"));

        // Add a second sheet
        workbook.add_sheet("Sheet2").unwrap();
        let sheet2 = workbook.get_sheet_mut(1).unwrap();
        sheet2.set_cell(CellCoord::new(0, 0), Cell::text("Hello"));
        sheet2.set_cell(CellCoord::new(1, 0), Cell::text("World"));

        workbook
    }

    #[test]
    fn test_search_literal_case_sensitive() {
        let workbook = create_test_workbook();
        let options = SearchOptions {
            query: "Hello".to_string(),
            match_case: true,
            match_entire_cell: false,
            use_regex: false,
            search_formulas: false,
            sheet_indices: None,
        };

        let results = SearchEngine::search(&workbook, &options).unwrap();

        // Should find "Hello World" in Sheet1 and "Hello" in Sheet2
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].row, 0);
        assert_eq!(results[0].col, 0);
        assert_eq!(results[0].sheet_index, 0);
    }

    #[test]
    fn test_search_literal_case_insensitive() {
        let workbook = create_test_workbook();
        let options = SearchOptions {
            query: "hello".to_string(),
            match_case: false,
            match_entire_cell: false,
            use_regex: false,
            search_formulas: false,
            sheet_indices: None,
        };

        let results = SearchEngine::search(&workbook, &options).unwrap();

        // Should find both "Hello World" and "hello world" in Sheet1, and "Hello" in Sheet2
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_entire_cell() {
        let workbook = create_test_workbook();
        let options = SearchOptions {
            query: "Hello".to_string(),
            match_case: true,
            match_entire_cell: true,
            use_regex: false,
            search_formulas: false,
            sheet_indices: None,
        };

        let results = SearchEngine::search(&workbook, &options).unwrap();

        // Should only find exact "Hello" in Sheet2, not "Hello World"
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].row, 0);
        assert_eq!(results[0].col, 0);
        assert_eq!(results[0].sheet_index, 1);
    }

    #[test]
    fn test_search_regex() {
        let workbook = create_test_workbook();
        let options = SearchOptions {
            query: r"\d+".to_string(), // Match numbers
            match_case: false,
            match_entire_cell: false,
            use_regex: true,
            search_formulas: false,
            sheet_indices: None,
        };

        let results = SearchEngine::search(&workbook, &options).unwrap();

        // Should find "42" in cell A2 (number) and "42" in cell B2 (text)
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_formulas() {
        let workbook = create_test_workbook();
        let options = SearchOptions {
            query: "SUM".to_string(),
            match_case: false,
            match_entire_cell: false,
            use_regex: false,
            search_formulas: true,
            sheet_indices: None,
        };

        let results = SearchEngine::search(&workbook, &options).unwrap();

        // Should find the formula in A3
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].row, 2);
        assert_eq!(results[0].col, 0);
        assert!(results[0].is_formula);
    }

    #[test]
    fn test_search_specific_sheets() {
        let workbook = create_test_workbook();
        let options = SearchOptions {
            query: "Hello".to_string(),
            match_case: true,
            match_entire_cell: false,
            use_regex: false,
            search_formulas: false,
            sheet_indices: Some(vec![1]), // Only Sheet2
        };

        let results = SearchEngine::search(&workbook, &options).unwrap();

        // Should only find results in Sheet2
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].sheet_index, 1);
    }

    #[test]
    fn test_replace_literal() {
        let mut workbook = create_test_workbook();
        let options = ReplaceOptions {
            search: SearchOptions {
                query: "Hello".to_string(),
                match_case: true,
                match_entire_cell: false,
                use_regex: false,
                search_formulas: false,
                sheet_indices: None,
            },
            replacement: "Hi".to_string(),
        };

        let results = SearchEngine::replace(&mut workbook, &options).unwrap();

        // Should replace in both sheets
        assert_eq!(results.len(), 2);

        // Check that the replacement happened
        let cell = workbook.sheets[0].get_cell(CellCoord::new(0, 0)).unwrap();
        assert_eq!(cell.content.display_value(), "Hi World");

        let cell = workbook.sheets[1].get_cell(CellCoord::new(0, 0)).unwrap();
        assert_eq!(cell.content.display_value(), "Hi");
    }

    #[test]
    fn test_replace_regex() {
        let mut workbook = create_test_workbook();
        let options = ReplaceOptions {
            search: SearchOptions {
                query: r"\d+".to_string(),
                match_case: false,
                match_entire_cell: false,
                use_regex: true,
                search_formulas: false,
                sheet_indices: None,
            },
            replacement: "XX".to_string(),
        };

        let results = SearchEngine::replace(&mut workbook, &options).unwrap();

        // Should replace numbers
        assert_eq!(results.len(), 2);

        // Check replacements
        let cell = workbook.sheets[0].get_cell(CellCoord::new(1, 0)).unwrap();
        assert_eq!(cell.content.display_value(), "XX");

        let cell = workbook.sheets[0].get_cell(CellCoord::new(1, 1)).unwrap();
        assert_eq!(cell.content.display_value(), "The answer is XX");
    }

    #[test]
    fn test_replace_cannot_replace_formula() {
        let mut workbook = create_test_workbook();
        let options = ReplaceOptions {
            search: SearchOptions {
                query: "SUM".to_string(),
                match_case: false,
                match_entire_cell: false,
                use_regex: false,
                search_formulas: true,
                sheet_indices: None,
            },
            replacement: "AVERAGE".to_string(),
        };

        let result = SearchEngine::replace(&mut workbook, &options);

        // Should return error
        assert!(matches!(result, Err(SearchError::CannotReplaceFormula)));
    }

    #[test]
    fn test_replace_case_insensitive() {
        let mut workbook = create_test_workbook();
        let options = ReplaceOptions {
            search: SearchOptions {
                query: "world".to_string(),
                match_case: false,
                match_entire_cell: false,
                use_regex: false,
                search_formulas: false,
                sheet_indices: None,
            },
            replacement: "Earth".to_string(),
        };

        let results = SearchEngine::replace(&mut workbook, &options).unwrap();

        // Should replace both "World" and "world"
        assert!(results.len() >= 2);

        let cell = workbook.sheets[0].get_cell(CellCoord::new(0, 0)).unwrap();
        assert_eq!(cell.content.display_value(), "Hello Earth");

        let cell = workbook.sheets[0].get_cell(CellCoord::new(0, 1)).unwrap();
        assert_eq!(cell.content.display_value(), "hello Earth");
    }

    #[test]
    fn test_invalid_regex() {
        let workbook = create_test_workbook();
        let options = SearchOptions {
            query: "[invalid".to_string(), // Invalid regex
            match_case: false,
            match_entire_cell: false,
            use_regex: true,
            search_formulas: false,
            sheet_indices: None,
        };

        let result = SearchEngine::search(&workbook, &options);
        assert!(matches!(result, Err(SearchError::InvalidRegex(_))));
    }

    #[test]
    fn test_invalid_sheet_index() {
        let workbook = create_test_workbook();
        let options = SearchOptions {
            query: "test".to_string(),
            match_case: false,
            match_entire_cell: false,
            use_regex: false,
            search_formulas: false,
            sheet_indices: Some(vec![99]), // Invalid index
        };

        let result = SearchEngine::search(&workbook, &options);
        assert!(matches!(result, Err(SearchError::InvalidSheetIndex(99))));
    }
}
