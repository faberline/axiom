//! Join operations for DataFrame.

use crate::frame::dataframe::DataFrame;
use crate::frame::error::Result;
use crate::frame::series::Series;
use crate::frame::value::Value;
use std::collections::HashMap;

/// Type of join operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JoinType {
    /// Inner join - only matching keys.
    Inner,
    /// Left join - all left rows, matching right rows.
    Left,
    /// Right join - all right rows, matching left rows.
    Right,
    /// Outer join - all rows from both.
    Outer,
}

/// Join two DataFrames on specified columns.
pub fn join(left: &DataFrame, right: &DataFrame, on: &[&str], how: JoinType) -> Result<DataFrame> {
    // Validate join columns exist in both DataFrames
    for &col in on {
        left.get(col)?;
        right.get(col)?;
    }

    // Build index for right DataFrame
    let mut right_index: HashMap<Vec<Value>, Vec<usize>> = HashMap::new();
    for i in 0..right.nrows() {
        let key: Vec<Value> = on
            .iter()
            .map(|&col| right.get(col).unwrap().iloc(i).unwrap().clone())
            .collect();
        right_index.entry(key).or_default().push(i);
    }

    // Determine output columns (avoid duplicating join keys)
    let left_cols: Vec<&str> = left.columns().iter().map(|s| s.as_str()).collect();
    let right_cols: Vec<&str> = right
        .columns()
        .iter()
        .filter(|c| !on.contains(&c.as_str()))
        .map(|s| s.as_str())
        .collect();

    // Build result data
    let mut result_data: HashMap<String, Vec<Value>> = HashMap::new();

    // Initialize columns
    for &col in &left_cols {
        result_data.insert(col.to_string(), Vec::new());
    }
    for &col in &right_cols {
        result_data.insert(format!("{}_right", col), Vec::new());
    }

    // Track which right rows have been matched (for outer join)
    let mut right_matched: Vec<bool> = vec![false; right.nrows()];

    // Process left rows
    for left_i in 0..left.nrows() {
        let key: Vec<Value> = on
            .iter()
            .map(|&col| left.get(col).unwrap().iloc(left_i).unwrap().clone())
            .collect();

        let right_matches = right_index.get(&key);

        match (how, right_matches) {
            (JoinType::Inner, None) => {
                // No match, skip
            }
            (JoinType::Inner, Some(matches)) => {
                // Add all matching pairs
                for &right_i in matches {
                    right_matched[right_i] = true;
                    add_row(
                        &mut result_data,
                        left,
                        Some(left_i),
                        right,
                        Some(right_i),
                        &left_cols,
                        &right_cols,
                    );
                }
            }
            (JoinType::Left, None) => {
                // Add left row with nulls for right
                add_row(
                    &mut result_data,
                    left,
                    Some(left_i),
                    right,
                    None,
                    &left_cols,
                    &right_cols,
                );
            }
            (JoinType::Left, Some(matches)) => {
                for &right_i in matches {
                    right_matched[right_i] = true;
                    add_row(
                        &mut result_data,
                        left,
                        Some(left_i),
                        right,
                        Some(right_i),
                        &left_cols,
                        &right_cols,
                    );
                }
            }
            (JoinType::Right, None) => {
                // Will be handled in right-only pass
            }
            (JoinType::Right, Some(matches)) => {
                for &right_i in matches {
                    right_matched[right_i] = true;
                    add_row(
                        &mut result_data,
                        left,
                        Some(left_i),
                        right,
                        Some(right_i),
                        &left_cols,
                        &right_cols,
                    );
                }
            }
            (JoinType::Outer, None) => {
                add_row(
                    &mut result_data,
                    left,
                    Some(left_i),
                    right,
                    None,
                    &left_cols,
                    &right_cols,
                );
            }
            (JoinType::Outer, Some(matches)) => {
                for &right_i in matches {
                    right_matched[right_i] = true;
                    add_row(
                        &mut result_data,
                        left,
                        Some(left_i),
                        right,
                        Some(right_i),
                        &left_cols,
                        &right_cols,
                    );
                }
            }
        }
    }

    // Process unmatched right rows (for right and outer joins)
    if matches!(how, JoinType::Right | JoinType::Outer) {
        for (right_i, &matched) in right_matched.iter().enumerate() {
            if !matched {
                add_row(
                    &mut result_data,
                    left,
                    None,
                    right,
                    Some(right_i),
                    &left_cols,
                    &right_cols,
                );
            }
        }
    }

    // Build DataFrame
    let mut columns: Vec<(&str, Series)> = Vec::new();

    for &col in &left_cols {
        let values = result_data.remove(col).unwrap();
        columns.push((col, Series::new(values)));
    }

    for &col in &right_cols {
        let key = format!("{}_right", col);
        let values = result_data.remove(&key).unwrap();
        columns.push((Box::leak(key.into_boxed_str()), Series::new(values)));
    }

    DataFrame::from_columns(columns)
}

fn add_row(
    result: &mut HashMap<String, Vec<Value>>,
    left: &DataFrame,
    left_i: Option<usize>,
    right: &DataFrame,
    right_i: Option<usize>,
    left_cols: &[&str],
    right_cols: &[&str],
) {
    // Add left columns
    for &col in left_cols {
        let value = left_i
            .and_then(|i| left.get(col).ok()?.iloc(i).ok().cloned())
            .unwrap_or(Value::Null);
        result.get_mut(col).unwrap().push(value);
    }

    // Add right columns
    for &col in right_cols {
        let key = format!("{}_right", col);
        let value = right_i
            .and_then(|i| right.get(col).ok()?.iloc(i).ok().cloned())
            .unwrap_or(Value::Null);
        result.get_mut(&key).unwrap().push(value);
    }
}

// Extension trait for DataFrame
impl DataFrame {
    /// Join with another DataFrame.
    pub fn join(&self, other: &DataFrame, on: &[&str], how: JoinType) -> Result<DataFrame> {
        join(self, other, on, how)
    }

    /// Merge (alias for join with inner).
    pub fn merge(&self, other: &DataFrame, on: &[&str]) -> Result<DataFrame> {
        self.join(other, on, JoinType::Inner)
    }

    /// Left join.
    pub fn left_join(&self, other: &DataFrame, on: &[&str]) -> Result<DataFrame> {
        self.join(other, on, JoinType::Left)
    }

    /// Right join.
    pub fn right_join(&self, other: &DataFrame, on: &[&str]) -> Result<DataFrame> {
        self.join(other, on, JoinType::Right)
    }

    /// Outer join.
    pub fn outer_join(&self, other: &DataFrame, on: &[&str]) -> Result<DataFrame> {
        self.join(other, on, JoinType::Outer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_left_df() -> DataFrame {
        DataFrame::from_columns(vec![
            ("key", Series::new(vec!["a", "b", "c"])),
            ("left_val", Series::new(vec![1, 2, 3])),
        ])
        .unwrap()
    }

    fn create_right_df() -> DataFrame {
        DataFrame::from_columns(vec![
            ("key", Series::new(vec!["b", "c", "d"])),
            ("right_val", Series::new(vec![20, 30, 40])),
        ])
        .unwrap()
    }

    #[test]
    fn test_inner_join() {
        let left = create_left_df();
        let right = create_right_df();
        let result = left.join(&right, &["key"], JoinType::Inner).unwrap();

        assert_eq!(result.nrows(), 2); // b, c
        assert_eq!(result.ncols(), 3); // key, left_val, right_val_right
    }

    #[test]
    fn test_left_join() {
        let left = create_left_df();
        let right = create_right_df();
        let result = left.left_join(&right, &["key"]).unwrap();

        assert_eq!(result.nrows(), 3); // a, b, c
    }

    #[test]
    fn test_right_join() {
        let left = create_left_df();
        let right = create_right_df();
        let result = left.right_join(&right, &["key"]).unwrap();

        assert_eq!(result.nrows(), 3); // b, c, d
    }

    #[test]
    fn test_outer_join() {
        let left = create_left_df();
        let right = create_right_df();
        let result = left.outer_join(&right, &["key"]).unwrap();

        assert_eq!(result.nrows(), 4); // a, b, c, d
    }
}
