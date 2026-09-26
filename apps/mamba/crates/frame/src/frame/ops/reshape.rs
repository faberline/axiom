//! Reshape operations: stack, unstack, wide_to_long, cross_tab, pivot_table.

use crate::frame::dataframe::DataFrame;
use crate::frame::error::{FrameError, Result};
use crate::frame::series::Series;
use crate::frame::value::Value;
use std::collections::HashMap;

/// Aggregation function type for pivot_table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AggFunc {
    Sum,
    Mean,
    Count,
    Min,
    Max,
    First,
    Last,
}

/// Pivot table with aggregation support.
///
/// Groups by (index, columns) and aggregates values.
pub fn pivot_table(
    df: &DataFrame,
    values: &str,
    index: &str,
    columns: &str,
    aggfunc: AggFunc,
) -> Result<DataFrame> {
    let idx_col = df.get(index)?;
    let col_col = df.get(columns)?;
    let val_col = df.get(values)?;

    // Get unique index and column values (sorted for deterministic output)
    let mut unique_idx = idx_col.unique();
    unique_idx.sort();
    let mut unique_cols = col_col.unique();
    unique_cols.sort();

    let idx_map: HashMap<Value, usize> = unique_idx
        .iter()
        .enumerate()
        .map(|(i, v)| (v.clone(), i))
        .collect();

    let col_map: HashMap<Value, usize> = unique_cols
        .iter()
        .enumerate()
        .map(|(i, v)| (v.clone(), i))
        .collect();

    // Collect values into groups: (row_idx, col_idx) -> Vec<f64>
    let mut groups: HashMap<(usize, usize), Vec<f64>> = HashMap::new();

    for i in 0..df.nrows() {
        let idx_val = idx_col.iloc(i)?;
        let col_val = col_col.iloc(i)?;
        let val = val_col.iloc(i)?;

        if let (Some(&ri), Some(&ci)) = (idx_map.get(idx_val), col_map.get(col_val)) {
            if let Some(f) = val.as_float() {
                groups.entry((ri, ci)).or_default().push(f);
            }
        }
    }

    // Aggregate
    let mut result_data: Vec<Vec<Value>> =
        vec![vec![Value::Null; unique_idx.len()]; unique_cols.len()];

    for (&(ri, ci), vals) in &groups {
        let agg_val = match aggfunc {
            AggFunc::Sum => vals.iter().sum::<f64>(),
            AggFunc::Mean => vals.iter().sum::<f64>() / vals.len() as f64,
            AggFunc::Count => vals.len() as f64,
            AggFunc::Min => vals.iter().cloned().fold(f64::INFINITY, f64::min),
            AggFunc::Max => vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            AggFunc::First => *vals.first().unwrap_or(&f64::NAN),
            AggFunc::Last => *vals.last().unwrap_or(&f64::NAN),
        };
        result_data[ci][ri] = Value::Float(agg_val);
    }

    // Build result DataFrame
    let mut cols_vec: Vec<(&str, Series)> = vec![(index, Series::new(unique_idx))];
    for (i, col_val) in unique_cols.iter().enumerate() {
        let col_name: &str = Box::leak(col_val.to_string().into_boxed_str());
        cols_vec.push((col_name, Series::new(result_data[i].clone())));
    }

    DataFrame::from_columns(cols_vec)
}

/// Stack: pivot columns into rows (wide -> long).
///
/// Takes specified columns and stacks them into two columns: "variable" and "value".
/// All other columns are repeated for each stacked row.
pub fn stack(df: &DataFrame, columns_to_stack: &[&str]) -> Result<DataFrame> {
    let id_vars: Vec<&str> = df
        .columns()
        .iter()
        .filter(|c| !columns_to_stack.contains(&c.as_str()))
        .map(|s| s.as_str())
        .collect();

    df.melt(&id_vars, columns_to_stack)
}

/// Unstack: pivot rows into columns (long -> wide).
///
/// Takes a "variable" column and a "value" column and creates separate
/// columns for each unique variable value.
pub fn unstack(df: &DataFrame, index: &str, variable: &str, value: &str) -> Result<DataFrame> {
    df.pivot(index, variable, value)
}

/// Wide to long format conversion.
///
/// Converts columns with a common prefix (e.g., "score_1", "score_2")
/// into long format with a suffix column and value column.
pub fn wide_to_long(
    df: &DataFrame,
    stub_names: &[&str],
    i: &str,
    j: &str,
    sep: &str,
) -> Result<DataFrame> {
    // Identify columns matching each stub
    let mut stub_cols: HashMap<&str, Vec<String>> = HashMap::new();
    for stub in stub_names {
        let matching: Vec<String> = df
            .columns()
            .iter()
            .filter(|c| c.starts_with(stub) && c.len() > stub.len())
            .cloned()
            .collect();
        stub_cols.insert(stub, matching);
    }

    // Determine suffixes from the first stub
    let first_stub = stub_names
        .first()
        .ok_or_else(|| FrameError::InvalidOperation("stub_names must not be empty".into()))?;
    let suffixes: Vec<String> = stub_cols
        .get(first_stub)
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|c| {
            let prefix = format!("{}{}", first_stub, sep);
            c.strip_prefix(&prefix).map(String::from)
        })
        .collect();

    if suffixes.is_empty() {
        return Err(FrameError::InvalidOperation(
            "no matching columns found for stub names".into(),
        ));
    }

    // ID column
    let id_col = df.get(i)?;
    let n_rows = df.nrows();
    let n_suffixes = suffixes.len();
    let total_rows = n_rows * n_suffixes;

    // Build output
    let mut id_data: Vec<Value> = Vec::with_capacity(total_rows);
    let mut j_data: Vec<Value> = Vec::with_capacity(total_rows);
    let mut stub_data: HashMap<&str, Vec<Value>> = HashMap::new();
    for stub in stub_names {
        stub_data.insert(stub, Vec::with_capacity(total_rows));
    }

    for suffix in &suffixes {
        for row in 0..n_rows {
            id_data.push(id_col.iloc(row).cloned().unwrap_or(Value::Null));
            j_data.push(Value::String(suffix.clone()));

            for stub in stub_names {
                let col_name = format!("{}{}{}", stub, sep, suffix);
                let val = df
                    .get(&col_name)
                    .ok()
                    .and_then(|s| s.iloc(row).cloned().ok())
                    .unwrap_or(Value::Null);
                stub_data.get_mut(stub).unwrap().push(val);
            }
        }
    }

    let mut result_cols: Vec<(&str, Series)> =
        vec![(i, Series::new(id_data)), (j, Series::new(j_data))];
    for stub in stub_names {
        let data = stub_data.remove(stub).unwrap();
        result_cols.push((*stub, Series::new(data)));
    }

    DataFrame::from_columns(result_cols)
}

/// Cross tabulation: frequency table of two columns.
pub fn cross_tab(df: &DataFrame, index: &str, columns: &str) -> Result<DataFrame> {
    pivot_table(df, columns, index, columns, AggFunc::Count).or_else(|_| {
        // Fallback: build manually using counts
        let idx_col = df.get(index)?;
        let col_col = df.get(columns)?;

        let mut unique_idx = idx_col.unique();
        unique_idx.sort();
        let mut unique_cols = col_col.unique();
        unique_cols.sort();

        let idx_map: HashMap<Value, usize> = unique_idx
            .iter()
            .enumerate()
            .map(|(i, v)| (v.clone(), i))
            .collect();

        let col_map: HashMap<Value, usize> = unique_cols
            .iter()
            .enumerate()
            .map(|(i, v)| (v.clone(), i))
            .collect();

        // Count occurrences
        let mut counts = vec![vec![0i64; unique_idx.len()]; unique_cols.len()];

        for row in 0..df.nrows() {
            let iv = idx_col.iloc(row)?;
            let cv = col_col.iloc(row)?;
            if let (Some(&ri), Some(&ci)) = (idx_map.get(iv), col_map.get(cv)) {
                counts[ci][ri] += 1;
            }
        }

        let mut result_cols: Vec<(&str, Series)> = vec![(index, Series::new(unique_idx))];
        for (ci, col_val) in unique_cols.iter().enumerate() {
            let col_name: &str = Box::leak(col_val.to_string().into_boxed_str());
            let data: Vec<Value> = counts[ci].iter().map(|&c| Value::Int(c)).collect();
            result_cols.push((col_name, Series::new(data)));
        }

        DataFrame::from_columns(result_cols)
    })
}

// Extension methods for DataFrame
impl DataFrame {
    /// Pivot table with aggregation.
    pub fn pivot_table(
        &self,
        values: &str,
        index: &str,
        columns: &str,
        aggfunc: AggFunc,
    ) -> Result<DataFrame> {
        pivot_table(self, values, index, columns, aggfunc)
    }

    /// Stack columns into rows.
    pub fn stack(&self, columns_to_stack: &[&str]) -> Result<DataFrame> {
        stack(self, columns_to_stack)
    }

    /// Unstack rows into columns.
    pub fn unstack(&self, index: &str, variable: &str, value: &str) -> Result<DataFrame> {
        unstack(self, index, variable, value)
    }

    /// Wide to long format.
    pub fn wide_to_long(
        &self,
        stub_names: &[&str],
        i: &str,
        j: &str,
        sep: &str,
    ) -> Result<DataFrame> {
        wide_to_long(self, stub_names, i, j, sep)
    }

    /// Cross tabulation.
    pub fn cross_tab(&self, index: &str, columns: &str) -> Result<DataFrame> {
        cross_tab(self, index, columns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pivot_table_sum() {
        let df = DataFrame::from_columns(vec![
            ("region", Series::new(vec!["east", "east", "west", "west"])),
            ("product", Series::new(vec!["a", "b", "a", "b"])),
            ("sales", Series::new(vec![10.0, 20.0, 30.0, 40.0])),
        ])
        .unwrap();

        let result = df
            .pivot_table("sales", "region", "product", AggFunc::Sum)
            .unwrap();
        assert_eq!(result.nrows(), 2);
        assert!(result.ncols() >= 3); // region + a + b
    }

    #[test]
    fn test_pivot_table_mean() {
        let df = DataFrame::from_columns(vec![
            ("region", Series::new(vec!["east", "east", "east", "west"])),
            ("product", Series::new(vec!["a", "a", "b", "a"])),
            ("sales", Series::new(vec![10.0, 20.0, 30.0, 40.0])),
        ])
        .unwrap();

        let result = df
            .pivot_table("sales", "region", "product", AggFunc::Mean)
            .unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_stack() {
        let df = DataFrame::from_columns(vec![
            ("id", Series::new(vec!["a", "b"])),
            ("x", Series::new(vec![1, 2])),
            ("y", Series::new(vec![3, 4])),
        ])
        .unwrap();

        let stacked = df.stack(&["x", "y"]).unwrap();
        assert_eq!(stacked.nrows(), 4);
        assert_eq!(stacked.ncols(), 3); // id, variable, value
    }

    #[test]
    fn test_unstack() {
        let df = DataFrame::from_columns(vec![
            ("id", Series::new(vec!["a", "a", "b", "b"])),
            ("variable", Series::new(vec!["x", "y", "x", "y"])),
            ("value", Series::new(vec![1, 2, 3, 4])),
        ])
        .unwrap();

        let unstacked = df.unstack("id", "variable", "value").unwrap();
        assert_eq!(unstacked.nrows(), 2);
    }

    #[test]
    fn test_wide_to_long() {
        let df = DataFrame::from_columns(vec![
            ("id", Series::new(vec!["a", "b"])),
            ("score_1", Series::new(vec![10, 20])),
            ("score_2", Series::new(vec![30, 40])),
        ])
        .unwrap();

        let long = df.wide_to_long(&["score"], "id", "suffix", "_").unwrap();
        assert_eq!(long.nrows(), 4);
        assert!(long.get("id").is_ok());
        assert!(long.get("suffix").is_ok());
        assert!(long.get("score").is_ok());
    }

    #[test]
    fn test_cross_tab() {
        let df = DataFrame::from_columns(vec![
            ("gender", Series::new(vec!["M", "F", "M", "F", "M"])),
            ("product", Series::new(vec!["a", "a", "b", "b", "a"])),
        ])
        .unwrap();

        let ct = df.cross_tab("gender", "product").unwrap();
        assert_eq!(ct.nrows(), 2); // F, M
    }

    #[test]
    fn test_pivot_table_count() {
        let df = DataFrame::from_columns(vec![
            ("dept", Series::new(vec!["eng", "eng", "sales", "sales"])),
            ("level", Series::new(vec!["jr", "sr", "jr", "sr"])),
            ("salary", Series::new(vec![50.0, 80.0, 40.0, 70.0])),
        ])
        .unwrap();

        let result = df
            .pivot_table("salary", "dept", "level", AggFunc::Count)
            .unwrap();
        assert_eq!(result.nrows(), 2);
    }
}
