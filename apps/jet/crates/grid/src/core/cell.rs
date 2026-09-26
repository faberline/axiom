use serde::{Deserialize, Serialize};

use crate::core::error::CellError;
use crate::core::format::CellFormat;

/// Represents the raw value stored in a cell
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum CellValue {
    #[default]
    Empty,
    Number(f64),
    Text(String),
    Boolean(bool),
    Error(CellError),
}

impl CellValue {
    /// Check if the value is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, CellValue::Empty)
    }

    /// Try to get the value as a number
    pub fn as_number(&self) -> Option<f64> {
        match self {
            CellValue::Number(n) => Some(*n),
            CellValue::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
            CellValue::Text(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// Try to get the value as a string
    pub fn as_text(&self) -> String {
        match self {
            CellValue::Empty => String::new(),
            CellValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            CellValue::Text(s) => s.clone(),
            CellValue::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            CellValue::Error(e) => e.to_string(),
        }
    }

    /// Try to get the value as a boolean
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            CellValue::Boolean(b) => Some(*b),
            CellValue::Number(n) => Some(*n != 0.0),
            CellValue::Text(s) => match s.to_uppercase().as_str() {
                "TRUE" | "YES" | "1" => Some(true),
                "FALSE" | "NO" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }
}

/// The content of a cell - either a raw value or a formula
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CellContent {
    #[serde(rename = "Value")]
    Value {
        #[serde(rename = "value")]
        value: CellValue,
        #[serde(rename = "originalInput", skip_serializing_if = "Option::is_none")]
        original_input: Option<String>,
    },
    #[serde(rename = "Formula")]
    Formula {
        #[serde(rename = "expression")]
        /// Original formula text (e.g., "=SUM(A1:A10)")
        expression: String,
        #[serde(rename = "cachedValue")]
        /// Computed/cached value
        cached_value: CellValue,
    },
}

impl Default for CellContent {
    fn default() -> Self {
        CellContent::Value {
            value: CellValue::Empty,
            original_input: None,
        }
    }
}

impl CellContent {
    /// Create a new formula content
    pub fn formula(expression: String) -> Self {
        CellContent::Formula {
            expression,
            cached_value: CellValue::Empty,
        }
    }

    /// Get the computed value (for both value and formula)
    pub fn computed_value(&self) -> &CellValue {
        match self {
            CellContent::Value { value, .. } => value,
            CellContent::Formula { cached_value, .. } => cached_value,
        }
    }

    /// Check if this is a formula
    pub fn is_formula(&self) -> bool {
        matches!(self, CellContent::Formula { .. })
    }

    /// Get the formula expression if this is a formula
    pub fn formula_expression(&self) -> Option<&str> {
        match self {
            CellContent::Formula { expression, .. } => Some(expression),
            _ => None,
        }
    }

    /// Get the display value as a string
    pub fn display_value(&self) -> String {
        self.computed_value().as_text()
    }

    /// Get the original user input string for editing
    pub fn original_input(&self) -> String {
        match self {
            CellContent::Value {
                original_input,
                value,
            } => original_input.clone().unwrap_or_else(|| value.as_text()),
            CellContent::Formula { expression, .. } => expression.clone(),
        }
    }

    /// Check if this content is empty (empty value, not a formula)
    pub fn is_empty(&self) -> bool {
        matches!(
            self,
            CellContent::Value {
                value: CellValue::Empty,
                ..
            }
        )
    }
}

/// Complete cell data structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cell {
    pub content: CellContent,
    #[serde(default)]
    pub format: CellFormat,
}

impl Cell {
    /// Create a new cell with a value
    pub fn new(content: CellContent) -> Self {
        Cell {
            content,
            format: CellFormat::default(),
        }
    }

    /// Create a cell with a number value
    pub fn number(value: f64) -> Self {
        Cell::new(CellContent::Value {
            value: CellValue::Number(value),
            original_input: None,
        })
    }

    /// Create a cell with a text value
    pub fn text(value: impl Into<String>) -> Self {
        Cell::new(CellContent::Value {
            value: CellValue::Text(value.into()),
            original_input: None,
        })
    }

    /// Create a cell with a boolean value
    pub fn boolean(value: bool) -> Self {
        Cell::new(CellContent::Value {
            value: CellValue::Boolean(value),
            original_input: None,
        })
    }

    /// Create a cell with a formula
    pub fn formula(expression: impl Into<String>) -> Self {
        Cell::new(CellContent::formula(expression.into()))
    }

    /// Get the computed value of the cell
    pub fn computed_value(&self) -> &CellValue {
        self.content.computed_value()
    }

    /// Check if the cell is empty (empty value and default format)
    pub fn is_empty(&self) -> bool {
        matches!(
            self.content,
            CellContent::Value {
                value: CellValue::Empty,
                ..
            }
        ) && self.format == CellFormat::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_value_as_number() {
        assert_eq!(CellValue::Number(42.0).as_number(), Some(42.0));
        assert_eq!(CellValue::Boolean(true).as_number(), Some(1.0));
        assert_eq!(CellValue::Text("123".to_string()).as_number(), Some(123.0));
        assert_eq!(CellValue::Empty.as_number(), None);
    }

    #[test]
    fn test_cell_value_as_text() {
        assert_eq!(CellValue::Number(42.0).as_text(), "42");
        assert_eq!(CellValue::Number(42.5).as_text(), "42.5");
        assert_eq!(CellValue::Boolean(true).as_text(), "TRUE");
        assert_eq!(CellValue::Text("hello".to_string()).as_text(), "hello");
    }

    #[test]
    fn test_cell_creation() {
        let cell = Cell::number(42.0);
        assert_eq!(cell.computed_value().as_number(), Some(42.0));

        let cell = Cell::text("hello");
        assert_eq!(cell.computed_value().as_text(), "hello");

        let cell = Cell::formula("=A1+B1");
        assert!(cell.content.is_formula());
        assert_eq!(cell.content.formula_expression(), Some("=A1+B1"));
    }
}
