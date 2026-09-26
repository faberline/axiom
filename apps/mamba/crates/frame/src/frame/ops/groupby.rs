//! GroupBy operations for DataFrame.

use crate::frame::dataframe::DataFrame;
use crate::frame::error::Result;
use crate::frame::series::Series;
use crate::frame::value::Value;
use std::collections::HashMap;

/// GroupBy object for aggregation operations.
pub struct GroupBy<'a> {
    df: &'a DataFrame,
    by: Vec<String>,
    groups: HashMap<Vec<Value>, Vec<usize>>,
}

impl<'a> GroupBy<'a> {
    /// Create a new GroupBy from DataFrame and column names.
    pub fn new(df: &'a DataFrame, by: &[&str]) -> Result<Self> {
        // Validate columns exist
        for &col_name in by {
            df.get(col_name)?;
        }

        // Build groups
        let mut groups: HashMap<Vec<Value>, Vec<usize>> = HashMap::new();

        for i in 0..df.nrows() {
            let key: Vec<Value> = by
                .iter()
                .map(|&col_name| df.get(col_name).unwrap().iloc(i).unwrap().clone())
                .collect();

            groups.entry(key).or_default().push(i);
        }

        Ok(Self {
            df,
            by: by.iter().map(|s| s.to_string()).collect(),
            groups,
        })
    }

    /// Get number of groups.
    pub fn ngroups(&self) -> usize {
        self.groups.len()
    }

    /// Get group keys.
    pub fn groups(&self) -> &HashMap<Vec<Value>, Vec<usize>> {
        &self.groups
    }

    /// Sum aggregation.
    pub fn sum(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            let sum: f64 = indices
                .iter()
                .filter_map(|&i| series.iloc(i).ok()?.as_float())
                .sum();
            Value::Float(sum)
        })
    }

    /// Mean aggregation.
    pub fn mean(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            let vals: Vec<f64> = indices
                .iter()
                .filter_map(|&i| series.iloc(i).ok()?.as_float())
                .collect();
            if vals.is_empty() {
                Value::Null
            } else {
                Value::Float(vals.iter().sum::<f64>() / vals.len() as f64)
            }
        })
    }

    /// Count aggregation.
    pub fn count(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            let count = indices
                .iter()
                .filter(|&&i| !series.iloc(i).map(|v| v.is_null()).unwrap_or(true))
                .count();
            Value::Int(count as i64)
        })
    }

    /// Min aggregation.
    pub fn min(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            indices
                .iter()
                .filter_map(|&i| series.iloc(i).ok())
                .filter(|v| !v.is_null())
                .min()
                .cloned()
                .unwrap_or(Value::Null)
        })
    }

    /// Max aggregation.
    pub fn max(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            indices
                .iter()
                .filter_map(|&i| series.iloc(i).ok())
                .filter(|v| !v.is_null())
                .max()
                .cloned()
                .unwrap_or(Value::Null)
        })
    }

    /// First value in each group.
    pub fn first(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            indices
                .first()
                .and_then(|&i| series.iloc(i).ok())
                .cloned()
                .unwrap_or(Value::Null)
        })
    }

    /// Last value in each group.
    pub fn last(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            indices
                .last()
                .and_then(|&i| series.iloc(i).ok())
                .cloned()
                .unwrap_or(Value::Null)
        })
    }

    /// Apply custom aggregation function.
    pub fn aggregate<F>(&self, agg_fn: F) -> Result<DataFrame>
    where
        F: Fn(&Series, &[usize]) -> Value,
    {
        // Get non-groupby columns
        let agg_cols: Vec<&str> = self
            .df
            .columns()
            .iter()
            .filter(|c| !self.by.contains(c))
            .map(|s| s.as_str())
            .collect();

        // Materialize groups as a sorted vector to ensure stable, deterministic iteration order
        let mut sorted_groups: Vec<(&Vec<Value>, &Vec<usize>)> = self.groups.iter().collect();
        sorted_groups.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

        // Build result columns
        let mut result_columns: Vec<(&str, Vec<Value>)> = Vec::new();

        // Add groupby columns
        for by_col in &self.by {
            let values: Vec<Value> = sorted_groups
                .iter()
                .map(|(key, _)| {
                    let idx = self.by.iter().position(|b| b == by_col).unwrap();
                    key[idx].clone()
                })
                .collect();
            result_columns.push((by_col.as_str(), values));
        }

        // Add aggregated columns
        for &col_name in &agg_cols {
            let series = self.df.get(col_name)?;
            let values: Vec<Value> = sorted_groups
                .iter()
                .map(|(_, indices)| agg_fn(series, indices))
                .collect();
            result_columns.push((col_name, values));
        }

        // Build DataFrame
        let columns: Vec<_> = result_columns
            .into_iter()
            .map(|(name, values)| (name, Series::new(values)))
            .collect();

        DataFrame::from_columns(columns)
    }

    /// Transform: apply a function and return result aligned with original DataFrame.
    ///
    /// Unlike aggregate, transform returns a value for each row in the original DataFrame.
    ///
    /// # Example
    /// ```ignore
    /// // Z-score within each group
    /// let result = df.groupby(&["category"]).unwrap().transform(|series, indices| {
    ///     let vals: Vec<f64> = indices.iter()
    ///         .filter_map(|&i| series.iloc(i).ok()?.as_float())
    ///         .collect();
    ///     let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    ///     let std = (vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64).sqrt();
    ///     indices.iter().map(|&i| {
    ///         let v = series.iloc(i).ok()?.as_float()?;
    ///         Some(Value::Float((v - mean) / std))
    ///     }).collect()
    /// });
    /// ```
    pub fn transform<F>(&self, transform_fn: F) -> Result<DataFrame>
    where
        F: Fn(&Series, &[usize]) -> Vec<Option<Value>>,
    {
        // Get non-groupby columns
        let transform_cols: Vec<&str> = self
            .df
            .columns()
            .iter()
            .filter(|c| !self.by.contains(c))
            .map(|s| s.as_str())
            .collect();

        let n = self.df.nrows();

        // Build result columns with original data structure
        let mut result_columns: Vec<(&str, Vec<Value>)> = Vec::new();

        // Keep groupby columns
        for by_col in &self.by {
            let series = self.df.get(by_col)?;
            let values: Vec<Value> = (0..n)
                .map(|i| series.iloc(i).cloned().unwrap_or(Value::Null))
                .collect();
            result_columns.push((by_col.as_str(), values));
        }

        // Transform each column
        for &col_name in &transform_cols {
            let series = self.df.get(col_name)?;
            let mut result = vec![Value::Null; n];

            for indices in self.groups.values() {
                let transformed = transform_fn(series, indices);
                for (i, &row_idx) in indices.iter().enumerate() {
                    if let Some(Some(value)) = transformed.get(i) {
                        result[row_idx] = value.clone();
                    }
                }
            }

            result_columns.push((col_name, result));
        }

        let columns: Vec<_> = result_columns
            .into_iter()
            .map(|(name, values)| (name, Series::new(values)))
            .collect();

        DataFrame::from_columns(columns)
    }

    /// Filter: keep only groups that satisfy the predicate.
    ///
    /// # Example
    /// ```ignore
    /// // Keep groups with mean > 30
    /// let result = df.groupby(&["category"]).unwrap().filter(|_df, indices| {
    ///     let vals: Vec<f64> = indices.iter()
    ///         .filter_map(|&i| df.get("value").ok()?.iloc(i).ok()?.as_float())
    ///         .collect();
    ///     vals.iter().sum::<f64>() / vals.len() as f64 > 30.0
    /// });
    /// ```
    pub fn filter_groups<F>(&self, predicate: F) -> Result<DataFrame>
    where
        F: Fn(&DataFrame, &[usize]) -> bool,
    {
        let mut keep_indices: Vec<usize> = Vec::new();

        for indices in self.groups.values() {
            if predicate(self.df, indices) {
                keep_indices.extend(indices);
            }
        }

        // Sort to maintain original order
        keep_indices.sort_unstable();

        self.df.iloc(&keep_indices)
    }

    /// Compute variance within each group.
    pub fn var(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            let vals: Vec<f64> = indices
                .iter()
                .filter_map(|&i| series.iloc(i).ok()?.as_float())
                .collect();
            if vals.len() < 2 {
                return Value::Null;
            }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            let variance =
                vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
            Value::Float(variance)
        })
    }

    /// Compute standard deviation within each group.
    pub fn std(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            let vals: Vec<f64> = indices
                .iter()
                .filter_map(|&i| series.iloc(i).ok()?.as_float())
                .collect();
            if vals.len() < 2 {
                return Value::Null;
            }
            let mean = vals.iter().sum::<f64>() / vals.len() as f64;
            let variance =
                vals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (vals.len() - 1) as f64;
            Value::Float(variance.sqrt())
        })
    }

    /// Compute median within each group.
    pub fn median(&self) -> Result<DataFrame> {
        self.aggregate(|series, indices| {
            let mut vals: Vec<f64> = indices
                .iter()
                .filter_map(|&i| series.iloc(i).ok()?.as_float())
                .collect();
            if vals.is_empty() {
                return Value::Null;
            }
            vals.sort_by(|a, b| a.total_cmp(b));
            let mid = vals.len() / 2;
            if vals.len() % 2 == 0 {
                Value::Float((vals[mid - 1] + vals[mid]) / 2.0)
            } else {
                Value::Float(vals[mid])
            }
        })
    }

    /// Apply multiple aggregations.
    pub fn agg(&self, aggs: HashMap<&str, &str>) -> Result<DataFrame> {
        let mut result_columns: Vec<(&str, Vec<Value>)> = Vec::new();

        // Materialize groups as a sorted vector to ensure stable, deterministic iteration order
        let mut sorted_groups: Vec<(&Vec<Value>, &Vec<usize>)> = self.groups.iter().collect();
        sorted_groups.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

        // Add groupby columns
        for by_col in &self.by {
            let values: Vec<Value> = sorted_groups
                .iter()
                .map(|(key, _)| {
                    let idx = self.by.iter().position(|b| b == by_col).unwrap();
                    key[idx].clone()
                })
                .collect();
            result_columns.push((by_col.as_str(), values));
        }

        // Add aggregated columns
        for (&col_name, &agg_type) in &aggs {
            let series = self.df.get(col_name)?;
            let values: Vec<Value> = sorted_groups
                .iter()
                .map(|(_, indices)| match agg_type {
                    "sum" => {
                        let sum: f64 = indices
                            .iter()
                            .filter_map(|&i| series.iloc(i).ok()?.as_float())
                            .sum();
                        Value::Float(sum)
                    }
                    "mean" => {
                        let vals: Vec<f64> = indices
                            .iter()
                            .filter_map(|&i| series.iloc(i).ok()?.as_float())
                            .collect();
                        if vals.is_empty() {
                            Value::Null
                        } else {
                            Value::Float(vals.iter().sum::<f64>() / vals.len() as f64)
                        }
                    }
                    "count" => {
                        let count = indices
                            .iter()
                            .filter(|&&i| !series.iloc(i).map(|v| v.is_null()).unwrap_or(true))
                            .count();
                        Value::Int(count as i64)
                    }
                    "min" => indices
                        .iter()
                        .filter_map(|&i| series.iloc(i).ok())
                        .filter(|v| !v.is_null())
                        .min()
                        .cloned()
                        .unwrap_or(Value::Null),
                    "max" => indices
                        .iter()
                        .filter_map(|&i| series.iloc(i).ok())
                        .filter(|v| !v.is_null())
                        .max()
                        .cloned()
                        .unwrap_or(Value::Null),
                    "first" => indices
                        .first()
                        .and_then(|&i| series.iloc(i).ok())
                        .cloned()
                        .unwrap_or(Value::Null),
                    "last" => indices
                        .last()
                        .and_then(|&i| series.iloc(i).ok())
                        .cloned()
                        .unwrap_or(Value::Null),
                    _ => Value::Null,
                })
                .collect();
            result_columns.push((col_name, values));
        }

        let columns: Vec<_> = result_columns
            .into_iter()
            .map(|(name, values)| (name, Series::new(values)))
            .collect();

        DataFrame::from_columns(columns)
    }
}

// Extension trait for DataFrame
impl DataFrame {
    /// Group by columns.
    pub fn groupby(&self, by: &[&str]) -> Result<GroupBy<'_>> {
        GroupBy::new(self, by)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_df() -> DataFrame {
        DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "B", "A", "B", "A"])),
            ("value", Series::new(vec![10.0, 20.0, 30.0, 40.0, 50.0])),
        ])
        .unwrap()
    }

    #[test]
    fn test_groupby_sum() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().sum().unwrap();

        assert_eq!(result.nrows(), 2);
        // A: 10 + 30 + 50 = 90, B: 20 + 40 = 60
    }

    #[test]
    fn test_groupby_mean() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().mean().unwrap();

        assert_eq!(result.nrows(), 2);
        // A: (10 + 30 + 50) / 3 = 30, B: (20 + 40) / 2 = 30
    }

    #[test]
    fn test_groupby_count() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().count().unwrap();

        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_ngroups() {
        let df = create_test_df();
        let gb = df.groupby(&["category"]).unwrap();
        assert_eq!(gb.ngroups(), 2);
    }

    // Phase 2 GroupBy tests
    #[test]
    fn test_groupby_var() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("value", Series::new(vec![1.0, 3.0, 10.0, 20.0])),
        ])
        .unwrap();
        let result = df.groupby(&["category"]).unwrap().var().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_std() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("value", Series::new(vec![1.0, 3.0, 10.0, 20.0])),
        ])
        .unwrap();
        let result = df.groupby(&["category"]).unwrap().std().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_median() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "A", "B", "B"])),
            ("value", Series::new(vec![1.0, 2.0, 3.0, 10.0, 20.0])),
        ])
        .unwrap();
        let result = df.groupby(&["category"]).unwrap().median().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_transform() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("value", Series::new(vec![10.0, 20.0, 100.0, 200.0])),
        ])
        .unwrap();

        // Transform: demean within group
        let result = df
            .groupby(&["category"])
            .unwrap()
            .transform(|series, indices| {
                let vals: Vec<f64> = indices
                    .iter()
                    .filter_map(|&i| series.iloc(i).ok()?.as_float())
                    .collect();
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                indices
                    .iter()
                    .map(|&i| {
                        series
                            .iloc(i)
                            .ok()?
                            .as_float()
                            .map(|v| Value::Float(v - mean))
                    })
                    .collect()
            })
            .unwrap();

        assert_eq!(result.nrows(), 4);
    }

    #[test]
    fn test_groupby_filter() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("value", Series::new(vec![10.0, 20.0, 100.0, 200.0])),
        ])
        .unwrap();

        // Filter: keep groups with mean > 50
        let result = df
            .groupby(&["category"])
            .unwrap()
            .filter_groups(|df, indices| {
                let vals: Vec<f64> = indices
                    .iter()
                    .filter_map(|&i| df.get("value").ok()?.iloc(i).ok()?.as_float())
                    .collect();
                vals.iter().sum::<f64>() / vals.len() as f64 > 50.0
            })
            .unwrap();

        assert_eq!(result.nrows(), 2); // Only group B
    }

    #[test]
    fn test_groupby_min() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().min().unwrap();
        assert_eq!(result.nrows(), 2);
        // A: min(10, 30, 50) = 10, B: min(20, 40) = 20
    }

    #[test]
    fn test_groupby_max() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().max().unwrap();
        assert_eq!(result.nrows(), 2);
        // A: max(10, 30, 50) = 50, B: max(20, 40) = 40
    }

    #[test]
    fn test_groupby_first() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().first().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_last() {
        let df = create_test_df();
        let result = df.groupby(&["category"]).unwrap().last().unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_groups() {
        let df = create_test_df();
        let gb = df.groupby(&["category"]).unwrap();
        let groups = gb.groups();
        assert_eq!(groups.len(), 2);
        // Should have keys for A and B
    }

    #[test]
    fn test_groupby_aggregate() {
        let df = create_test_df();
        let result = df
            .groupby(&["category"])
            .unwrap()
            .aggregate(|series, indices| {
                let sum: f64 = indices
                    .iter()
                    .filter_map(|&i| series.iloc(i).ok()?.as_float())
                    .sum();
                Value::Float(sum * 2.0) // Custom: double the sum
            })
            .unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_agg() {
        let df = create_test_df();
        let mut aggs = HashMap::new();
        aggs.insert("value", "sum");
        let result = df.groupby(&["category"]).unwrap().agg(aggs).unwrap();
        assert_eq!(result.nrows(), 2);
    }

    #[test]
    fn test_groupby_agg_multiple() {
        let df = DataFrame::from_columns(vec![
            ("category", Series::new(vec!["A", "A", "B", "B"])),
            ("x", Series::new(vec![1.0, 2.0, 3.0, 4.0])),
            ("y", Series::new(vec![10.0, 20.0, 30.0, 40.0])),
        ])
        .unwrap();
        let mut aggs = HashMap::new();
        aggs.insert("x", "mean");
        aggs.insert("y", "sum");
        let result = df.groupby(&["category"]).unwrap().agg(aggs).unwrap();
        assert_eq!(result.nrows(), 2);
    }
}
