//! Multi-sheet CSV I/O (Excel-like).
//!
//! Stores multiple DataFrames in a single file with sheet separators.
//!
//! File format:
//!   ---SHEET:name---
//!   <CSV data>
//!   ---SHEET:other_name---
//!   <CSV data>

use crate::frame::dataframe::DataFrame;
use crate::frame::error::{FrameError, Result};
use std::fs;
use std::path::Path;

const SHEET_PREFIX: &str = "---SHEET:";
const SHEET_SUFFIX: &str = "---";

/// A workbook containing multiple named sheets (DataFrames).
#[derive(Debug, Clone)]
pub struct Workbook {
    sheets: Vec<(String, DataFrame)>,
}

impl Workbook {
    /// Create an empty workbook.
    pub fn new() -> Self {
        Self { sheets: Vec::new() }
    }

    /// Add a sheet to the workbook.
    pub fn add_sheet(&mut self, name: &str, df: DataFrame) {
        // Replace if exists
        if let Some(pos) = self.sheets.iter().position(|(n, _)| n == name) {
            self.sheets[pos] = (name.to_string(), df);
        } else {
            self.sheets.push((name.to_string(), df));
        }
    }

    /// Get a sheet by name.
    pub fn get_sheet(&self, name: &str) -> Option<&DataFrame> {
        self.sheets
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, df)| df)
    }

    /// Get number of sheets.
    pub fn nsheets(&self) -> usize {
        self.sheets.len()
    }

    /// Get all sheet names.
    pub fn sheet_names(&self) -> Vec<&str> {
        self.sheets.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// Iterate over sheets.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &DataFrame)> {
        self.sheets.iter().map(|(n, df)| (n.as_str(), df))
    }

    /// Write workbook to a multi-sheet CSV file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        save_workbook(self, path)
    }

    /// Read workbook from a multi-sheet CSV file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        load_workbook(path)
    }

    /// Write to a string.
    pub fn to_string(&self) -> Result<String> {
        write_workbook_string(self)
    }

    /// Read from a string.
    pub fn from_string(s: &str) -> Result<Self> {
        read_workbook_string(s)
    }
}

impl Default for Workbook {
    fn default() -> Self {
        Self::new()
    }
}

/// Write a workbook to a file.
pub fn save_workbook<P: AsRef<Path>>(wb: &Workbook, path: P) -> Result<()> {
    let content = write_workbook_string(wb)?;
    fs::write(path, content).map_err(|e| FrameError::IoError(e.to_string()))
}

/// Read a workbook from a file.
pub fn load_workbook<P: AsRef<Path>>(path: P) -> Result<Workbook> {
    let content = fs::read_to_string(path).map_err(|e| FrameError::IoError(e.to_string()))?;
    read_workbook_string(&content)
}

/// Write workbook to string.
fn write_workbook_string(wb: &Workbook) -> Result<String> {
    let mut output = String::new();

    for (name, df) in &wb.sheets {
        output.push_str(&format!("{}{}{}\n", SHEET_PREFIX, name, SHEET_SUFFIX));

        // Write header
        output.push_str(&df.columns().join(","));
        output.push('\n');

        // Write data rows
        for i in 0..df.nrows() {
            let row: Vec<String> = df
                .columns()
                .iter()
                .map(|col| {
                    df.get(col)
                        .ok()
                        .and_then(|s| s.iloc(i).ok())
                        .map(format_value)
                        .unwrap_or_default()
                })
                .collect();
            output.push_str(&row.join(","));
            output.push('\n');
        }
    }

    Ok(output)
}

/// Read workbook from string.
fn read_workbook_string(s: &str) -> Result<Workbook> {
    let mut wb = Workbook::new();
    let mut current_sheet: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in s.lines() {
        if line.starts_with(SHEET_PREFIX) && line.ends_with(SHEET_SUFFIX) {
            // Save previous sheet
            if let Some(name) = current_sheet.take() {
                let df = parse_csv_lines(&current_lines)?;
                wb.add_sheet(&name, df);
                current_lines.clear();
            }

            // Extract sheet name
            let name = &line[SHEET_PREFIX.len()..line.len() - SHEET_SUFFIX.len()];
            current_sheet = Some(name.to_string());
        } else if current_sheet.is_some() {
            current_lines.push(line.to_string());
        }
    }

    // Save last sheet
    if let Some(name) = current_sheet {
        let df = parse_csv_lines(&current_lines)?;
        wb.add_sheet(&name, df);
    }

    Ok(wb)
}

/// Parse CSV lines into a DataFrame.
fn parse_csv_lines(lines: &[String]) -> Result<DataFrame> {
    use crate::frame::series::Series;
    use crate::frame::value::Value;

    if lines.is_empty() {
        return Ok(DataFrame::new());
    }

    // First line is header
    let header: Vec<String> = lines[0].split(',').map(|s| s.trim().to_string()).collect();
    let mut columns: Vec<Vec<Value>> = vec![Vec::new(); header.len()];

    // Data rows
    for line in &lines[1..] {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        for (i, field) in fields.iter().enumerate() {
            if i < header.len() {
                columns[i].push(parse_field(field.trim()));
            }
        }
        // Pad missing columns
        for col in columns.iter_mut().skip(fields.len()) {
            col.push(Value::Null);
        }
    }

    let series_cols: Vec<(&str, Series)> = header
        .iter()
        .zip(columns.into_iter())
        .map(|(name, data)| (name.as_str(), Series::new(data)))
        .collect();

    DataFrame::from_columns(series_cols)
}

/// Parse a single field value.
fn parse_field(s: &str) -> crate::frame::value::Value {
    use crate::frame::value::Value;

    if s.is_empty() || s == "null" {
        return Value::Null;
    }
    if let Ok(i) = s.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(s.to_string()),
    }
}

/// Format a value for CSV output.
fn format_value(v: &crate::frame::value::Value) -> String {
    use crate::frame::value::Value;
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => {
            if s.contains(',') || s.contains('\n') || s.contains('"') {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::series::Series;

    #[test]
    fn test_workbook_basic() {
        let mut wb = Workbook::new();
        let df1 = DataFrame::from_columns(vec![
            ("a", Series::new(vec![1, 2, 3])),
            ("b", Series::new(vec![4, 5, 6])),
        ])
        .unwrap();
        let df2 = DataFrame::from_columns(vec![("x", Series::new(vec![10.0, 20.0]))]).unwrap();

        wb.add_sheet("sheet1", df1);
        wb.add_sheet("sheet2", df2);

        assert_eq!(wb.nsheets(), 2);
        assert_eq!(wb.sheet_names(), vec!["sheet1", "sheet2"]);
        assert_eq!(wb.get_sheet("sheet1").unwrap().nrows(), 3);
        assert_eq!(wb.get_sheet("sheet2").unwrap().nrows(), 2);
    }

    #[test]
    fn test_workbook_roundtrip_string() {
        let mut wb = Workbook::new();
        wb.add_sheet(
            "data",
            DataFrame::from_columns(vec![
                ("name", Series::new(vec!["Alice", "Bob"])),
                ("age", Series::new(vec![25, 30])),
            ])
            .unwrap(),
        );
        wb.add_sheet(
            "scores",
            DataFrame::from_columns(vec![("score", Series::new(vec![95.5, 87.0, 92.3]))]).unwrap(),
        );

        let s = wb.to_string().unwrap();
        let wb2 = Workbook::from_string(&s).unwrap();

        assert_eq!(wb2.nsheets(), 2);
        assert_eq!(wb2.get_sheet("data").unwrap().nrows(), 2);
        assert_eq!(wb2.get_sheet("scores").unwrap().nrows(), 3);
    }

    #[test]
    fn test_workbook_file_roundtrip() {
        let mut wb = Workbook::new();
        wb.add_sheet(
            "test",
            DataFrame::from_columns(vec![("x", Series::new(vec![1, 2]))]).unwrap(),
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workbook.csv");

        wb.save(&path).unwrap();
        let wb2 = Workbook::load(&path).unwrap();

        assert_eq!(wb2.nsheets(), 1);
        assert_eq!(wb2.get_sheet("test").unwrap().nrows(), 2);
    }

    #[test]
    fn test_workbook_replace_sheet() {
        let mut wb = Workbook::new();
        wb.add_sheet(
            "data",
            DataFrame::from_columns(vec![("a", Series::new(vec![1]))]).unwrap(),
        );
        wb.add_sheet(
            "data",
            DataFrame::from_columns(vec![("b", Series::new(vec![2, 3]))]).unwrap(),
        );

        assert_eq!(wb.nsheets(), 1);
        assert_eq!(wb.get_sheet("data").unwrap().nrows(), 2);
    }

    #[test]
    fn test_workbook_empty() {
        let wb = Workbook::new();
        assert_eq!(wb.nsheets(), 0);
        let s = wb.to_string().unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn test_workbook_iter() {
        let mut wb = Workbook::new();
        wb.add_sheet(
            "a",
            DataFrame::from_columns(vec![("x", Series::new(vec![1]))]).unwrap(),
        );
        wb.add_sheet(
            "b",
            DataFrame::from_columns(vec![("y", Series::new(vec![2]))]).unwrap(),
        );

        let names: Vec<&str> = wb.iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["a", "b"]);
    }
}
