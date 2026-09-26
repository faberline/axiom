use crate::core::{CellError, CellValue};

/// SUM - Sum all numeric values
pub fn sum(values: &[CellValue]) -> CellValue {
    let mut total = 0.0;

    for value in values {
        match value {
            CellValue::Number(n) => total += n,
            CellValue::Boolean(b) => total += if *b { 1.0 } else { 0.0 },
            CellValue::Error(e) => return CellValue::Error(e.clone()),
            _ => {} // Skip empty and text
        }
    }

    CellValue::Number(total)
}

/// AVERAGE - Average of numeric values
pub fn average(values: &[CellValue]) -> CellValue {
    let mut total = 0.0;
    let mut count = 0;

    for value in values {
        match value {
            CellValue::Number(n) => {
                total += n;
                count += 1;
            }
            CellValue::Error(e) => return CellValue::Error(e.clone()),
            _ => {} // Skip empty, text, boolean for AVERAGE
        }
    }

    if count == 0 {
        CellValue::Error(CellError::DivisionByZero)
    } else {
        CellValue::Number(total / count as f64)
    }
}

/// COUNT - Count numeric values
pub fn count(values: &[CellValue]) -> CellValue {
    let count = values
        .iter()
        .filter(|v| matches!(v, CellValue::Number(_)))
        .count();

    CellValue::Number(count as f64)
}

/// COUNTA - Count non-empty values
pub fn counta(values: &[CellValue]) -> CellValue {
    let count = values
        .iter()
        .filter(|v| !matches!(v, CellValue::Empty))
        .count();

    CellValue::Number(count as f64)
}

/// MIN - Minimum numeric value
pub fn min(values: &[CellValue]) -> CellValue {
    let mut result: Option<f64> = None;

    for value in values {
        match value {
            CellValue::Number(n) => {
                result = Some(result.map_or(*n, |r| r.min(*n)));
            }
            CellValue::Error(e) => return CellValue::Error(e.clone()),
            _ => {}
        }
    }

    match result {
        Some(n) => CellValue::Number(n),
        None => CellValue::Number(0.0),
    }
}

/// MAX - Maximum numeric value
pub fn max(values: &[CellValue]) -> CellValue {
    let mut result: Option<f64> = None;

    for value in values {
        match value {
            CellValue::Number(n) => {
                result = Some(result.map_or(*n, |r| r.max(*n)));
            }
            CellValue::Error(e) => return CellValue::Error(e.clone()),
            _ => {}
        }
    }

    match result {
        Some(n) => CellValue::Number(n),
        None => CellValue::Number(0.0),
    }
}

/// ABS - Absolute value
pub fn abs(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    match &values[0] {
        CellValue::Number(n) => CellValue::Number(n.abs()),
        CellValue::Error(e) => CellValue::Error(e.clone()),
        _ => CellValue::Error(CellError::InvalidValue),
    }
}

/// ROUND - Round to specified decimal places
pub fn round(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    let num = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let decimals = if values.len() > 1 {
        values[1].as_number().unwrap_or(0.0) as i32
    } else {
        0
    };

    let factor = 10_f64.powi(decimals);
    CellValue::Number((num * factor).round() / factor)
}

/// FLOOR - Round down to specified decimal places
pub fn floor(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    let num = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let significance = if values.len() > 1 {
        values[1].as_number().unwrap_or(1.0)
    } else {
        1.0
    };

    if significance == 0.0 {
        return CellValue::Error(CellError::DivisionByZero);
    }

    CellValue::Number((num / significance).floor() * significance)
}

/// CEILING - Round up to specified decimal places
pub fn ceiling(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    let num = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let significance = if values.len() > 1 {
        values[1].as_number().unwrap_or(1.0)
    } else {
        1.0
    };

    if significance == 0.0 {
        return CellValue::Error(CellError::DivisionByZero);
    }

    CellValue::Number((num / significance).ceil() * significance)
}

/// SQRT - Square root
pub fn sqrt(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    match values[0].as_number() {
        Some(n) if n >= 0.0 => CellValue::Number(n.sqrt()),
        Some(_) => CellValue::Error(CellError::NumError),
        None => CellValue::Error(CellError::InvalidValue),
    }
}

/// POWER - Raise to power
pub fn power(values: &[CellValue]) -> CellValue {
    if values.len() < 2 {
        return CellValue::Error(CellError::InvalidValue);
    }

    let base = match values[0].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let exp = match values[1].as_number() {
        Some(n) => n,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let result = base.powf(exp);
    if result.is_nan() || result.is_infinite() {
        CellValue::Error(CellError::NumError)
    } else {
        CellValue::Number(result)
    }
}

/// Criteria for conditional functions (COUNTIF, SUMIF, etc.)
#[derive(Debug, Clone)]
pub enum Criteria {
    Equal(CellValue),
    NotEqual(CellValue),
    GreaterThan(f64),
    GreaterThanOrEqual(f64),
    LessThan(f64),
    LessThanOrEqual(f64),
}

impl Criteria {
    /// Parse a criteria string like ">5", "<=10", "=text", "<>value"
    pub fn parse(value: &CellValue) -> Option<Criteria> {
        match value {
            CellValue::Number(n) => Some(Criteria::Equal(CellValue::Number(*n))),
            CellValue::Boolean(b) => Some(Criteria::Equal(CellValue::Boolean(*b))),
            CellValue::Text(s) => {
                let s = s.trim();
                if s.is_empty() {
                    return Some(Criteria::Equal(CellValue::Text(String::new())));
                }

                // Parse comparison operators
                if let Some(rest) = s.strip_prefix(">=") {
                    rest.trim()
                        .parse::<f64>()
                        .ok()
                        .map(Criteria::GreaterThanOrEqual)
                } else if let Some(rest) = s.strip_prefix("<=") {
                    rest.trim()
                        .parse::<f64>()
                        .ok()
                        .map(Criteria::LessThanOrEqual)
                } else if let Some(rest) = s.strip_prefix("<>") {
                    let rest = rest.trim();
                    if let Ok(n) = rest.parse::<f64>() {
                        Some(Criteria::NotEqual(CellValue::Number(n)))
                    } else {
                        Some(Criteria::NotEqual(CellValue::Text(rest.to_string())))
                    }
                } else if let Some(rest) = s.strip_prefix(">") {
                    rest.trim().parse::<f64>().ok().map(Criteria::GreaterThan)
                } else if let Some(rest) = s.strip_prefix("<") {
                    rest.trim().parse::<f64>().ok().map(Criteria::LessThan)
                } else if let Some(rest) = s.strip_prefix("=") {
                    let rest = rest.trim();
                    if let Ok(n) = rest.parse::<f64>() {
                        Some(Criteria::Equal(CellValue::Number(n)))
                    } else {
                        Some(Criteria::Equal(CellValue::Text(rest.to_string())))
                    }
                } else {
                    // Plain text - exact match (case-insensitive)
                    if let Ok(n) = s.parse::<f64>() {
                        Some(Criteria::Equal(CellValue::Number(n)))
                    } else {
                        Some(Criteria::Equal(CellValue::Text(s.to_string())))
                    }
                }
            }
            _ => None,
        }
    }

    /// Check if a value matches this criteria
    pub fn matches(&self, value: &CellValue) -> bool {
        match self {
            Criteria::Equal(target) => match (value, target) {
                (CellValue::Number(a), CellValue::Number(b)) => (a - b).abs() < 1e-10,
                (CellValue::Text(a), CellValue::Text(b)) => a.to_lowercase() == b.to_lowercase(),
                (CellValue::Boolean(a), CellValue::Boolean(b)) => a == b,
                (CellValue::Empty, CellValue::Text(s)) if s.is_empty() => true,
                _ => false,
            },
            Criteria::NotEqual(target) => match (value, target) {
                (CellValue::Number(a), CellValue::Number(b)) => (a - b).abs() >= 1e-10,
                (CellValue::Text(a), CellValue::Text(b)) => a.to_lowercase() != b.to_lowercase(),
                (CellValue::Boolean(a), CellValue::Boolean(b)) => a != b,
                _ => true,
            },
            Criteria::GreaterThan(n) => value.as_number().is_some_and(|v| v > *n),
            Criteria::GreaterThanOrEqual(n) => value.as_number().is_some_and(|v| v >= *n),
            Criteria::LessThan(n) => value.as_number().is_some_and(|v| v < *n),
            Criteria::LessThanOrEqual(n) => value.as_number().is_some_and(|v| v <= *n),
        }
    }
}

/// COUNTIF - Count cells matching criteria
/// Args: range_values, criteria
pub fn countif(range_values: &[CellValue], criteria: &CellValue) -> CellValue {
    let criteria = match Criteria::parse(criteria) {
        Some(c) => c,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let count = range_values.iter().filter(|v| criteria.matches(v)).count();
    CellValue::Number(count as f64)
}

/// SUMIF - Sum cells where criteria matches
/// Args: criteria_range_values, criteria, sum_range_values (optional, defaults to criteria_range)
pub fn sumif(
    criteria_range: &[CellValue],
    criteria: &CellValue,
    sum_range: Option<&[CellValue]>,
) -> CellValue {
    let criteria = match Criteria::parse(criteria) {
        Some(c) => c,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let sum_values = sum_range.unwrap_or(criteria_range);

    let mut total = 0.0;
    for (i, value) in criteria_range.iter().enumerate() {
        if criteria.matches(value) {
            if let Some(sum_val) = sum_values.get(i) {
                if let Some(n) = sum_val.as_number() {
                    total += n;
                }
            }
        }
    }

    CellValue::Number(total)
}

/// AVERAGEIF - Average cells where criteria matches
/// Args: criteria_range_values, criteria, average_range_values (optional)
pub fn averageif(
    criteria_range: &[CellValue],
    criteria: &CellValue,
    average_range: Option<&[CellValue]>,
) -> CellValue {
    let criteria = match Criteria::parse(criteria) {
        Some(c) => c,
        None => return CellValue::Error(CellError::InvalidValue),
    };

    let avg_values = average_range.unwrap_or(criteria_range);

    let mut total = 0.0;
    let mut count = 0;
    for (i, value) in criteria_range.iter().enumerate() {
        if criteria.matches(value) {
            if let Some(avg_val) = avg_values.get(i) {
                if let Some(n) = avg_val.as_number() {
                    total += n;
                    count += 1;
                }
            }
        }
    }

    if count == 0 {
        CellValue::Error(CellError::DivisionByZero)
    } else {
        CellValue::Number(total / count as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sum() {
        let values = vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
        ];
        assert_eq!(sum(&values), CellValue::Number(6.0));
    }

    #[test]
    fn test_average() {
        let values = vec![
            CellValue::Number(2.0),
            CellValue::Number(4.0),
            CellValue::Number(6.0),
        ];
        assert_eq!(average(&values), CellValue::Number(4.0));
    }

    #[test]
    fn test_count() {
        let values = vec![
            CellValue::Number(1.0),
            CellValue::Text("hello".to_string()),
            CellValue::Number(2.0),
            CellValue::Empty,
        ];
        assert_eq!(count(&values), CellValue::Number(2.0));
    }

    #[test]
    fn test_min_max() {
        let values = vec![
            CellValue::Number(5.0),
            CellValue::Number(2.0),
            CellValue::Number(8.0),
        ];
        assert_eq!(min(&values), CellValue::Number(2.0));
        assert_eq!(max(&values), CellValue::Number(8.0));
    }

    #[test]
    fn test_round() {
        let values = vec![CellValue::Number(3.14159), CellValue::Number(2.0)];
        assert_eq!(round(&values), CellValue::Number(3.14));
    }

    #[test]
    fn test_criteria_parse() {
        // Number criteria
        let c = Criteria::parse(&CellValue::Number(5.0)).unwrap();
        assert!(c.matches(&CellValue::Number(5.0)));
        assert!(!c.matches(&CellValue::Number(4.0)));

        // String comparison operators
        let c = Criteria::parse(&CellValue::Text(">5".to_string())).unwrap();
        assert!(c.matches(&CellValue::Number(6.0)));
        assert!(!c.matches(&CellValue::Number(5.0)));

        let c = Criteria::parse(&CellValue::Text("<=10".to_string())).unwrap();
        assert!(c.matches(&CellValue::Number(10.0)));
        assert!(c.matches(&CellValue::Number(5.0)));
        assert!(!c.matches(&CellValue::Number(11.0)));

        let c = Criteria::parse(&CellValue::Text("<>5".to_string())).unwrap();
        assert!(c.matches(&CellValue::Number(4.0)));
        assert!(!c.matches(&CellValue::Number(5.0)));
    }

    #[test]
    fn test_countif() {
        let values = vec![
            CellValue::Number(1.0),
            CellValue::Number(5.0),
            CellValue::Number(10.0),
            CellValue::Number(5.0),
        ];

        // Count equals 5
        assert_eq!(
            countif(&values, &CellValue::Number(5.0)),
            CellValue::Number(2.0)
        );

        // Count > 3
        assert_eq!(
            countif(&values, &CellValue::Text(">3".to_string())),
            CellValue::Number(3.0)
        );
    }

    #[test]
    fn test_sumif() {
        let values = vec![
            CellValue::Number(1.0),
            CellValue::Number(5.0),
            CellValue::Number(10.0),
            CellValue::Number(5.0),
        ];

        // Sum where > 3
        assert_eq!(
            sumif(&values, &CellValue::Text(">3".to_string()), None),
            CellValue::Number(20.0)
        );

        // Sum where = 5
        assert_eq!(
            sumif(&values, &CellValue::Number(5.0), None),
            CellValue::Number(10.0)
        );
    }

    #[test]
    fn test_averageif() {
        let values = vec![
            CellValue::Number(2.0),
            CellValue::Number(4.0),
            CellValue::Number(6.0),
            CellValue::Number(8.0),
        ];

        // Average where > 3
        assert_eq!(
            averageif(&values, &CellValue::Text(">3".to_string()), None),
            CellValue::Number(6.0)
        ); // (4+6+8)/3
    }
}
