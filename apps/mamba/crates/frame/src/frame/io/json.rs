//! JSON reader and writer for DataFrame.
//!
//! Uses workspace `serde_json` for parsing/serialization, converting between
//! `serde_json::Value` and pulsar's `Value` type.

use crate::frame::dataframe::DataFrame;
use crate::frame::error::{FrameError, Result};
use crate::frame::series::Series;
use crate::frame::value::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// JSON orientation for serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JsonOrient {
    /// `[{"col1": val1, "col2": val2}, ...]`
    #[default]
    Records,
    /// `{"col1": [val1, ...], "col2": [val2, ...]}`
    Columns,
    /// `[[val1, val2], [val3, val4]]`
    Values,
}

/// Options for JSON serialization.
#[derive(Debug, Clone)]
pub struct JsonOptions {
    pub orient: JsonOrient,
    pub pretty: bool,
}

impl Default for JsonOptions {
    fn default() -> Self {
        Self {
            orient: JsonOrient::Records,
            pretty: false,
        }
    }
}

// ============================================================================
// serde_json::Value <-> pulsar Value
// ============================================================================

fn from_json_value(jv: &serde_json::Value) -> Value {
    match jv {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        // Nested arrays/objects → stringify (no nested Value variants in pulsar)
        other => Value::String(serde_json::to_string(other).unwrap_or_default()),
    }
}

fn to_json_value(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::Value::Number((*i).into()),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(s) => serde_json::Value::String(s.clone()),
    }
}

// ============================================================================
// Flatten nested objects with dot-notation
// ============================================================================

fn flatten_object(
    prefix: &str,
    obj: &serde_json::Map<String, serde_json::Value>,
    out: &mut Vec<(String, Value)>,
) {
    for (key, val) in obj {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };
        if let serde_json::Value::Object(nested) = val {
            flatten_object(&full_key, nested, out);
        } else {
            out.push((full_key, from_json_value(val)));
        }
    }
}

// ============================================================================
// Read
// ============================================================================

/// Read a JSON file into a DataFrame.
pub fn read_json<P: AsRef<Path>>(path: P) -> Result<DataFrame> {
    let content = fs::read_to_string(path).map_err(|e| FrameError::IoError(e.to_string()))?;
    read_json_str(&content)
}

/// Read a JSON string into a DataFrame.
pub fn read_json_str(json: &str) -> Result<DataFrame> {
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|e| FrameError::InvalidOperation(e.to_string()))?;

    match &parsed {
        serde_json::Value::Array(arr) => parse_records_array(arr),
        serde_json::Value::Object(obj) => parse_columns_object(obj),
        _ => Err(FrameError::InvalidOperation(
            "JSON must be an array or object".into(),
        )),
    }
}

fn parse_records_array(arr: &[serde_json::Value]) -> Result<DataFrame> {
    if arr.is_empty() {
        return Ok(DataFrame::new());
    }

    let mut col_names: Vec<String> = Vec::new();
    let mut col_set = std::collections::HashSet::new();
    let mut flat_records: Vec<HashMap<String, Value>> = Vec::with_capacity(arr.len());

    for item in arr {
        let obj = item
            .as_object()
            .ok_or_else(|| FrameError::InvalidOperation("expected object in array".into()))?;

        let mut pairs = Vec::new();
        flatten_object("", obj, &mut pairs);

        let mut record = HashMap::new();
        for (key, val) in pairs {
            if col_set.insert(key.clone()) {
                col_names.push(key.clone());
            }
            record.insert(key, val);
        }
        flat_records.push(record);
    }

    let columns: Vec<(&str, Series)> = col_names
        .iter()
        .map(|name| {
            let data: Vec<Value> = flat_records
                .iter()
                .map(|r| r.get(name).cloned().unwrap_or(Value::Null))
                .collect();
            (name.as_str(), Series::new(data))
        })
        .collect();

    DataFrame::from_columns(columns)
}

fn parse_columns_object(obj: &serde_json::Map<String, serde_json::Value>) -> Result<DataFrame> {
    if obj.is_empty() {
        return Ok(DataFrame::new());
    }

    let mut columns = Vec::new();
    for (key, val) in obj {
        let arr = val
            .as_array()
            .ok_or_else(|| FrameError::InvalidOperation("expected array for column".into()))?;
        let data: Vec<Value> = arr.iter().map(|v| from_json_value(v)).collect();
        columns.push((key.as_str(), Series::new(data)));
    }

    DataFrame::from_columns(columns)
}

// ============================================================================
// Write
// ============================================================================

/// Write a DataFrame to a JSON string.
pub fn write_json_str(df: &DataFrame, options: &JsonOptions) -> Result<String> {
    let jv = match options.orient {
        JsonOrient::Records => build_records(df)?,
        JsonOrient::Columns => build_columns(df)?,
        JsonOrient::Values => build_values(df)?,
    };

    let out = if options.pretty {
        serde_json::to_string_pretty(&jv)
    } else {
        serde_json::to_string(&jv)
    };
    out.map_err(|e| FrameError::IoError(e.to_string()))
}

/// Save a DataFrame as JSON to a file.
pub fn save_json<P: AsRef<Path>>(df: &DataFrame, path: P, options: &JsonOptions) -> Result<()> {
    let json = write_json_str(df, options)?;
    fs::write(path, json).map_err(|e| FrameError::IoError(e.to_string()))
}

fn build_records(df: &DataFrame) -> Result<serde_json::Value> {
    let mut records = Vec::with_capacity(df.nrows());
    for i in 0..df.nrows() {
        let mut obj = serde_json::Map::new();
        for col_name in df.columns() {
            let val = df
                .get(col_name)
                .and_then(|s| s.iloc(i).cloned())
                .unwrap_or(Value::Null);
            obj.insert(col_name.clone(), to_json_value(&val));
        }
        records.push(serde_json::Value::Object(obj));
    }
    Ok(serde_json::Value::Array(records))
}

fn build_columns(df: &DataFrame) -> Result<serde_json::Value> {
    let mut obj = serde_json::Map::new();
    for col_name in df.columns() {
        let col = df.get(col_name)?;
        let arr: Vec<serde_json::Value> = (0..col.len())
            .map(|i| to_json_value(&col.iloc(i).cloned().unwrap_or(Value::Null)))
            .collect();
        obj.insert(col_name.clone(), serde_json::Value::Array(arr));
    }
    Ok(serde_json::Value::Object(obj))
}

fn build_values(df: &DataFrame) -> Result<serde_json::Value> {
    let mut rows = Vec::with_capacity(df.nrows());
    for i in 0..df.nrows() {
        let row: Vec<serde_json::Value> = df
            .columns()
            .iter()
            .map(|name| {
                let val = df
                    .get(name)
                    .and_then(|s| s.iloc(i).cloned())
                    .unwrap_or(Value::Null);
                to_json_value(&val)
            })
            .collect();
        rows.push(serde_json::Value::Array(row));
    }
    Ok(serde_json::Value::Array(rows))
}

// ============================================================================
// DataFrame extension
// ============================================================================

impl DataFrame {
    /// Read from JSON file.
    pub fn read_json<P: AsRef<Path>>(path: P) -> Result<DataFrame> {
        read_json(path)
    }

    /// Read from JSON string.
    pub fn from_json(json: &str) -> Result<DataFrame> {
        read_json_str(json)
    }

    /// Write to JSON string.
    pub fn to_json(&self, options: &JsonOptions) -> Result<String> {
        write_json_str(self, options)
    }

    /// Save to JSON file.
    pub fn save_json<P: AsRef<Path>>(&self, path: P, options: &JsonOptions) -> Result<()> {
        save_json(self, path, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_records() {
        let json = r#"[{"id": 1, "val": 10}, {"id": 2, "val": 20}]"#;
        let df = read_json_str(json).unwrap();
        assert_eq!(df.nrows(), 2);
        assert_eq!(df.ncols(), 2);
    }

    #[test]
    fn test_read_columns() {
        let json = r#"{"id": [1, 2, 3], "val": [10, 20, 30]}"#;
        let df = read_json_str(json).unwrap();
        assert_eq!(df.nrows(), 3);
        assert_eq!(df.ncols(), 2);
    }

    #[test]
    fn test_roundtrip_records() {
        let df = DataFrame::from_columns(vec![
            ("name", Series::new(vec!["Alice", "Bob"])),
            ("age", Series::new(vec![25, 30])),
        ])
        .unwrap();
        let json = write_json_str(&df, &JsonOptions::default()).unwrap();
        let df2 = read_json_str(&json).unwrap();
        assert_eq!(df2.nrows(), 2);
        assert_eq!(df2.ncols(), 2);
    }

    #[test]
    fn test_roundtrip_columns() {
        let df = DataFrame::from_columns(vec![("x", Series::new(vec![1.0, 2.0, 3.0]))]).unwrap();
        let opts = JsonOptions {
            orient: JsonOrient::Columns,
            pretty: false,
        };
        let json = write_json_str(&df, &opts).unwrap();
        let df2 = read_json_str(&json).unwrap();
        assert_eq!(df2.nrows(), 3);
    }

    #[test]
    fn test_empty() {
        assert!(read_json_str("[]").unwrap().is_empty());
        assert!(read_json_str("{}").unwrap().is_empty());
    }

    #[test]
    fn test_nulls() {
        let json = r#"[{"a": 1, "b": null}, {"a": null, "b": 2}]"#;
        let df = read_json_str(json).unwrap();
        assert!(df.get("b").unwrap().iloc(0).unwrap().is_null());
    }

    #[test]
    fn test_values_orient() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2])),
            ("b", Series::new(vec![3, 4])),
        ])
        .unwrap();
        let opts = JsonOptions {
            orient: JsonOrient::Values,
            pretty: false,
        };
        let json = write_json_str(&df, &opts).unwrap();
        assert!(json.starts_with("[["));
    }

    #[test]
    fn test_nested_flatten() {
        let json = r#"[{"meta": {"id": 1, "tag": "x"}, "val": 10}]"#;
        let df = read_json_str(json).unwrap();
        assert!(df.get("meta.id").is_ok());
        assert_eq!(df.get("meta.id").unwrap().iloc(0).unwrap(), &Value::Int(1));
    }

    #[test]
    fn test_deeply_nested() {
        let json = r#"[{"a": {"b": {"c": 42}}, "x": 1}]"#;
        let df = read_json_str(json).unwrap();
        assert_eq!(df.get("a.b.c").unwrap().iloc(0).unwrap(), &Value::Int(42));
    }
}
