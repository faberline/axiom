use crate::core::{CellRange, CellValue};
use serde::{Deserialize, Serialize};

/// Comparison operators for validation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationOperator {
    Between,
    NotBetween,
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
}

/// Source for dropdown list values
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ListSource {
    /// Static list of values
    Values { items: Vec<String> },
    /// Reference to a cell range (stored as string like "A1:A10" or "Sheet2!B1:B20")
    Range { reference: String },
}

/// Validation criteria types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidationCriteria {
    /// Dropdown list
    List {
        source: ListSource,
        #[serde(default = "default_true")]
        show_dropdown: bool,
    },

    /// Whole number validation
    WholeNumber {
        operator: ValidationOperator,
        value1: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        value2: Option<i64>,
    },

    /// Decimal number validation
    Decimal {
        operator: ValidationOperator,
        value1: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        value2: Option<f64>,
    },

    /// Text length validation
    TextLength {
        operator: ValidationOperator,
        value1: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        value2: Option<usize>,
    },

    /// Date validation (Unix timestamps)
    Date {
        operator: ValidationOperator,
        value1: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        value2: Option<i64>,
    },

    /// Custom formula (must evaluate to TRUE)
    Custom { formula: String },

    /// Any value allowed (essentially disables validation but keeps messages)
    Any,
}

fn default_true() -> bool {
    true
}

/// Alert style when validation fails
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AlertStyle {
    /// Prevents invalid entry (default)
    #[default]
    Stop,
    /// Shows warning but allows entry
    Warning,
    /// Shows information but allows entry
    Information,
}

/// Message to show to user
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ValidationMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Error alert configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationAlert {
    pub style: AlertStyle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Default for ValidationAlert {
    fn default() -> Self {
        Self {
            style: AlertStyle::Stop,
            title: None,
            message: None,
        }
    }
}

/// Result of validation
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationResult {
    Valid,
    Invalid(ValidationAlert),
    /// Used when validation depends on external data (like formula or range reference)
    NeedsContext,
}

/// A complete data validation rule
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataValidationRule {
    pub id: String,
    pub range: CellRange,
    pub criteria: ValidationCriteria,
    #[serde(default)]
    pub allow_blank: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_message: Option<ValidationMessage>,
    #[serde(default)]
    pub error_alert: ValidationAlert,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl DataValidationRule {
    pub fn new(id: String, range: CellRange, criteria: ValidationCriteria) -> Self {
        Self {
            id,
            range,
            criteria,
            allow_blank: true,
            input_message: None,
            error_alert: ValidationAlert::default(),
            enabled: true,
        }
    }

    /// Validate a cell value against this rule
    /// For List with Range source or Custom formula, returns NeedsContext
    pub fn validate(&self, value: &CellValue) -> ValidationResult {
        if !self.enabled {
            return ValidationResult::Valid;
        }

        // Handle blank values
        match value {
            CellValue::Empty => {
                if self.allow_blank {
                    return ValidationResult::Valid;
                } else {
                    return ValidationResult::Invalid(self.error_alert.clone());
                }
            }
            CellValue::Text(s) if s.is_empty() => {
                if self.allow_blank {
                    return ValidationResult::Valid;
                } else {
                    return ValidationResult::Invalid(self.error_alert.clone());
                }
            }
            _ => {}
        }

        let is_valid = match &self.criteria {
            ValidationCriteria::Any => true,

            ValidationCriteria::List { source, .. } => {
                match source {
                    ListSource::Values { items } => {
                        let text = value.as_text();
                        items.iter().any(|item| item == &text)
                    }
                    ListSource::Range { .. } => {
                        // Can't validate without sheet context
                        return ValidationResult::NeedsContext;
                    }
                }
            }

            ValidationCriteria::WholeNumber {
                operator,
                value1,
                value2,
            } => match value {
                CellValue::Number(n) => {
                    let n = *n as i64;
                    Self::check_operator(*operator, n, *value1, *value2)
                }
                _ => false,
            },

            ValidationCriteria::Decimal {
                operator,
                value1,
                value2,
            } => match value {
                CellValue::Number(n) => Self::check_operator_f64(*operator, *n, *value1, *value2),
                _ => false,
            },

            ValidationCriteria::TextLength {
                operator,
                value1,
                value2,
            } => {
                let len = match value {
                    CellValue::Text(s) => s.len(),
                    CellValue::Number(n) => n.to_string().len(),
                    _ => 0,
                };
                Self::check_operator(
                    *operator,
                    len as i64,
                    *value1 as i64,
                    value2.map(|v| v as i64),
                )
            }

            ValidationCriteria::Date {
                operator,
                value1,
                value2,
            } => {
                // For date validation, the cell should contain a number representing days since epoch
                match value {
                    CellValue::Number(n) => {
                        Self::check_operator(*operator, *n as i64, *value1, *value2)
                    }
                    _ => false,
                }
            }

            ValidationCriteria::Custom { .. } => {
                return ValidationResult::NeedsContext;
            }
        };

        if is_valid {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid(self.error_alert.clone())
        }
    }

    fn check_operator<T: Ord>(
        operator: ValidationOperator,
        value: T,
        v1: T,
        v2: Option<T>,
    ) -> bool {
        match operator {
            ValidationOperator::Equal => value == v1,
            ValidationOperator::NotEqual => value != v1,
            ValidationOperator::GreaterThan => value > v1,
            ValidationOperator::GreaterThanOrEqual => value >= v1,
            ValidationOperator::LessThan => value < v1,
            ValidationOperator::LessThanOrEqual => value <= v1,
            ValidationOperator::Between => {
                if let Some(v2) = v2 {
                    value >= v1 && value <= v2
                } else {
                    false
                }
            }
            ValidationOperator::NotBetween => {
                if let Some(v2) = v2 {
                    value < v1 || value > v2
                } else {
                    false
                }
            }
        }
    }

    fn check_operator_f64(
        operator: ValidationOperator,
        value: f64,
        v1: f64,
        v2: Option<f64>,
    ) -> bool {
        match operator {
            ValidationOperator::Equal => (value - v1).abs() < f64::EPSILON,
            ValidationOperator::NotEqual => (value - v1).abs() >= f64::EPSILON,
            ValidationOperator::GreaterThan => value > v1,
            ValidationOperator::GreaterThanOrEqual => value >= v1,
            ValidationOperator::LessThan => value < v1,
            ValidationOperator::LessThanOrEqual => value <= v1,
            ValidationOperator::Between => {
                if let Some(v2) = v2 {
                    value >= v1 && value <= v2
                } else {
                    false
                }
            }
            ValidationOperator::NotBetween => {
                if let Some(v2) = v2 {
                    value < v1 || value > v2
                } else {
                    false
                }
            }
        }
    }

    /// Get dropdown items for List validation (static values only)
    pub fn get_dropdown_items(&self) -> Option<Vec<String>> {
        match &self.criteria {
            ValidationCriteria::List {
                source: ListSource::Values { items },
                show_dropdown,
            } if *show_dropdown => Some(items.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::CellCoord;

    #[test]
    fn test_list_validation_static() {
        let rule = DataValidationRule::new(
            "test".into(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            ValidationCriteria::List {
                source: ListSource::Values {
                    items: vec!["Yes".into(), "No".into(), "Maybe".into()],
                },
                show_dropdown: true,
            },
        );

        assert_eq!(
            rule.validate(&CellValue::Text("Yes".into())),
            ValidationResult::Valid
        );
        assert_eq!(
            rule.validate(&CellValue::Text("No".into())),
            ValidationResult::Valid
        );
        assert!(matches!(
            rule.validate(&CellValue::Text("Invalid".into())),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_whole_number_between() {
        let rule = DataValidationRule::new(
            "test".into(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            ValidationCriteria::WholeNumber {
                operator: ValidationOperator::Between,
                value1: 1,
                value2: Some(100),
            },
        );

        assert_eq!(
            rule.validate(&CellValue::Number(50.0)),
            ValidationResult::Valid
        );
        assert_eq!(
            rule.validate(&CellValue::Number(1.0)),
            ValidationResult::Valid
        );
        assert_eq!(
            rule.validate(&CellValue::Number(100.0)),
            ValidationResult::Valid
        );
        assert!(matches!(
            rule.validate(&CellValue::Number(0.0)),
            ValidationResult::Invalid(_)
        ));
        assert!(matches!(
            rule.validate(&CellValue::Number(101.0)),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_text_length() {
        let rule = DataValidationRule::new(
            "test".into(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            ValidationCriteria::TextLength {
                operator: ValidationOperator::LessThanOrEqual,
                value1: 10,
                value2: None,
            },
        );

        assert_eq!(
            rule.validate(&CellValue::Text("short".into())),
            ValidationResult::Valid
        );
        assert!(matches!(
            rule.validate(&CellValue::Text("this is way too long".into())),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_allow_blank() {
        let mut rule = DataValidationRule::new(
            "test".into(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            ValidationCriteria::WholeNumber {
                operator: ValidationOperator::GreaterThan,
                value1: 0,
                value2: None,
            },
        );

        // Allow blank by default
        assert_eq!(rule.validate(&CellValue::Empty), ValidationResult::Valid);

        // Disallow blank
        rule.allow_blank = false;
        assert!(matches!(
            rule.validate(&CellValue::Empty),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_dropdown_items() {
        let rule = DataValidationRule::new(
            "test".into(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            ValidationCriteria::List {
                source: ListSource::Values {
                    items: vec!["A".into(), "B".into(), "C".into()],
                },
                show_dropdown: true,
            },
        );

        let items = rule.get_dropdown_items();
        assert_eq!(items, Some(vec!["A".into(), "B".into(), "C".into()]));
    }

    #[test]
    fn test_decimal_validation() {
        let rule = DataValidationRule::new(
            "test".into(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            ValidationCriteria::Decimal {
                operator: ValidationOperator::Between,
                value1: 0.0,
                value2: Some(1.0),
            },
        );

        assert_eq!(
            rule.validate(&CellValue::Number(0.5)),
            ValidationResult::Valid
        );
        assert_eq!(
            rule.validate(&CellValue::Number(0.0)),
            ValidationResult::Valid
        );
        assert_eq!(
            rule.validate(&CellValue::Number(1.0)),
            ValidationResult::Valid
        );
        assert!(matches!(
            rule.validate(&CellValue::Number(1.1)),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn test_list_validation_range_needs_context() {
        let rule = DataValidationRule::new(
            "test".into(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            ValidationCriteria::List {
                source: ListSource::Range {
                    reference: "A1:A10".into(),
                },
                show_dropdown: true,
            },
        );

        // Range-based validation needs context
        assert_eq!(
            rule.validate(&CellValue::Text("test".into())),
            ValidationResult::NeedsContext
        );
    }

    #[test]
    fn test_custom_formula_needs_context() {
        let rule = DataValidationRule::new(
            "test".into(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            ValidationCriteria::Custom {
                formula: "=A1>0".into(),
            },
        );

        // Custom formula validation needs context
        assert_eq!(
            rule.validate(&CellValue::Number(5.0)),
            ValidationResult::NeedsContext
        );
    }

    #[test]
    fn test_date_validation() {
        let rule = DataValidationRule::new(
            "test".into(),
            CellRange::new(CellCoord::new(0, 0), CellCoord::new(10, 0)),
            ValidationCriteria::Date {
                operator: ValidationOperator::GreaterThan,
                value1: 18000, // ~49 years from epoch
                value2: None,
            },
        );

        assert_eq!(
            rule.validate(&CellValue::Number(20000.0)),
            ValidationResult::Valid
        );
        assert!(matches!(
            rule.validate(&CellValue::Number(10000.0)),
            ValidationResult::Invalid(_)
        ));
    }
}
