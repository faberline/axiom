use crate::core::{CellFormat, CellRange, CellValue, Color};
use serde::{Deserialize, Serialize};

/// Comparison operators for value-based rules
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOperator {
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Equal,
    NotEqual,
    Between,
    NotBetween,
}

/// Text match operators
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TextOperator {
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    IsEmpty,
    IsNotEmpty,
}

/// Format to apply when rule matches
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ConditionalFormat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<Color>,
}

impl ConditionalFormat {
    /// Apply this conditional format to an existing CellFormat
    pub fn apply_to(&self, base: &CellFormat) -> CellFormat {
        let mut result = base.clone();
        if let Some(bold) = self.bold {
            result.bold = bold;
        }
        if let Some(italic) = self.italic {
            result.italic = italic;
        }
        if let Some(underline) = self.underline {
            result.underline = underline;
        }
        if let Some(ref color) = self.text_color {
            result.text_color = Some(*color);
        }
        if let Some(ref color) = self.background_color {
            result.background_color = Some(*color);
        }
        result
    }
}

/// Types of conditional formatting rules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConditionalRule {
    /// Compare cell value against thresholds
    ValueBased {
        operator: ComparisonOperator,
        value1: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        value2: Option<f64>, // For Between/NotBetween
        format: ConditionalFormat,
    },

    /// Match cell text against patterns
    TextBased {
        operator: TextOperator,
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
        #[serde(default)]
        case_sensitive: bool,
        format: ConditionalFormat,
    },

    /// Color scale based on value range in the data
    ColorScale {
        min_color: Color,
        max_color: Color,
        #[serde(skip_serializing_if = "Option::is_none")]
        mid_color: Option<Color>,
    },
}

impl ConditionalRule {
    /// Evaluate if this rule matches the given cell value
    /// Returns the format to apply, or None if rule doesn't match
    pub fn evaluate(
        &self,
        value: &CellValue,
        min_val: f64,
        max_val: f64,
    ) -> Option<ConditionalFormat> {
        match self {
            ConditionalRule::ValueBased {
                operator,
                value1,
                value2,
                format,
            } => {
                let num = match value {
                    CellValue::Number(n) => *n,
                    _ => return None,
                };

                let matches = match operator {
                    ComparisonOperator::GreaterThan => num > *value1,
                    ComparisonOperator::GreaterThanOrEqual => num >= *value1,
                    ComparisonOperator::LessThan => num < *value1,
                    ComparisonOperator::LessThanOrEqual => num <= *value1,
                    ComparisonOperator::Equal => (num - value1).abs() < f64::EPSILON,
                    ComparisonOperator::NotEqual => (num - value1).abs() >= f64::EPSILON,
                    ComparisonOperator::Between => {
                        if let Some(v2) = value2 {
                            num >= *value1 && num <= *v2
                        } else {
                            false
                        }
                    }
                    ComparisonOperator::NotBetween => {
                        if let Some(v2) = value2 {
                            num < *value1 || num > *v2
                        } else {
                            false
                        }
                    }
                };

                if matches {
                    Some(format.clone())
                } else {
                    None
                }
            }

            ConditionalRule::TextBased {
                operator,
                pattern,
                case_sensitive,
                format,
            } => {
                let text = match value {
                    CellValue::Text(s) => s.as_str(),
                    CellValue::Number(_) => return None, // Text rules don't apply to numbers
                    _ => return None,
                };

                let (text, pattern) = if *case_sensitive {
                    (text.to_string(), pattern.clone().unwrap_or_default())
                } else {
                    (
                        text.to_lowercase(),
                        pattern.clone().unwrap_or_default().to_lowercase(),
                    )
                };

                let matches = match operator {
                    TextOperator::Contains => text.contains(&pattern),
                    TextOperator::NotContains => !text.contains(&pattern),
                    TextOperator::StartsWith => text.starts_with(&pattern),
                    TextOperator::EndsWith => text.ends_with(&pattern),
                    TextOperator::IsEmpty => text.is_empty(),
                    TextOperator::IsNotEmpty => !text.is_empty(),
                };

                if matches {
                    Some(format.clone())
                } else {
                    None
                }
            }

            ConditionalRule::ColorScale {
                min_color,
                max_color,
                mid_color,
            } => {
                let num = match value {
                    CellValue::Number(n) => *n,
                    _ => return None,
                };

                if max_val <= min_val {
                    return Some(ConditionalFormat {
                        background_color: Some(*min_color),
                        ..Default::default()
                    });
                }

                let ratio = ((num - min_val) / (max_val - min_val)).clamp(0.0, 1.0);

                let color = if let Some(mid) = mid_color {
                    // 3-color scale
                    if ratio < 0.5 {
                        interpolate_color(min_color, mid, ratio * 2.0)
                    } else {
                        interpolate_color(mid, max_color, (ratio - 0.5) * 2.0)
                    }
                } else {
                    // 2-color scale
                    interpolate_color(min_color, max_color, ratio)
                };

                Some(ConditionalFormat {
                    background_color: Some(color),
                    ..Default::default()
                })
            }
        }
    }
}

/// Interpolate between two colors
fn interpolate_color(c1: &Color, c2: &Color, ratio: f64) -> Color {
    let ratio = ratio.clamp(0.0, 1.0);
    Color {
        r: (c1.r as f64 + (c2.r as f64 - c1.r as f64) * ratio) as u8,
        g: (c1.g as f64 + (c2.g as f64 - c1.g as f64) * ratio) as u8,
        b: (c1.b as f64 + (c2.b as f64 - c1.b as f64) * ratio) as u8,
        a: (c1.a as f64 + (c2.a as f64 - c1.a as f64) * ratio) as u8,
    }
}

/// A complete conditional formatting rule with range and priority
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConditionalFormattingRule {
    pub id: String,
    pub range: CellRange,
    pub rule: ConditionalRule,
    #[serde(default)]
    pub priority: i32, // Higher = applied later (overrides)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl ConditionalFormattingRule {
    pub fn new(id: String, range: CellRange, rule: ConditionalRule) -> Self {
        Self {
            id,
            range,
            rule,
            priority: 0,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::CellCoord;

    #[test]
    fn test_value_based_greater_than() {
        let rule = ConditionalRule::ValueBased {
            operator: ComparisonOperator::GreaterThan,
            value1: 50.0,
            value2: None,
            format: ConditionalFormat {
                background_color: Some(Color::RED),
                ..Default::default()
            },
        };

        assert!(rule
            .evaluate(&CellValue::Number(100.0), 0.0, 100.0)
            .is_some());
        assert!(rule
            .evaluate(&CellValue::Number(30.0), 0.0, 100.0)
            .is_none());
        assert!(rule
            .evaluate(&CellValue::Text("hello".into()), 0.0, 100.0)
            .is_none());
    }

    #[test]
    fn test_value_based_between() {
        let rule = ConditionalRule::ValueBased {
            operator: ComparisonOperator::Between,
            value1: 10.0,
            value2: Some(20.0),
            format: ConditionalFormat {
                bold: Some(true),
                ..Default::default()
            },
        };

        assert!(rule
            .evaluate(&CellValue::Number(15.0), 0.0, 100.0)
            .is_some());
        assert!(rule
            .evaluate(&CellValue::Number(10.0), 0.0, 100.0)
            .is_some());
        assert!(rule.evaluate(&CellValue::Number(5.0), 0.0, 100.0).is_none());
    }

    #[test]
    fn test_text_contains() {
        let rule = ConditionalRule::TextBased {
            operator: TextOperator::Contains,
            pattern: Some("error".into()),
            case_sensitive: false,
            format: ConditionalFormat {
                text_color: Some(Color::RED),
                ..Default::default()
            },
        };

        assert!(rule
            .evaluate(&CellValue::Text("Error occurred".into()), 0.0, 0.0)
            .is_some());
        assert!(rule
            .evaluate(&CellValue::Text("success".into()), 0.0, 0.0)
            .is_none());
    }

    #[test]
    fn test_color_scale() {
        let rule = ConditionalRule::ColorScale {
            min_color: Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            max_color: Color {
                r: 0,
                g: 255,
                b: 0,
                a: 255,
            },
            mid_color: None,
        };

        let fmt = rule.evaluate(&CellValue::Number(50.0), 0.0, 100.0).unwrap();
        let color = fmt.background_color.unwrap();
        // At 50%, should be yellowish (mix of red and green)
        assert!(color.r > 100 && color.r < 150);
        assert!(color.g > 100 && color.g < 150);
    }

    #[test]
    fn test_interpolate_color() {
        let red = Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        };
        let green = Color {
            r: 0,
            g: 255,
            b: 0,
            a: 255,
        };

        let mid = interpolate_color(&red, &green, 0.5);
        assert_eq!(mid.r, 127);
        assert_eq!(mid.g, 127);
    }

    #[test]
    fn test_range_contains() {
        let range = CellRange::new(CellCoord::new(1, 1), CellCoord::new(5, 5));
        assert!(range.contains(CellCoord::new(1, 1)));
        assert!(range.contains(CellCoord::new(3, 3)));
        assert!(range.contains(CellCoord::new(5, 5)));
        assert!(!range.contains(CellCoord::new(0, 0)));
        assert!(!range.contains(CellCoord::new(6, 3)));
    }
}
