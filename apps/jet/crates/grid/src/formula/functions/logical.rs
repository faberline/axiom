use crate::core::{CellError, CellValue};

/// IF - Conditional evaluation
pub fn if_fn(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    // Get condition
    let condition = match &values[0] {
        CellValue::Boolean(b) => *b,
        CellValue::Number(n) => *n != 0.0,
        CellValue::Error(e) => return CellValue::Error(e.clone()),
        _ => return CellValue::Error(CellError::InvalidValue),
    };

    if condition {
        // Return true value (or TRUE if not provided)
        if values.len() > 1 {
            values[1].clone()
        } else {
            CellValue::Boolean(true)
        }
    } else {
        // Return false value (or FALSE if not provided)
        if values.len() > 2 {
            values[2].clone()
        } else {
            CellValue::Boolean(false)
        }
    }
}

/// AND - Logical AND of all values
pub fn and(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    for value in values {
        match value {
            CellValue::Boolean(b) => {
                if !b {
                    return CellValue::Boolean(false);
                }
            }
            CellValue::Number(n) => {
                if *n == 0.0 {
                    return CellValue::Boolean(false);
                }
            }
            CellValue::Error(e) => return CellValue::Error(e.clone()),
            CellValue::Empty => {} // Skip empty
            CellValue::Text(_) => return CellValue::Error(CellError::InvalidValue),
        }
    }

    CellValue::Boolean(true)
}

/// OR - Logical OR of all values
pub fn or(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    for value in values {
        match value {
            CellValue::Boolean(b) => {
                if *b {
                    return CellValue::Boolean(true);
                }
            }
            CellValue::Number(n) => {
                if *n != 0.0 {
                    return CellValue::Boolean(true);
                }
            }
            CellValue::Error(e) => return CellValue::Error(e.clone()),
            CellValue::Empty => {} // Skip empty
            CellValue::Text(_) => return CellValue::Error(CellError::InvalidValue),
        }
    }

    CellValue::Boolean(false)
}

/// NOT - Logical NOT
pub fn not(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    match &values[0] {
        CellValue::Boolean(b) => CellValue::Boolean(!b),
        CellValue::Number(n) => CellValue::Boolean(*n == 0.0),
        CellValue::Error(e) => CellValue::Error(e.clone()),
        _ => CellValue::Error(CellError::InvalidValue),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_if() {
        // IF(TRUE, 1, 2) = 1
        let result = if_fn(&[
            CellValue::Boolean(true),
            CellValue::Number(1.0),
            CellValue::Number(2.0),
        ]);
        assert_eq!(result, CellValue::Number(1.0));

        // IF(FALSE, 1, 2) = 2
        let result = if_fn(&[
            CellValue::Boolean(false),
            CellValue::Number(1.0),
            CellValue::Number(2.0),
        ]);
        assert_eq!(result, CellValue::Number(2.0));
    }

    #[test]
    fn test_and() {
        assert_eq!(
            and(&[CellValue::Boolean(true), CellValue::Boolean(true)]),
            CellValue::Boolean(true)
        );
        assert_eq!(
            and(&[CellValue::Boolean(true), CellValue::Boolean(false)]),
            CellValue::Boolean(false)
        );
    }

    #[test]
    fn test_or() {
        assert_eq!(
            or(&[CellValue::Boolean(false), CellValue::Boolean(true)]),
            CellValue::Boolean(true)
        );
        assert_eq!(
            or(&[CellValue::Boolean(false), CellValue::Boolean(false)]),
            CellValue::Boolean(false)
        );
    }

    #[test]
    fn test_not() {
        assert_eq!(not(&[CellValue::Boolean(true)]), CellValue::Boolean(false));
        assert_eq!(not(&[CellValue::Boolean(false)]), CellValue::Boolean(true));
    }
}
