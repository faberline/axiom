use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents possible cell errors (Excel-compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellError {
    /// #DIV/0! - Division by zero
    DivisionByZero,
    /// #VALUE! - Invalid value type
    InvalidValue,
    /// #REF! - Invalid cell reference
    InvalidReference,
    /// #NAME? - Unrecognized function or name
    InvalidName,
    /// #NULL! - Null intersection
    NullError,
    /// #NUM! - Invalid numeric value
    NumError,
    /// #N/A - Value not available
    NotAvailable,
    /// Circular reference detected
    CircularReference,
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellError::DivisionByZero => write!(f, "#DIV/0!"),
            CellError::InvalidValue => write!(f, "#VALUE!"),
            CellError::InvalidReference => write!(f, "#REF!"),
            CellError::InvalidName => write!(f, "#NAME?"),
            CellError::NullError => write!(f, "#NULL!"),
            CellError::NumError => write!(f, "#NUM!"),
            CellError::NotAvailable => write!(f, "#N/A"),
            CellError::CircularReference => write!(f, "#CIRCULAR!"),
        }
    }
}

/// Represents application-level errors
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RusheetError {
    /// Sheet not found at index
    SheetNotFound(usize),
    /// Sheet name already exists
    SheetNameExists(String),
    /// Invalid sheet name (empty or restricted chars)
    InvalidSheetName(String),
    /// Cannot delete the last visible sheet
    CannotDeleteLastSheet,
    /// Invalid cell coordinates
    InvalidCoordinates(u32, u32),
    /// Range is out of bounds or invalid
    RangeOutOfBounds,
    /// Merge operation overlaps with existing merges
    MergeOverlap,
    /// Attempting to unmerge a cell that isn't merged
    UnmergeNotMerged,
    /// Generic error with message
    Generic(String),
}

impl fmt::Display for RusheetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RusheetError::SheetNotFound(idx) => write!(f, "Sheet not found at index {}", idx),
            RusheetError::SheetNameExists(name) => {
                write!(f, "Sheet name '{}' already exists", name)
            }
            RusheetError::InvalidSheetName(name) => write!(f, "Invalid sheet name: '{}'", name),
            RusheetError::CannotDeleteLastSheet => write!(f, "Cannot delete the last sheet"),
            RusheetError::InvalidCoordinates(r, c) => {
                write!(f, "Invalid coordinates: ({}, {})", r, c)
            }
            RusheetError::RangeOutOfBounds => write!(f, "Range out of bounds"),
            RusheetError::MergeOverlap => write!(f, "Merge range overlaps with existing merges"),
            RusheetError::UnmergeNotMerged => write!(f, "Cell is not merged"),
            RusheetError::Generic(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for RusheetError {}

impl RusheetError {
    pub fn code(&self) -> &'static str {
        match self {
            RusheetError::SheetNotFound(_) => "SHEET_NOT_FOUND",
            RusheetError::SheetNameExists(_) => "SHEET_NAME_EXISTS",
            RusheetError::InvalidSheetName(_) => "INVALID_SHEET_NAME",
            RusheetError::CannotDeleteLastSheet => "CANNOT_DELETE_LAST_SHEET",
            RusheetError::InvalidCoordinates(_, _) => "INVALID_COORDINATES",
            RusheetError::RangeOutOfBounds => "RANGE_OUT_OF_BOUNDS",
            RusheetError::MergeOverlap => "MERGE_OVERLAP",
            RusheetError::UnmergeNotMerged => "UNMERGE_NOT_MERGED",
            RusheetError::Generic(_) => "GENERIC_ERROR",
        }
    }
}
