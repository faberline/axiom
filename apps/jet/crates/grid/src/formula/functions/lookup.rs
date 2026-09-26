use crate::core::{CellError, CellValue};

/// MATCH - Search for a value in an array and return its relative position
/// Args: lookup_value, lookup_array (slice), match_type
pub fn match_fn(
    lookup_value: &CellValue,
    lookup_array: &[CellValue],
    match_type: i32,
) -> CellValue {
    if lookup_array.is_empty() {
        return CellValue::Error(CellError::NotAvailable);
    }

    match match_type {
        0 => match_exact(lookup_value, lookup_array),
        1 => match_less_than(lookup_value, lookup_array),
        -1 => match_greater_than(lookup_value, lookup_array),
        _ => CellValue::Error(CellError::InvalidValue),
    }
}

fn match_exact(target: &CellValue, array: &[CellValue]) -> CellValue {
    // Basic linear search for exact match
    // Support wildcards? Excel supports * and ? for text in match_type 0.
    // For now, let's just do equality.

    for (i, value) in array.iter().enumerate() {
        if values_equal(target, value) {
            return CellValue::Number((i + 1) as f64);
        }
    }

    CellValue::Error(CellError::NotAvailable)
}

fn match_less_than(target: &CellValue, array: &[CellValue]) -> CellValue {
    // Array must be sorted ascending.
    // Finds largest value that is <= target.
    // Since it's sorted, we can iterate and stop when we exceed target, or use last match.

    let mut best_idx = None;

    for (i, value) in array.iter().enumerate() {
        if let Some(ordering) = compare_values(value, target) {
            if ordering <= 0 {
                // value <= target
                best_idx = Some(i + 1);
            }
            // value > target: Since sorted ascending, we could stop but Excel behavior varies.
        }
    }

    match best_idx {
        Some(idx) => CellValue::Number(idx as f64),
        None => CellValue::Error(CellError::NotAvailable),
    }
}

fn match_greater_than(target: &CellValue, array: &[CellValue]) -> CellValue {
    // Array must be sorted descending.
    // Finds smallest value that is >= target.

    let mut best_idx = None;

    for (i, value) in array.iter().enumerate() {
        if let Some(ordering) = compare_values(value, target) {
            if ordering >= 0 {
                // value >= target
                best_idx = Some(i + 1);
            }
        }
    }

    match best_idx {
        Some(idx) => CellValue::Number(idx as f64),
        None => CellValue::Error(CellError::NotAvailable),
    }
}

fn values_equal(a: &CellValue, b: &CellValue) -> bool {
    match (a, b) {
        (CellValue::Number(n1), CellValue::Number(n2)) => (n1 - n2).abs() < 1e-10,
        (CellValue::Text(s1), CellValue::Text(s2)) => s1.to_lowercase() == s2.to_lowercase(),
        (CellValue::Boolean(b1), CellValue::Boolean(b2)) => b1 == b2,
        _ => false,
    }
}

fn compare_values(a: &CellValue, b: &CellValue) -> Option<i8> {
    match (a, b) {
        (CellValue::Number(n1), CellValue::Number(n2)) => {
            if n1 < n2 {
                Some(-1)
            } else if n1 > n2 {
                Some(1)
            } else {
                Some(0)
            }
        }
        (CellValue::Text(s1), CellValue::Text(s2)) => {
            let cmp = s1.to_lowercase().cmp(&s2.to_lowercase());
            Some(if cmp == std::cmp::Ordering::Less {
                -1
            } else if cmp == std::cmp::Ordering::Greater {
                1
            } else {
                0
            })
        }
        (CellValue::Boolean(b1), CellValue::Boolean(b2)) => {
            let n1 = if *b1 { 1 } else { 0 };
            let n2 = if *b2 { 1 } else { 0 };
            if n1 < n2 {
                Some(-1)
            } else if n1 > n2 {
                Some(1)
            } else {
                Some(0)
            }
        }
        _ => None, // Incomparable types
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_exact() {
        let array = vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ];

        assert_eq!(
            match_fn(&CellValue::Number(20.0), &array, 0),
            CellValue::Number(2.0)
        );
        assert!(matches!(
            match_fn(&CellValue::Number(25.0), &array, 0),
            CellValue::Error(CellError::NotAvailable)
        ));
    }

    #[test]
    fn test_match_less_than() {
        // Sorted ascending: 10, 20, 30
        let array = vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ];

        // Match 25 -> Should find 20 (index 2)
        assert_eq!(
            match_fn(&CellValue::Number(25.0), &array, 1),
            CellValue::Number(2.0)
        );

        // Match 9 -> #N/A (less than smallest)
        assert!(matches!(
            match_fn(&CellValue::Number(9.0), &array, 1),
            CellValue::Error(CellError::NotAvailable)
        ));

        // Match 35 -> 30 (index 3)
        assert_eq!(
            match_fn(&CellValue::Number(35.0), &array, 1),
            CellValue::Number(3.0)
        );
    }

    #[test]
    fn test_match_greater_than() {
        // Sorted descending: 30, 20, 10
        let array = vec![
            CellValue::Number(30.0),
            CellValue::Number(20.0),
            CellValue::Number(10.0),
        ];

        // Match 25 -> Should find 30 (index 1) - smallest value >= 25
        assert_eq!(
            match_fn(&CellValue::Number(25.0), &array, -1),
            CellValue::Number(1.0)
        );

        // Match 5 -> 10 (index 3)
        assert_eq!(
            match_fn(&CellValue::Number(5.0), &array, -1),
            CellValue::Number(3.0)
        );

        // Match 35 -> #N/A (greater than largest)
        assert!(matches!(
            match_fn(&CellValue::Number(35.0), &array, -1),
            CellValue::Error(CellError::NotAvailable)
        ));
    }

    #[test]
    fn test_match_text() {
        let array = vec![
            CellValue::Text("Apple".to_string()),
            CellValue::Text("Banana".to_string()),
            CellValue::Text("Cherry".to_string()),
        ];

        assert_eq!(
            match_fn(&CellValue::Text("banana".to_string()), &array, 0),
            CellValue::Number(2.0)
        );
    }
}

/// VLOOKUP - Searches for a value in the first column of a range and returns a value from the specified column
///
/// Arguments:
/// - lookup_value: The value to search for
/// - table: The flattened table data (row-major order)
/// - num_rows: Number of rows in the table
/// - num_cols: Number of columns in the table
/// - col_index: The column number (1-based) to return from
/// - approximate: If true, finds closest match (requires sorted first column); if false, exact match only
///
/// Returns:
/// - The value from the specified column in the matching row
/// - #N/A error if not found
/// - #REF! error if col_index is out of bounds
pub fn vlookup(
    lookup_value: &CellValue,
    table: &[CellValue],
    num_rows: usize,
    num_cols: usize,
    col_index: usize,
    approximate: bool,
) -> CellValue {
    // Validation
    if num_rows == 0 || num_cols == 0 || table.len() != num_rows * num_cols {
        return CellValue::Error(CellError::InvalidValue);
    }

    if col_index == 0 || col_index > num_cols {
        return CellValue::Error(CellError::InvalidReference);
    }

    // Extract first column for searching
    let first_col: Vec<CellValue> = (0..num_rows)
        .map(|row| table[row * num_cols].clone())
        .collect();

    // Use match logic to find row
    let match_type = if approximate { 1 } else { 0 };
    let match_result = match_fn(lookup_value, &first_col, match_type);

    match match_result {
        CellValue::Number(row_idx) => {
            let row = (row_idx as usize) - 1; // Convert 1-based to 0-based
            let value_index = row * num_cols + (col_index - 1);
            table
                .get(value_index)
                .cloned()
                .unwrap_or(CellValue::Error(CellError::NotAvailable))
        }
        CellValue::Error(e) => CellValue::Error(e),
        _ => CellValue::Error(CellError::NotAvailable),
    }
}

/// HLOOKUP - Searches for a value in the first row of a range and returns a value from the specified row
///
/// Arguments similar to VLOOKUP but searches horizontally
pub fn hlookup(
    lookup_value: &CellValue,
    table: &[CellValue],
    num_rows: usize,
    num_cols: usize,
    row_index: usize,
    approximate: bool,
) -> CellValue {
    // Validation
    if num_rows == 0 || num_cols == 0 || table.len() != num_rows * num_cols {
        return CellValue::Error(CellError::InvalidValue);
    }

    if row_index == 0 || row_index > num_rows {
        return CellValue::Error(CellError::InvalidReference);
    }

    // Extract first row for searching
    let first_row: Vec<CellValue> = (0..num_cols).map(|col| table[col].clone()).collect();

    // Use match logic to find column
    let match_type = if approximate { 1 } else { 0 };
    let match_result = match_fn(lookup_value, &first_row, match_type);

    match match_result {
        CellValue::Number(col_idx) => {
            let col = (col_idx as usize) - 1;
            let value_index = (row_index - 1) * num_cols + col;
            table
                .get(value_index)
                .cloned()
                .unwrap_or(CellValue::Error(CellError::NotAvailable))
        }
        CellValue::Error(e) => CellValue::Error(e),
        _ => CellValue::Error(CellError::NotAvailable),
    }
}

#[cfg(test)]
mod vlookup_tests {
    use super::*;

    #[test]
    fn test_vlookup_exact() {
        // Test data: 3 rows x 3 columns
        // | ID | Name    | Score |
        // | 10 | Alice   | 85    |
        // | 20 | Bob     | 90    |
        // | 30 | Charlie | 75    |
        let table = vec![
            CellValue::Number(10.0),
            CellValue::Text("Alice".to_string()),
            CellValue::Number(85.0),
            CellValue::Number(20.0),
            CellValue::Text("Bob".to_string()),
            CellValue::Number(90.0),
            CellValue::Number(30.0),
            CellValue::Text("Charlie".to_string()),
            CellValue::Number(75.0),
        ];

        // Look up 20, return column 2 (Name)
        let result = vlookup(&CellValue::Number(20.0), &table, 3, 3, 2, false);
        assert_eq!(result, CellValue::Text("Bob".to_string()));

        // Look up 10, return column 3 (Score)
        let result = vlookup(&CellValue::Number(10.0), &table, 3, 3, 3, false);
        assert_eq!(result, CellValue::Number(85.0));
    }

    #[test]
    fn test_vlookup_approximate() {
        // Test data sorted by first column (required for approximate match)
        // | Grade | Letter |
        // | 0     | F      |
        // | 60    | D      |
        // | 70    | C      |
        // | 80    | B      |
        // | 90    | A      |
        let table = vec![
            CellValue::Number(0.0),
            CellValue::Text("F".to_string()),
            CellValue::Number(60.0),
            CellValue::Text("D".to_string()),
            CellValue::Number(70.0),
            CellValue::Text("C".to_string()),
            CellValue::Number(80.0),
            CellValue::Text("B".to_string()),
            CellValue::Number(90.0),
            CellValue::Text("A".to_string()),
        ];

        // Look up 85 (should find 80 -> B)
        let result = vlookup(&CellValue::Number(85.0), &table, 5, 2, 2, true);
        assert_eq!(result, CellValue::Text("B".to_string()));

        // Look up 95 (should find 90 -> A)
        let result = vlookup(&CellValue::Number(95.0), &table, 5, 2, 2, true);
        assert_eq!(result, CellValue::Text("A".to_string()));

        // Look up 65 (should find 60 -> D)
        let result = vlookup(&CellValue::Number(65.0), &table, 5, 2, 2, true);
        assert_eq!(result, CellValue::Text("D".to_string()));
    }

    #[test]
    fn test_vlookup_not_found() {
        let table = vec![
            CellValue::Number(10.0),
            CellValue::Text("Alice".to_string()),
            CellValue::Number(20.0),
            CellValue::Text("Bob".to_string()),
        ];

        // Exact match not found
        let result = vlookup(&CellValue::Number(15.0), &table, 2, 2, 2, false);
        assert!(matches!(result, CellValue::Error(CellError::NotAvailable)));
    }

    #[test]
    fn test_vlookup_invalid_col_index() {
        let table = vec![
            CellValue::Number(10.0),
            CellValue::Text("Alice".to_string()),
        ];

        // col_index = 0 (invalid)
        let result = vlookup(&CellValue::Number(10.0), &table, 1, 2, 0, false);
        assert!(matches!(
            result,
            CellValue::Error(CellError::InvalidReference)
        ));

        // col_index = 3 (out of bounds)
        let result = vlookup(&CellValue::Number(10.0), &table, 1, 2, 3, false);
        assert!(matches!(
            result,
            CellValue::Error(CellError::InvalidReference)
        ));
    }

    #[test]
    fn test_hlookup_exact() {
        // Test data: 3 rows x 3 columns
        // | Product A | Product B | Product C |
        // | 100       | 200       | 300       |
        // | In Stock  | Sold Out  | In Stock  |
        let table = vec![
            CellValue::Text("Product A".to_string()),
            CellValue::Text("Product B".to_string()),
            CellValue::Text("Product C".to_string()),
            CellValue::Number(100.0),
            CellValue::Number(200.0),
            CellValue::Number(300.0),
            CellValue::Text("In Stock".to_string()),
            CellValue::Text("Sold Out".to_string()),
            CellValue::Text("In Stock".to_string()),
        ];

        // Look up "Product B", return row 2 (Price)
        let result = hlookup(
            &CellValue::Text("Product B".to_string()),
            &table,
            3,
            3,
            2,
            false,
        );
        assert_eq!(result, CellValue::Number(200.0));

        // Look up "Product C", return row 3 (Status)
        let result = hlookup(
            &CellValue::Text("Product C".to_string()),
            &table,
            3,
            3,
            3,
            false,
        );
        assert_eq!(result, CellValue::Text("In Stock".to_string()));
    }

    #[test]
    fn test_hlookup_not_found() {
        let table = vec![
            CellValue::Text("A".to_string()),
            CellValue::Text("B".to_string()),
            CellValue::Number(1.0),
            CellValue::Number(2.0),
        ];

        // Exact match not found
        let result = hlookup(&CellValue::Text("C".to_string()), &table, 2, 2, 2, false);
        assert!(matches!(result, CellValue::Error(CellError::NotAvailable)));
    }

    #[test]
    fn test_hlookup_invalid_row_index() {
        let table = vec![
            CellValue::Text("A".to_string()),
            CellValue::Text("B".to_string()),
        ];

        // row_index = 0 (invalid)
        let result = hlookup(&CellValue::Text("A".to_string()), &table, 1, 2, 0, false);
        assert!(matches!(
            result,
            CellValue::Error(CellError::InvalidReference)
        ));

        // row_index = 2 (out of bounds)
        let result = hlookup(&CellValue::Text("A".to_string()), &table, 1, 2, 2, false);
        assert!(matches!(
            result,
            CellValue::Error(CellError::InvalidReference)
        ));
    }
}
