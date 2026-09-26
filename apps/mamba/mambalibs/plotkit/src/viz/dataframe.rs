//! DataFrame integration for plotting.
//!
//! Provides trait-based plotting from column data, enabling `df.plot()` style
//! usage when combined with the frame crate.
//!
//! # Architecture
//!
//! The [`Plottable`] trait provides column access methods. Any DataFrame-like
//! struct can implement it, enabling auto-chart-generation from columns.
//!
//! The [`DataFramePlotter`] consumes a `Plottable` reference and provides
//! fluent methods to create different chart types.

use super::chart::Chart;
use super::series_data::DataSeries;

/// Trait for types that can provide column data for plotting.
///
/// Implement this trait for your DataFrame type to enable `plot()` integration.
pub trait Plottable {
    /// Get a numeric column by name. Returns None if not found or not numeric.
    fn get_f64_column(&self, name: &str) -> Option<Vec<f64>>;

    /// Get a string column by name. Returns None if not found.
    fn get_string_column(&self, name: &str) -> Option<Vec<String>>;

    /// Get all column names.
    fn column_names(&self) -> Vec<String>;

    /// Number of rows.
    fn n_rows(&self) -> usize;
}

/// A simple in-memory column store for standalone use.
///
/// This provides a basic `Plottable` without requiring `frame`.
#[derive(Debug, Clone, Default)]
pub struct ColumnData {
    columns: Vec<(String, ColumnValues)>,
}

/// Column value types.
#[derive(Debug, Clone)]
pub enum ColumnValues {
    Float(Vec<f64>),
    Text(Vec<String>),
}

impl ColumnData {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
        }
    }

    /// Add a numeric column.
    pub fn add_f64(mut self, name: &str, values: Vec<f64>) -> Self {
        self.columns
            .push((name.to_string(), ColumnValues::Float(values)));
        self
    }

    /// Add a string column.
    pub fn add_string(mut self, name: &str, values: Vec<String>) -> Self {
        self.columns
            .push((name.to_string(), ColumnValues::Text(values)));
        self
    }
}

impl Plottable for ColumnData {
    fn get_f64_column(&self, name: &str) -> Option<Vec<f64>> {
        self.columns.iter().find_map(|(n, v)| {
            if n == name {
                match v {
                    ColumnValues::Float(f) => Some(f.clone()),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    fn get_string_column(&self, name: &str) -> Option<Vec<String>> {
        self.columns.iter().find_map(|(n, v)| {
            if n == name {
                match v {
                    ColumnValues::Text(t) => Some(t.clone()),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    fn column_names(&self) -> Vec<String> {
        self.columns.iter().map(|(n, _)| n.clone()).collect()
    }

    fn n_rows(&self) -> usize {
        self.columns
            .first()
            .map(|(_, v)| match v {
                ColumnValues::Float(f) => f.len(),
                ColumnValues::Text(t) => t.len(),
            })
            .unwrap_or(0)
    }
}

/// Fluent plotter that creates charts from a `Plottable` data source.
pub struct DataFramePlotter<'a, P: Plottable> {
    data: &'a P,
    width: f64,
    height: f64,
    title: Option<String>,
}

impl<'a, P: Plottable> DataFramePlotter<'a, P> {
    /// Create a new plotter for the given data source.
    pub fn new(data: &'a P) -> Self {
        Self {
            data,
            width: 800.0,
            height: 600.0,
            title: None,
        }
    }

    /// Set chart dimensions.
    pub fn size(mut self, width: f64, height: f64) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set chart title.
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    /// Create a line chart with the given x and y columns.
    pub fn line(&self, x_col: &str, y_col: &str) -> Option<Chart> {
        let x = self.data.get_f64_column(x_col)?;
        let y = self.data.get_f64_column(y_col)?;
        let mut chart = Chart::new()
            .size(self.width, self.height)
            .add_series(DataSeries::line(x, y).with_label(y_col));
        if let Some(t) = &self.title {
            chart = chart.title(t);
        }
        Some(chart)
    }

    /// Create a scatter plot with the given x and y columns.
    pub fn scatter(&self, x_col: &str, y_col: &str) -> Option<Chart> {
        let x = self.data.get_f64_column(x_col)?;
        let y = self.data.get_f64_column(y_col)?;
        let mut chart = Chart::new()
            .size(self.width, self.height)
            .add_series(DataSeries::scatter(x, y).with_label(y_col));
        if let Some(t) = &self.title {
            chart = chart.title(t);
        }
        Some(chart)
    }

    /// Create a bar chart from a label column and a value column.
    pub fn bar(&self, label_col: &str, value_col: &str) -> Option<Chart> {
        let labels = self.data.get_string_column(label_col)?;
        let values = self.data.get_f64_column(value_col)?;
        let mut chart = Chart::new()
            .size(self.width, self.height)
            .add_series(DataSeries::bar(labels, values));
        if let Some(t) = &self.title {
            chart = chart.title(t);
        }
        Some(chart)
    }

    /// Create a histogram from a single numeric column.
    pub fn histogram(&self, col: &str, bins: usize) -> Option<Chart> {
        let data = self.data.get_f64_column(col)?;
        let mut chart = Chart::new()
            .size(self.width, self.height)
            .add_series(DataSeries::histogram(data, bins));
        if let Some(t) = &self.title {
            chart = chart.title(t);
        }
        Some(chart)
    }

    /// Create a multi-line chart, plotting multiple y columns against one x column.
    pub fn multi_line(&self, x_col: &str, y_cols: &[&str]) -> Option<Chart> {
        let x = self.data.get_f64_column(x_col)?;
        let mut chart = Chart::new().size(self.width, self.height);
        if let Some(t) = &self.title {
            chart = chart.title(t);
        }
        for &y_name in y_cols {
            if let Some(y) = self.data.get_f64_column(y_name) {
                chart = chart.add_series(DataSeries::line(x.clone(), y).with_label(y_name));
            }
        }
        if chart.series.is_empty() {
            return None;
        }
        Some(chart)
    }

    /// Create a box plot from multiple numeric columns.
    pub fn box_plot(&self, cols: &[&str]) -> Option<Chart> {
        let mut groups = Vec::new();
        let mut labels = Vec::new();
        for &col in cols {
            if let Some(data) = self.data.get_f64_column(col) {
                groups.push(data);
                labels.push(col.to_string());
            }
        }
        if groups.is_empty() {
            return None;
        }
        let mut chart = Chart::new()
            .size(self.width, self.height)
            .add_series(DataSeries::box_plot(groups, labels));
        if let Some(t) = &self.title {
            chart = chart.title(t);
        }
        Some(chart)
    }

    /// Create a heatmap from a 2D grid of values (each column is a column of the matrix).
    pub fn heatmap(&self, cols: &[&str]) -> Option<Chart> {
        let mut matrix = Vec::new();
        for &col in cols {
            if let Some(row) = self.data.get_f64_column(col) {
                matrix.push(row);
            }
        }
        if matrix.is_empty() {
            return None;
        }
        let mut chart = Chart::new()
            .size(self.width, self.height)
            .add_series(DataSeries::heatmap(matrix));
        if let Some(t) = &self.title {
            chart = chart.title(t);
        }
        Some(chart)
    }

    /// Create an area chart from x and y columns.
    pub fn area(&self, x_col: &str, y_col: &str) -> Option<Chart> {
        let x = self.data.get_f64_column(x_col)?;
        let y = self.data.get_f64_column(y_col)?;
        let mut chart = Chart::new()
            .size(self.width, self.height)
            .add_series(DataSeries::area(x, y).with_label(y_col));
        if let Some(t) = &self.title {
            chart = chart.title(t);
        }
        Some(chart)
    }

    /// Create a pie chart from label and value columns.
    pub fn pie(&self, label_col: &str, value_col: &str) -> Option<Chart> {
        let labels = self.data.get_string_column(label_col)?;
        let values = self.data.get_f64_column(value_col)?;
        let mut chart = Chart::new()
            .size(self.width, self.height)
            .add_series(DataSeries::pie(labels, values));
        if let Some(t) = &self.title {
            chart = chart.title(t);
        }
        Some(chart)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> ColumnData {
        ColumnData::new()
            .add_f64("x", vec![1.0, 2.0, 3.0, 4.0, 5.0])
            .add_f64("y1", vec![10.0, 20.0, 15.0, 25.0, 30.0])
            .add_f64("y2", vec![5.0, 15.0, 10.0, 20.0, 25.0])
            .add_string(
                "label",
                vec!["A".into(), "B".into(), "C".into(), "D".into(), "E".into()],
            )
    }

    #[test]
    fn test_column_data_plottable() {
        let data = sample_data();
        assert_eq!(data.n_rows(), 5);
        assert_eq!(data.column_names().len(), 4);
        assert!(data.get_f64_column("x").is_some());
        assert!(data.get_string_column("label").is_some());
        assert!(data.get_f64_column("nonexistent").is_none());
    }

    #[test]
    fn test_plotter_line() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data).title("Line Chart");
        let chart = plotter.line("x", "y1").unwrap();
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("<path"));
        assert!(svg.contains("Line Chart"));
    }

    #[test]
    fn test_plotter_scatter() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data);
        let chart = plotter.scatter("x", "y1").unwrap();
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn test_plotter_bar() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data);
        let chart = plotter.bar("label", "y1").unwrap();
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("<rect"));
    }

    #[test]
    fn test_plotter_histogram() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data);
        let chart = plotter.histogram("y1", 3).unwrap();
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("<rect"));
    }

    #[test]
    fn test_plotter_multi_line() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data).title("Multi");
        let chart = plotter.multi_line("x", &["y1", "y2"]).unwrap();
        assert_eq!(chart.series.len(), 2);
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("<path"));
    }

    #[test]
    fn test_plotter_multi_line_missing_col() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data);
        let chart = plotter.multi_line("x", &["y1", "no_such_col"]).unwrap();
        assert_eq!(chart.series.len(), 1);
    }

    #[test]
    fn test_plotter_multi_line_no_x() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data);
        assert!(plotter.multi_line("no_x", &["y1"]).is_none());
    }

    #[test]
    fn test_plotter_box_plot() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data);
        let chart = plotter.box_plot(&["y1", "y2"]).unwrap();
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("<line"));
    }

    #[test]
    fn test_plotter_heatmap() {
        let data = ColumnData::new()
            .add_f64("col1", vec![1.0, 2.0, 3.0])
            .add_f64("col2", vec![4.0, 5.0, 6.0]);
        let plotter = DataFramePlotter::new(&data);
        let chart = plotter.heatmap(&["col1", "col2"]).unwrap();
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("<rect"));
    }

    #[test]
    fn test_plotter_area() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data);
        let chart = plotter.area("x", "y1").unwrap();
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("opacity=\"0.3\""));
    }

    #[test]
    fn test_plotter_pie() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data);
        let chart = plotter.pie("label", "y1").unwrap();
        let svg = chart.to_svg().unwrap();
        assert!(svg.contains("<path"));
    }

    #[test]
    fn test_plotter_nonexistent_column() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data);
        assert!(plotter.line("x", "no_col").is_none());
        assert!(plotter.bar("no_col", "y1").is_none());
    }

    #[test]
    fn test_plotter_size() {
        let data = sample_data();
        let plotter = DataFramePlotter::new(&data).size(400.0, 300.0);
        let chart = plotter.line("x", "y1").unwrap();
        assert_eq!(chart.width, 400.0);
        assert_eq!(chart.height, 300.0);
    }

    #[test]
    fn test_column_data_empty() {
        let data = ColumnData::new();
        assert_eq!(data.n_rows(), 0);
        assert!(data.column_names().is_empty());
    }

    #[test]
    fn test_column_data_string_as_f64_returns_none() {
        let data = ColumnData::new().add_string("names", vec!["A".into(), "B".into()]);
        assert!(data.get_f64_column("names").is_none());
    }

    #[test]
    fn test_column_data_f64_as_string_returns_none() {
        let data = ColumnData::new().add_f64("nums", vec![1.0, 2.0]);
        assert!(data.get_string_column("nums").is_none());
    }
}
