//! Columnar serialization format (Parquet-like).
//!
//! A simple binary columnar format for fast serialization/deserialization:
//!
//! File layout:
//!   - Magic bytes: "CCLF" (4 bytes)
//!   - Version: u8 (1 byte)
//!   - Number of columns: u32 LE (4 bytes)
//!   - Number of rows: u64 LE (8 bytes)
//!   - For each column:
//!     - Column name length: u16 LE
//!     - Column name: UTF-8 bytes
//!     - Column type tag: u8 (0=null, 1=bool, 2=int, 3=float, 4=string)
//!     - Null bitmap: ceil(nrows/8) bytes
//!     - Data (type-dependent):
//!       - bool: ceil(nrows/8) bytes as bitmap
//!       - int: nrows * 8 bytes (i64 LE)
//!       - float: nrows * 8 bytes (f64 LE)
//!       - string: for each row: u32 LE length + UTF-8 bytes

use crate::frame::dataframe::DataFrame;
use crate::frame::error::{FrameError, Result};
use crate::frame::series::Series;
use crate::frame::value::Value;
use std::io::{Read, Write};
use std::path::Path;

const MAGIC: &[u8; 4] = b"CCLF";
const VERSION: u8 = 1;

// Type tags
const TYPE_NULL: u8 = 0;
const TYPE_BOOL: u8 = 1;
const TYPE_INT: u8 = 2;
const TYPE_FLOAT: u8 = 3;
const TYPE_STRING: u8 = 4;

/// Detect the dominant non-null type in a series.
fn detect_type(values: &[Value]) -> u8 {
    let mut int_count = 0;
    let mut float_count = 0;
    let mut string_count = 0;
    let mut bool_count = 0;

    for v in values {
        match v {
            Value::Int(_) => int_count += 1,
            Value::Float(_) => float_count += 1,
            Value::String(_) => string_count += 1,
            Value::Bool(_) => bool_count += 1,
            Value::Null => {}
        }
    }

    if string_count > 0 {
        TYPE_STRING
    } else if float_count > 0 {
        TYPE_FLOAT
    } else if int_count > 0 {
        TYPE_INT
    } else if bool_count > 0 {
        TYPE_BOOL
    } else {
        TYPE_NULL
    }
}

/// Write a DataFrame in columnar format.
pub fn write_columnar<W: Write>(df: &DataFrame, writer: &mut W) -> Result<()> {
    let nrows = df.nrows();
    let ncols = df.ncols();

    // Header
    writer.write_all(MAGIC).map_err(io_err)?;
    writer.write_all(&[VERSION]).map_err(io_err)?;
    writer
        .write_all(&(ncols as u32).to_le_bytes())
        .map_err(io_err)?;
    writer
        .write_all(&(nrows as u64).to_le_bytes())
        .map_err(io_err)?;

    // Columns
    for col_name in df.columns() {
        let series = df.get(col_name)?;
        let values = series.values();

        // Column name
        let name_bytes = col_name.as_bytes();
        writer
            .write_all(&(name_bytes.len() as u16).to_le_bytes())
            .map_err(io_err)?;
        writer.write_all(name_bytes).map_err(io_err)?;

        // Type tag
        let type_tag = detect_type(values);
        writer.write_all(&[type_tag]).map_err(io_err)?;

        // Null bitmap
        let bitmap_len = (nrows + 7) / 8;
        let mut bitmap = vec![0u8; bitmap_len];
        for (i, v) in values.iter().enumerate() {
            if !v.is_null() {
                bitmap[i / 8] |= 1 << (i % 8);
            }
        }
        writer.write_all(&bitmap).map_err(io_err)?;

        // Data
        write_column_data(writer, values, type_tag, nrows)?;
    }

    Ok(())
}

/// Write column data based on type.
fn write_column_data<W: Write>(
    writer: &mut W,
    values: &[Value],
    type_tag: u8,
    nrows: usize,
) -> Result<()> {
    match type_tag {
        TYPE_BOOL => {
            let bitmap_len = (nrows + 7) / 8;
            let mut bitmap = vec![0u8; bitmap_len];
            for (i, v) in values.iter().enumerate() {
                if let Some(true) = v.as_bool() {
                    bitmap[i / 8] |= 1 << (i % 8);
                }
            }
            writer.write_all(&bitmap).map_err(io_err)?;
        }
        TYPE_INT => {
            for v in values {
                let val = v.as_int().unwrap_or(0);
                writer.write_all(&val.to_le_bytes()).map_err(io_err)?;
            }
        }
        TYPE_FLOAT => {
            for v in values {
                let val = v.as_float().unwrap_or(0.0);
                writer.write_all(&val.to_le_bytes()).map_err(io_err)?;
            }
        }
        TYPE_STRING => {
            for v in values {
                let s = match v {
                    Value::String(s) => s.as_str(),
                    Value::Null => "",
                    other => {
                        // Inline the string representation
                        let owned = other.to_string();
                        let bytes = owned.as_bytes();
                        writer
                            .write_all(&(bytes.len() as u32).to_le_bytes())
                            .map_err(io_err)?;
                        writer.write_all(bytes).map_err(io_err)?;
                        continue;
                    }
                };
                let bytes = s.as_bytes();
                writer
                    .write_all(&(bytes.len() as u32).to_le_bytes())
                    .map_err(io_err)?;
                writer.write_all(bytes).map_err(io_err)?;
            }
        }
        _ => {
            // TYPE_NULL: no data to write
        }
    }
    Ok(())
}

/// Read a DataFrame from columnar format.
pub fn read_columnar<R: Read>(reader: &mut R) -> Result<DataFrame> {
    // Magic
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).map_err(io_err)?;
    if &magic != MAGIC {
        return Err(FrameError::InvalidOperation(
            "not a valid columnar file (bad magic)".into(),
        ));
    }

    // Version
    let mut version = [0u8; 1];
    reader.read_exact(&mut version).map_err(io_err)?;
    if version[0] != VERSION {
        return Err(FrameError::InvalidOperation(format!(
            "unsupported version: {}",
            version[0]
        )));
    }

    // Header
    let mut buf4 = [0u8; 4];
    reader.read_exact(&mut buf4).map_err(io_err)?;
    let ncols = u32::from_le_bytes(buf4) as usize;

    let mut buf8 = [0u8; 8];
    reader.read_exact(&mut buf8).map_err(io_err)?;
    let nrows = u64::from_le_bytes(buf8) as usize;

    // Columns
    let mut columns: Vec<(&str, Series)> = Vec::with_capacity(ncols);
    let mut names: Vec<String> = Vec::with_capacity(ncols);

    for _ in 0..ncols {
        // Column name
        let mut name_len_buf = [0u8; 2];
        reader.read_exact(&mut name_len_buf).map_err(io_err)?;
        let name_len = u16::from_le_bytes(name_len_buf) as usize;

        let mut name_buf = vec![0u8; name_len];
        reader.read_exact(&mut name_buf).map_err(io_err)?;
        let col_name =
            String::from_utf8(name_buf).map_err(|e| FrameError::IoError(e.to_string()))?;

        // Type tag
        let mut type_buf = [0u8; 1];
        reader.read_exact(&mut type_buf).map_err(io_err)?;
        let type_tag = type_buf[0];

        // Null bitmap
        let bitmap_len = (nrows + 7) / 8;
        let mut bitmap = vec![0u8; bitmap_len];
        reader.read_exact(&mut bitmap).map_err(io_err)?;

        // Data
        let values = read_column_data(reader, type_tag, nrows, &bitmap)?;

        names.push(col_name);
        columns.push(("", Series::new(values)));
    }

    // Fix column name references (need to point to owned strings)
    let cols_with_names: Vec<(&str, Series)> = names
        .iter()
        .zip(columns.into_iter().map(|(_, s)| s))
        .map(|(name, s)| (name.as_str(), s))
        .collect();

    DataFrame::from_columns(cols_with_names)
}

/// Read column data based on type.
fn read_column_data<R: Read>(
    reader: &mut R,
    type_tag: u8,
    nrows: usize,
    bitmap: &[u8],
) -> Result<Vec<Value>> {
    let is_not_null = |i: usize| -> bool {
        bitmap
            .get(i / 8)
            .map(|b| b & (1 << (i % 8)) != 0)
            .unwrap_or(false)
    };

    match type_tag {
        TYPE_BOOL => {
            let data_bitmap_len = (nrows + 7) / 8;
            let mut data_bitmap = vec![0u8; data_bitmap_len];
            reader.read_exact(&mut data_bitmap).map_err(io_err)?;

            let values: Vec<Value> = (0..nrows)
                .map(|i| {
                    if is_not_null(i) {
                        let val = data_bitmap
                            .get(i / 8)
                            .map(|b| b & (1 << (i % 8)) != 0)
                            .unwrap_or(false);
                        Value::Bool(val)
                    } else {
                        Value::Null
                    }
                })
                .collect();
            Ok(values)
        }
        TYPE_INT => {
            let mut buf = vec![0u8; nrows * 8];
            reader.read_exact(&mut buf).map_err(io_err)?;

            let values: Vec<Value> = (0..nrows)
                .map(|i| {
                    if is_not_null(i) {
                        let bytes: [u8; 8] = buf[i * 8..(i + 1) * 8].try_into().unwrap();
                        Value::Int(i64::from_le_bytes(bytes))
                    } else {
                        Value::Null
                    }
                })
                .collect();
            Ok(values)
        }
        TYPE_FLOAT => {
            let mut buf = vec![0u8; nrows * 8];
            reader.read_exact(&mut buf).map_err(io_err)?;

            let values: Vec<Value> = (0..nrows)
                .map(|i| {
                    if is_not_null(i) {
                        let bytes: [u8; 8] = buf[i * 8..(i + 1) * 8].try_into().unwrap();
                        Value::Float(f64::from_le_bytes(bytes))
                    } else {
                        Value::Null
                    }
                })
                .collect();
            Ok(values)
        }
        TYPE_STRING => {
            let mut values = Vec::with_capacity(nrows);
            for i in 0..nrows {
                let mut len_buf = [0u8; 4];
                reader.read_exact(&mut len_buf).map_err(io_err)?;
                let str_len = u32::from_le_bytes(len_buf) as usize;

                let mut str_buf = vec![0u8; str_len];
                reader.read_exact(&mut str_buf).map_err(io_err)?;

                if is_not_null(i) {
                    let s = String::from_utf8(str_buf)
                        .map_err(|e| FrameError::IoError(e.to_string()))?;
                    values.push(Value::String(s));
                } else {
                    values.push(Value::Null);
                }
            }
            Ok(values)
        }
        _ => {
            // TYPE_NULL: all nulls
            Ok(vec![Value::Null; nrows])
        }
    }
}

fn io_err(e: std::io::Error) -> FrameError {
    FrameError::IoError(e.to_string())
}

/// Write DataFrame to a columnar file.
pub fn save_columnar<P: AsRef<Path>>(df: &DataFrame, path: P) -> Result<()> {
    let file = std::fs::File::create(path).map_err(io_err)?;
    let mut writer = std::io::BufWriter::new(file);
    write_columnar(df, &mut writer)
}

/// Read DataFrame from a columnar file.
pub fn load_columnar<P: AsRef<Path>>(path: P) -> Result<DataFrame> {
    let file = std::fs::File::open(path).map_err(io_err)?;
    let mut reader = std::io::BufReader::new(file);
    read_columnar(&mut reader)
}

// Extension trait for DataFrame
impl DataFrame {
    /// Write to columnar format file.
    pub fn to_columnar<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        save_columnar(self, path)
    }

    /// Read from columnar format file.
    pub fn read_columnar<P: AsRef<Path>>(path: P) -> Result<DataFrame> {
        load_columnar(path)
    }

    /// Serialize to columnar bytes.
    pub fn to_columnar_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        write_columnar(self, &mut buf)?;
        Ok(buf)
    }

    /// Deserialize from columnar bytes.
    pub fn from_columnar_bytes(bytes: &[u8]) -> Result<DataFrame> {
        let mut cursor = std::io::Cursor::new(bytes);
        read_columnar(&mut cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_ints() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1i64, 2, 3])),
            ("b", Series::new(vec![10i64, 20, 30])),
        ])
        .unwrap();

        let bytes = df.to_columnar_bytes().unwrap();
        let df2 = DataFrame::from_columnar_bytes(&bytes).unwrap();
        assert_eq!(df2.nrows(), 3);
        assert_eq!(df2.ncols(), 2);
        assert_eq!(df2.get("a").unwrap().iloc(0).unwrap(), &Value::Int(1));
        assert_eq!(df2.get("b").unwrap().iloc(2).unwrap(), &Value::Int(30));
    }

    #[test]
    fn test_roundtrip_floats() {
        let df = DataFrame::from_columns(vec![("x", Series::new(vec![1.5, 2.5, 3.5]))]).unwrap();

        let bytes = df.to_columnar_bytes().unwrap();
        let df2 = DataFrame::from_columnar_bytes(&bytes).unwrap();
        assert_eq!(df2.get("x").unwrap().iloc(1).unwrap(), &Value::Float(2.5));
    }

    #[test]
    fn test_roundtrip_strings() {
        let df =
            DataFrame::from_columns(vec![("name", Series::new(vec!["Alice", "Bob", "Charlie"]))])
                .unwrap();

        let bytes = df.to_columnar_bytes().unwrap();
        let df2 = DataFrame::from_columnar_bytes(&bytes).unwrap();
        assert_eq!(
            df2.get("name").unwrap().iloc(0).unwrap(),
            &Value::String("Alice".into())
        );
    }

    #[test]
    fn test_roundtrip_bools() {
        let df =
            DataFrame::from_columns(vec![("flag", Series::new(vec![true, false, true]))]).unwrap();

        let bytes = df.to_columnar_bytes().unwrap();
        let df2 = DataFrame::from_columnar_bytes(&bytes).unwrap();
        assert_eq!(
            df2.get("flag").unwrap().iloc(0).unwrap(),
            &Value::Bool(true)
        );
        assert_eq!(
            df2.get("flag").unwrap().iloc(1).unwrap(),
            &Value::Bool(false)
        );
    }

    #[test]
    fn test_roundtrip_with_nulls() {
        let df = DataFrame::from_columns(vec![
            (
                "a",
                Series::new(vec![Value::Int(1), Value::Null, Value::Int(3)]),
            ),
            (
                "b",
                Series::new(vec![Value::Float(1.0), Value::Float(2.0), Value::Null]),
            ),
        ])
        .unwrap();

        let bytes = df.to_columnar_bytes().unwrap();
        let df2 = DataFrame::from_columnar_bytes(&bytes).unwrap();
        assert_eq!(df2.nrows(), 3);
        assert!(df2.get("a").unwrap().iloc(1).unwrap().is_null());
        assert!(df2.get("b").unwrap().iloc(2).unwrap().is_null());
        assert_eq!(df2.get("a").unwrap().iloc(0).unwrap(), &Value::Int(1));
    }

    #[test]
    fn test_roundtrip_mixed_types() {
        let df = DataFrame::from_columns(vec![
            ("id", Series::new(vec![1i64, 2, 3])),
            ("score", Series::new(vec![95.5, 87.0, 92.3])),
            ("name", Series::new(vec!["Alice", "Bob", "Charlie"])),
        ])
        .unwrap();

        let bytes = df.to_columnar_bytes().unwrap();
        let df2 = DataFrame::from_columnar_bytes(&bytes).unwrap();
        assert_eq!(df2.nrows(), 3);
        assert_eq!(df2.ncols(), 3);
    }

    #[test]
    fn test_roundtrip_empty() {
        let df = DataFrame::new();
        let bytes = df.to_columnar_bytes().unwrap();
        let df2 = DataFrame::from_columnar_bytes(&bytes).unwrap();
        assert!(df2.is_empty());
    }

    #[test]
    fn test_file_roundtrip() {
        let df = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2, 3])),
            ("b", Series::new(vec![4.0, 5.0, 6.0])),
        ])
        .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.cclf");

        df.to_columnar(&path).unwrap();
        let df2 = DataFrame::read_columnar(&path).unwrap();
        assert_eq!(df2.nrows(), 3);
        assert_eq!(df2.ncols(), 2);
    }

    #[test]
    fn test_bad_magic() {
        let bytes = b"BAAD\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        let result = DataFrame::from_columnar_bytes(bytes);
        assert!(result.is_err());
    }
}
