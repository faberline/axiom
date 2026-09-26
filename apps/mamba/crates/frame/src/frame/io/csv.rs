//! CSV reader and writer for DataFrame.

use crate::frame::dataframe::DataFrame;
use crate::frame::error::{FrameError, Result};
use crate::frame::series::Series;
use crate::frame::value::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// CSV parsing options.
#[derive(Debug, Clone)]
pub struct CsvOptions {
    /// Delimiter character.
    pub delimiter: char,
    /// Whether the first row is a header.
    pub has_header: bool,
    /// Quote character.
    pub quote: char,
    /// Skip blank lines.
    pub skip_blank_lines: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: ',',
            has_header: true,
            quote: '"',
            skip_blank_lines: true,
        }
    }
}

/// Read a CSV file into a DataFrame.
pub fn read_csv<P: AsRef<Path>>(path: P) -> Result<DataFrame> {
    read_csv_with_options(path, CsvOptions::default())
}

/// Read a CSV file with custom options.
pub fn read_csv_with_options<P: AsRef<Path>>(path: P, options: CsvOptions) -> Result<DataFrame> {
    let file = File::open(path).map_err(|e| FrameError::IoError(e.to_string()))?;
    let reader = BufReader::new(file);

    let mut lines = reader.lines();
    let mut columns: Vec<Vec<Value>> = Vec::new();
    let mut column_names: Vec<String> = Vec::new();

    // Read header
    if options.has_header {
        if let Some(line) = lines.next() {
            let line = line.map_err(|e| FrameError::IoError(e.to_string()))?;
            column_names = parse_csv_line(&line, options.delimiter, options.quote);
            columns = vec![Vec::new(); column_names.len()];
        }
    }

    // Read data rows
    for line_result in lines {
        let line = line_result.map_err(|e| FrameError::IoError(e.to_string()))?;

        if options.skip_blank_lines && line.trim().is_empty() {
            continue;
        }

        let fields = parse_csv_line(&line, options.delimiter, options.quote);

        // If no header, create column names on first data row
        if column_names.is_empty() {
            column_names = (0..fields.len()).map(|i| format!("col_{}", i)).collect();
            columns = vec![Vec::new(); fields.len()];
        }

        // Add values to columns
        let num_cols = columns.len();
        for (i, field) in fields.into_iter().enumerate() {
            if i < num_cols {
                columns[i].push(parse_value(&field));
            }
        }

        // Pad missing columns with null
        let skip_count = column_names.len().min(num_cols);
        for col in columns.iter_mut().skip(skip_count) {
            col.push(Value::Null);
        }
    }

    // Build DataFrame
    let series: Vec<_> = column_names
        .iter()
        .zip(columns.into_iter())
        .map(|(name, values)| (name.as_str(), Series::new(values)))
        .collect();

    DataFrame::from_columns(series)
}

/// Write a DataFrame to a CSV file.
pub fn write_csv<P: AsRef<Path>>(df: &DataFrame, path: P) -> Result<()> {
    write_csv_with_options(df, path, CsvOptions::default())
}

/// Write a DataFrame to a CSV file with custom options.
pub fn write_csv_with_options<P: AsRef<Path>>(
    df: &DataFrame,
    path: P,
    options: CsvOptions,
) -> Result<()> {
    let file = File::create(path).map_err(|e| FrameError::IoError(e.to_string()))?;
    let mut writer = BufWriter::new(file);

    let delim = options.delimiter;

    // Write header
    if options.has_header {
        let header = df.columns().join(&delim.to_string());
        writeln!(writer, "{}", header).map_err(|e| FrameError::IoError(e.to_string()))?;
    }

    // Write data rows
    for i in 0..df.nrows() {
        let row: Vec<String> = df
            .columns()
            .iter()
            .map(|col| {
                df.get(col)
                    .ok()
                    .and_then(|s| s.iloc(i).ok())
                    .map(|v| format_csv_value(v, options.quote))
                    .unwrap_or_default()
            })
            .collect();

        writeln!(writer, "{}", row.join(&delim.to_string()))
            .map_err(|e| FrameError::IoError(e.to_string()))?;
    }

    Ok(())
}

/// Parse a single CSV line into fields.
fn parse_csv_line(line: &str, delimiter: char, quote: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == quote {
            if in_quotes {
                // Check for escaped quote
                if chars.peek() == Some(&quote) {
                    current.push(quote);
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                in_quotes = true;
            }
        } else if c == delimiter && !in_quotes {
            fields.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }

    fields.push(current.trim().to_string());
    fields
}

/// Parse a string value into a Value.
fn parse_value(s: &str) -> Value {
    if s.is_empty() {
        return Value::Null;
    }

    // Try to parse as integer
    if let Ok(i) = s.parse::<i64>() {
        return Value::Int(i);
    }

    // Try to parse as float
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }

    // Try to parse as boolean
    match s.to_lowercase().as_str() {
        "true" | "yes" | "1" => return Value::Bool(true),
        "false" | "no" | "0" => return Value::Bool(false),
        _ => {}
    }

    // Default to string
    Value::String(s.to_string())
}

/// Format a value for CSV output.
fn format_csv_value(value: &Value, quote: char) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(s) => {
            if s.contains(',') || s.contains(quote) || s.contains('\n') {
                format!(
                    "{}{}{}",
                    quote,
                    s.replace(quote, &format!("{}{}", quote, quote)),
                    quote
                )
            } else {
                s.clone()
            }
        }
    }
}

// Extension trait for DataFrame
impl DataFrame {
    /// Read from CSV file.
    pub fn read_csv<P: AsRef<Path>>(path: P) -> Result<DataFrame> {
        read_csv(path)
    }

    /// Write to CSV file.
    pub fn to_csv<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        write_csv(self, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_csv_line() {
        let line = "a,b,c";
        let fields = parse_csv_line(line, ',', '"');
        assert_eq!(fields, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_parse_csv_line_quoted() {
        let line = r#""hello, world",b,c"#;
        let fields = parse_csv_line(line, ',', '"');
        assert_eq!(fields, vec!["hello, world", "b", "c"]);
    }

    #[test]
    fn test_parse_value() {
        assert_eq!(parse_value("42"), Value::Int(42));
        assert_eq!(parse_value("3.14"), Value::Float(3.14));
        assert_eq!(parse_value("true"), Value::Bool(true));
        assert_eq!(parse_value("hello"), Value::String("hello".to_string()));
        assert_eq!(parse_value(""), Value::Null);
    }

    #[test]
    fn test_read_write_csv() {
        // Create test CSV
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "name,age,score").unwrap();
        writeln!(temp, "Alice,25,95.5").unwrap();
        writeln!(temp, "Bob,30,87.0").unwrap();
        temp.flush().unwrap();

        // Read it back
        let df = read_csv(temp.path()).unwrap();
        assert_eq!(df.shape(), (2, 3));
        assert_eq!(df.columns(), &["name", "age", "score"]);

        // Write to new file
        let output = NamedTempFile::new().unwrap();
        write_csv(&df, output.path()).unwrap();

        // Read again and verify
        let df2 = read_csv(output.path()).unwrap();
        assert_eq!(df2.shape(), (2, 3));
    }
}
