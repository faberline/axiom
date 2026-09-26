use crate::core::{CellError, CellValue};

/// CONCAT / CONCATENATE - Concatenate strings
pub fn concat(values: &[CellValue]) -> CellValue {
    let mut result = String::new();

    for value in values {
        match value {
            CellValue::Error(e) => return CellValue::Error(e.clone()),
            _ => result.push_str(&value.as_text()),
        }
    }

    CellValue::Text(result)
}

/// LEN - Length of text
pub fn len(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    match &values[0] {
        CellValue::Error(e) => CellValue::Error(e.clone()),
        value => CellValue::Number(value.as_text().len() as f64),
    }
}

/// UPPER - Convert to uppercase
pub fn upper(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    match &values[0] {
        CellValue::Error(e) => CellValue::Error(e.clone()),
        value => CellValue::Text(value.as_text().to_uppercase()),
    }
}

/// LOWER - Convert to lowercase
pub fn lower(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    match &values[0] {
        CellValue::Error(e) => CellValue::Error(e.clone()),
        value => CellValue::Text(value.as_text().to_lowercase()),
    }
}

/// TRIM - Remove leading/trailing whitespace
pub fn trim(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    match &values[0] {
        CellValue::Error(e) => CellValue::Error(e.clone()),
        value => CellValue::Text(value.as_text().trim().to_string()),
    }
}

/// LEFT - Extract leftmost characters
pub fn left(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    let text = match &values[0] {
        CellValue::Error(e) => return CellValue::Error(e.clone()),
        value => value.as_text(),
    };

    let num_chars = if values.len() > 1 {
        match values[1].as_number() {
            Some(n) if n >= 0.0 => n as usize,
            Some(_) => return CellValue::Error(CellError::InvalidValue),
            None => return CellValue::Error(CellError::InvalidValue),
        }
    } else {
        1
    };

    let result: String = text.chars().take(num_chars).collect();
    CellValue::Text(result)
}

/// RIGHT - Extract rightmost characters
pub fn right(values: &[CellValue]) -> CellValue {
    if values.is_empty() {
        return CellValue::Error(CellError::InvalidValue);
    }

    let text = match &values[0] {
        CellValue::Error(e) => return CellValue::Error(e.clone()),
        value => value.as_text(),
    };

    let num_chars = if values.len() > 1 {
        match values[1].as_number() {
            Some(n) if n >= 0.0 => n as usize,
            Some(_) => return CellValue::Error(CellError::InvalidValue),
            None => return CellValue::Error(CellError::InvalidValue),
        }
    } else {
        1
    };

    let char_count = text.chars().count();
    let skip = char_count.saturating_sub(num_chars);
    let result: String = text.chars().skip(skip).collect();
    CellValue::Text(result)
}

/// MID - Extract middle characters
pub fn mid(values: &[CellValue]) -> CellValue {
    if values.len() < 3 {
        return CellValue::Error(CellError::InvalidValue);
    }

    let text = match &values[0] {
        CellValue::Error(e) => return CellValue::Error(e.clone()),
        value => value.as_text(),
    };

    let start_pos = match values[1].as_number() {
        Some(n) if n >= 1.0 => (n as usize) - 1, // 1-indexed to 0-indexed
        _ => return CellValue::Error(CellError::InvalidValue),
    };

    let num_chars = match values[2].as_number() {
        Some(n) if n >= 0.0 => n as usize,
        _ => return CellValue::Error(CellError::InvalidValue),
    };

    let result: String = text.chars().skip(start_pos).take(num_chars).collect();
    CellValue::Text(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concat() {
        let result = concat(&[
            CellValue::Text("Hello".to_string()),
            CellValue::Text(" ".to_string()),
            CellValue::Text("World".to_string()),
        ]);
        assert_eq!(result, CellValue::Text("Hello World".to_string()));
    }

    #[test]
    fn test_len() {
        let result = len(&[CellValue::Text("Hello".to_string())]);
        assert_eq!(result, CellValue::Number(5.0));
    }

    #[test]
    fn test_upper_lower() {
        let result = upper(&[CellValue::Text("hello".to_string())]);
        assert_eq!(result, CellValue::Text("HELLO".to_string()));

        let result = lower(&[CellValue::Text("HELLO".to_string())]);
        assert_eq!(result, CellValue::Text("hello".to_string()));
    }

    #[test]
    fn test_left_right_mid() {
        let text = CellValue::Text("Hello World".to_string());

        // LEFT("Hello World", 5) = "Hello"
        let result = left(&[text.clone(), CellValue::Number(5.0)]);
        assert_eq!(result, CellValue::Text("Hello".to_string()));

        // RIGHT("Hello World", 5) = "World"
        let result = right(&[text.clone(), CellValue::Number(5.0)]);
        assert_eq!(result, CellValue::Text("World".to_string()));

        // MID("Hello World", 7, 5) = "World"
        let result = mid(&[text, CellValue::Number(7.0), CellValue::Number(5.0)]);
        assert_eq!(result, CellValue::Text("World".to_string()));
    }
}
