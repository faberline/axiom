//! IO operations for DataFrame.

pub mod columnar;
mod csv;
pub mod sheets;

#[cfg(feature = "io-extra")]
pub mod json;

pub use columnar::{load_columnar, read_columnar, save_columnar, write_columnar};
pub use csv::{read_csv, read_csv_with_options, write_csv, write_csv_with_options, CsvOptions};
pub use sheets::Workbook;

#[cfg(feature = "io-extra")]
pub use json::{read_json, read_json_str, save_json, write_json_str, JsonOptions, JsonOrient};
