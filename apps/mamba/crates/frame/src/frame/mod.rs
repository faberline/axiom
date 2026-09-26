//! DataFrame module (Pandas-like).

mod dataframe;
mod error;
mod index;
pub mod io;
pub mod multi_index;
pub mod ops;
mod series;
mod value;

pub use dataframe::DataFrame;
pub use error::{FrameError, Result};
pub use index::Index;
pub use io::{read_csv, write_csv, CsvOptions, Workbook};
pub use multi_index::MultiIndex;

#[cfg(feature = "io-extra")]
pub use io::{read_json, read_json_str, save_json, write_json_str, JsonOptions, JsonOrient};
pub use ops::{AggFunc, GroupBy, JoinType};
pub use series::Series;
pub use value::Value;
