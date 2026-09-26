use crate::core::{CellError, CellValue};
use crate::formula::ast::{BinaryOp, Expr, UnaryOp};
use crate::formula::functions;

/// Evaluator for formula AST
pub struct Evaluator<F>
where
    F: Fn(u32, u32) -> CellValue,
{
    get_cell_value: F,
}

impl<F> Evaluator<F>
where
    F: Fn(u32, u32) -> CellValue,
{
    pub fn new(get_cell_value: F) -> Self {
        Self { get_cell_value }
    }

    /// Evaluate an expression AST to a value
    pub fn evaluate(&self, expr: &Expr) -> CellValue {
        match expr {
            Expr::Number(n) => CellValue::Number(*n),
            Expr::String(s) => CellValue::Text(s.clone()),
            Expr::Boolean(b) => CellValue::Boolean(*b),
            Expr::Error(e) => CellValue::Error(e.clone()),

            Expr::CellRef { row, col, .. } => (self.get_cell_value)(*row, *col),

            Expr::Range { .. } => {
                // Ranges should be expanded by functions, not evaluated directly
                CellValue::Error(CellError::InvalidValue)
            }

            Expr::SheetRef { .. } => {
                // Cross-sheet references not implemented yet
                CellValue::Error(CellError::InvalidReference)
            }

            Expr::Binary { left, op, right } => self.evaluate_binary(left, *op, right),

            Expr::Unary { op, operand } => self.evaluate_unary(*op, operand),

            Expr::FunctionCall { name, args } => self.evaluate_function(name, args),

            Expr::Grouped(inner) => self.evaluate(inner),
        }
    }

    fn evaluate_binary(&self, left: &Expr, op: BinaryOp, right: &Expr) -> CellValue {
        let left_val = self.evaluate(left);
        let right_val = self.evaluate(right);

        // Propagate errors
        if let CellValue::Error(e) = &left_val {
            return CellValue::Error(e.clone());
        }
        if let CellValue::Error(e) = &right_val {
            return CellValue::Error(e.clone());
        }

        match op {
            BinaryOp::Add => self.numeric_op(&left_val, &right_val, |a, b| a + b),
            BinaryOp::Sub => self.numeric_op(&left_val, &right_val, |a, b| a - b),
            BinaryOp::Mul => self.numeric_op(&left_val, &right_val, |a, b| a * b),
            BinaryOp::Div => match (left_val.as_number(), right_val.as_number()) {
                (Some(a), Some(b)) => {
                    if b == 0.0 {
                        CellValue::Error(CellError::DivisionByZero)
                    } else {
                        CellValue::Number(a / b)
                    }
                }
                _ => CellValue::Error(CellError::InvalidValue),
            },
            BinaryOp::Pow => self.numeric_op(&left_val, &right_val, |a, b| a.powf(b)),

            BinaryOp::Concat => {
                let left_str = left_val.as_text();
                let right_str = right_val.as_text();
                CellValue::Text(format!("{}{}", left_str, right_str))
            }

            BinaryOp::Eq => CellValue::Boolean(self.compare_values(&left_val, &right_val) == 0),
            BinaryOp::Ne => CellValue::Boolean(self.compare_values(&left_val, &right_val) != 0),
            BinaryOp::Lt => CellValue::Boolean(self.compare_values(&left_val, &right_val) < 0),
            BinaryOp::Gt => CellValue::Boolean(self.compare_values(&left_val, &right_val) > 0),
            BinaryOp::Le => CellValue::Boolean(self.compare_values(&left_val, &right_val) <= 0),
            BinaryOp::Ge => CellValue::Boolean(self.compare_values(&left_val, &right_val) >= 0),
        }
    }

    fn numeric_op(
        &self,
        left: &CellValue,
        right: &CellValue,
        op: impl Fn(f64, f64) -> f64,
    ) -> CellValue {
        match (left.as_number(), right.as_number()) {
            (Some(a), Some(b)) => {
                let result = op(a, b);
                if result.is_nan() || result.is_infinite() {
                    CellValue::Error(CellError::NumError)
                } else {
                    CellValue::Number(result)
                }
            }
            _ => CellValue::Error(CellError::InvalidValue),
        }
    }

    fn compare_values(&self, left: &CellValue, right: &CellValue) -> i8 {
        match (left, right) {
            (CellValue::Number(a), CellValue::Number(b)) => {
                if a < b {
                    -1
                } else if a > b {
                    1
                } else {
                    0
                }
            }
            (CellValue::Text(a), CellValue::Text(b)) => {
                a.to_lowercase().cmp(&b.to_lowercase()) as i8
            }
            (CellValue::Boolean(a), CellValue::Boolean(b)) => (*a as i8) - (*b as i8),
            // Type coercion for comparisons
            _ => {
                let a_str = left.as_text();
                let b_str = right.as_text();
                a_str.cmp(&b_str) as i8
            }
        }
    }

    fn evaluate_unary(&self, op: UnaryOp, operand: &Expr) -> CellValue {
        let value = self.evaluate(operand);

        if let CellValue::Error(e) = &value {
            return CellValue::Error(e.clone());
        }

        match op {
            UnaryOp::Neg => match value.as_number() {
                Some(n) => CellValue::Number(-n),
                None => CellValue::Error(CellError::InvalidValue),
            },
            UnaryOp::Pos => match value.as_number() {
                Some(n) => CellValue::Number(n),
                None => CellValue::Error(CellError::InvalidValue),
            },
            UnaryOp::Percent => match value.as_number() {
                Some(n) => CellValue::Number(n / 100.0),
                None => CellValue::Error(CellError::InvalidValue),
            },
        }
    }

    fn evaluate_function(&self, name: &str, args: &[Expr]) -> CellValue {
        // Collect values, expanding ranges
        let values: Vec<CellValue> = args
            .iter()
            .flat_map(|arg| self.expand_argument(arg))
            .collect();

        match name.to_uppercase().as_str() {
            // Math functions
            "SUM" => functions::math::sum(&values),
            "AVERAGE" | "AVG" => functions::math::average(&values),
            "COUNT" => functions::math::count(&values),
            "COUNTA" => functions::math::counta(&values),
            "MIN" => functions::math::min(&values),
            "MAX" => functions::math::max(&values),
            "ABS" => functions::math::abs(&values),
            "ROUND" => functions::math::round(&values),
            "FLOOR" => functions::math::floor(&values),
            "CEILING" | "CEIL" => functions::math::ceiling(&values),
            "SQRT" => functions::math::sqrt(&values),
            "POWER" | "POW" => functions::math::power(&values),

            // Conditional functions - need special handling
            "COUNTIF" => {
                if args.len() < 2 {
                    return CellValue::Error(CellError::InvalidValue);
                }
                let range_values = self.expand_argument(&args[0]);
                let criteria = self.evaluate(&args[1]);
                functions::math::countif(&range_values, &criteria)
            }
            "SUMIF" => {
                if args.len() < 2 {
                    return CellValue::Error(CellError::InvalidValue);
                }
                let criteria_range = self.expand_argument(&args[0]);
                let criteria = self.evaluate(&args[1]);
                let sum_range = if args.len() > 2 {
                    Some(self.expand_argument(&args[2]))
                } else {
                    None
                };
                functions::math::sumif(&criteria_range, &criteria, sum_range.as_deref())
            }
            "AVERAGEIF" => {
                if args.len() < 2 {
                    return CellValue::Error(CellError::InvalidValue);
                }
                let criteria_range = self.expand_argument(&args[0]);
                let criteria = self.evaluate(&args[1]);
                let avg_range = if args.len() > 2 {
                    Some(self.expand_argument(&args[2]))
                } else {
                    None
                };
                functions::math::averageif(&criteria_range, &criteria, avg_range.as_deref())
            }

            // Logical functions
            "IF" => {
                // IF needs special handling - don't expand args
                let arg_values: Vec<CellValue> = args.iter().map(|a| self.evaluate(a)).collect();
                functions::logical::if_fn(&arg_values)
            }
            "AND" => functions::logical::and(&values),
            "OR" => functions::logical::or(&values),
            "NOT" => functions::logical::not(&values),
            "TRUE" => CellValue::Boolean(true),
            "FALSE" => CellValue::Boolean(false),

            // Text functions
            "CONCAT" | "CONCATENATE" => functions::text::concat(&values),
            "LEN" => functions::text::len(&values),
            "UPPER" => functions::text::upper(&values),
            "LOWER" => functions::text::lower(&values),
            "TRIM" => functions::text::trim(&values),
            "LEFT" => functions::text::left(&values),
            "RIGHT" => functions::text::right(&values),
            "MID" => functions::text::mid(&values),

            // Date/Time functions
            "TODAY" => functions::datetime::today(&values),
            "NOW" => functions::datetime::now(&values),
            "DATE" => functions::datetime::date(&values),
            "TIME" => functions::datetime::time(&values),
            "YEAR" => functions::datetime::year(&values),
            "MONTH" => functions::datetime::month(&values),
            "DAY" => functions::datetime::day(&values),
            "HOUR" => functions::datetime::hour(&values),
            "MINUTE" => functions::datetime::minute(&values),
            "SECOND" => functions::datetime::second(&values),
            "DATEDIF" => functions::datetime::datedif(&values),

            // Lookup functions
            "MATCH" => {
                if args.len() < 2 {
                    return CellValue::Error(CellError::InvalidValue);
                }
                let lookup_value = self.evaluate(&args[0]);
                let lookup_array = self.expand_argument(&args[1]);
                let match_type = if args.len() > 2 {
                    match self.evaluate(&args[2]) {
                        CellValue::Number(n) => n as i32,
                        _ => 1,
                    }
                } else {
                    1
                };
                functions::lookup::match_fn(&lookup_value, &lookup_array, match_type)
            }

            "VLOOKUP" => {
                if args.len() < 3 {
                    return CellValue::Error(CellError::InvalidValue);
                }

                let lookup_value = self.evaluate(&args[0]);

                // For table_array, we need dimensions
                let (table_data, num_rows, num_cols) = self.expand_range_with_dimensions(&args[1]);

                let col_index = match self.evaluate(&args[2]) {
                    CellValue::Number(n) => n as usize,
                    _ => return CellValue::Error(CellError::InvalidValue),
                };

                let approximate = if args.len() > 3 {
                    match self.evaluate(&args[3]) {
                        CellValue::Boolean(b) => b,
                        CellValue::Number(n) => n != 0.0,
                        _ => true,
                    }
                } else {
                    true
                };

                functions::lookup::vlookup(
                    &lookup_value,
                    &table_data,
                    num_rows,
                    num_cols,
                    col_index,
                    approximate,
                )
            }

            "HLOOKUP" => {
                if args.len() < 3 {
                    return CellValue::Error(CellError::InvalidValue);
                }

                let lookup_value = self.evaluate(&args[0]);

                // For table_array, we need dimensions
                let (table_data, num_rows, num_cols) = self.expand_range_with_dimensions(&args[1]);

                let row_index = match self.evaluate(&args[2]) {
                    CellValue::Number(n) => n as usize,
                    _ => return CellValue::Error(CellError::InvalidValue),
                };

                let approximate = if args.len() > 3 {
                    match self.evaluate(&args[3]) {
                        CellValue::Boolean(b) => b,
                        CellValue::Number(n) => n != 0.0,
                        _ => true,
                    }
                } else {
                    true
                };

                functions::lookup::hlookup(
                    &lookup_value,
                    &table_data,
                    num_rows,
                    num_cols,
                    row_index,
                    approximate,
                )
            }

            _ => CellValue::Error(CellError::InvalidName),
        }
    }

    /// Expand a range with dimensions needed for VLOOKUP/HLOOKUP
    fn expand_range_with_dimensions(&self, expr: &Expr) -> (Vec<CellValue>, usize, usize) {
        match expr {
            Expr::Range { start, end } => {
                let (start_row, start_col) = self.get_cell_coords(start);
                let (end_row, end_col) = self.get_cell_coords(end);

                let min_row = start_row.min(end_row);
                let max_row = start_row.max(end_row);
                let min_col = start_col.min(end_col);
                let max_col = start_col.max(end_col);

                let num_rows = (max_row - min_row + 1) as usize;
                let num_cols = (max_col - min_col + 1) as usize;

                let mut values = Vec::with_capacity(num_rows * num_cols);
                for row in min_row..=max_row {
                    for col in min_col..=max_col {
                        values.push((self.get_cell_value)(row, col));
                    }
                }

                (values, num_rows, num_cols)
            }
            _ => {
                let value = self.evaluate(expr);
                (vec![value], 1, 1)
            }
        }
    }

    /// Get cell coordinates from a CellRef expression
    fn get_cell_coords(&self, expr: &Expr) -> (u32, u32) {
        match expr {
            Expr::CellRef { col, row, .. } => (*row, *col),
            _ => (0, 0),
        }
    }

    /// Expand an argument, handling ranges
    fn expand_argument(&self, expr: &Expr) -> Vec<CellValue> {
        match expr {
            Expr::Range { start, end } => {
                if let (
                    Expr::CellRef {
                        row: r1, col: c1, ..
                    },
                    Expr::CellRef {
                        row: r2, col: c2, ..
                    },
                ) = (start.as_ref(), end.as_ref())
                {
                    let min_row = (*r1).min(*r2);
                    let max_row = (*r1).max(*r2);
                    let min_col = (*c1).min(*c2);
                    let max_col = (*c1).max(*c2);

                    let mut values = Vec::new();
                    for row in min_row..=max_row {
                        for col in min_col..=max_col {
                            values.push((self.get_cell_value)(row, col));
                        }
                    }
                    values
                } else {
                    vec![CellValue::Error(CellError::InvalidReference)]
                }
            }
            _ => vec![self.evaluate(expr)],
        }
    }
}

/// Evaluator with cross-sheet reference support
pub struct CrossSheetEvaluator<F>
where
    F: Fn(Option<&str>, u32, u32) -> CellValue,
{
    get_cell_value: F,
    current_sheet: Option<String>,
}

impl<F> CrossSheetEvaluator<F>
where
    F: Fn(Option<&str>, u32, u32) -> CellValue,
{
    pub fn new(get_cell_value: F) -> Self {
        Self {
            get_cell_value,
            current_sheet: None,
        }
    }

    /// Create evaluator with current sheet context
    pub fn with_sheet(get_cell_value: F, current_sheet: &str) -> Self {
        Self {
            get_cell_value,
            current_sheet: Some(current_sheet.to_string()),
        }
    }

    /// Evaluate an expression AST to a value
    pub fn evaluate(&self, expr: &Expr) -> CellValue {
        match expr {
            Expr::Number(n) => CellValue::Number(*n),
            Expr::String(s) => CellValue::Text(s.clone()),
            Expr::Boolean(b) => CellValue::Boolean(*b),
            Expr::Error(e) => CellValue::Error(e.clone()),

            Expr::CellRef { row, col, .. } => {
                // Use current sheet context for unqualified references
                (self.get_cell_value)(self.current_sheet.as_deref(), *row, *col)
            }

            Expr::Range { .. } => {
                // Ranges should be expanded by functions, not evaluated directly
                CellValue::Error(CellError::InvalidValue)
            }

            Expr::SheetRef {
                sheet_name,
                reference,
            } => {
                // Evaluate the reference within the context of the specified sheet
                self.evaluate_with_sheet(reference, sheet_name)
            }

            Expr::Binary { left, op, right } => self.evaluate_binary(left, *op, right),

            Expr::Unary { op, operand } => self.evaluate_unary(*op, operand),

            Expr::FunctionCall { name, args } => self.evaluate_function(name, args),

            Expr::Grouped(inner) => self.evaluate(inner),
        }
    }

    /// Evaluate an expression with a specific sheet context
    fn evaluate_with_sheet(&self, expr: &Expr, sheet_name: &str) -> CellValue {
        match expr {
            Expr::CellRef { row, col, .. } => (self.get_cell_value)(Some(sheet_name), *row, *col),
            Expr::Range { .. } => {
                // Ranges in sheet refs should be handled by functions
                CellValue::Error(CellError::InvalidValue)
            }
            _ => self.evaluate(expr),
        }
    }

    fn evaluate_binary(&self, left: &Expr, op: BinaryOp, right: &Expr) -> CellValue {
        let left_val = self.evaluate(left);
        let right_val = self.evaluate(right);

        // Propagate errors
        if let CellValue::Error(e) = &left_val {
            return CellValue::Error(e.clone());
        }
        if let CellValue::Error(e) = &right_val {
            return CellValue::Error(e.clone());
        }

        match op {
            BinaryOp::Add => self.numeric_op(&left_val, &right_val, |a, b| a + b),
            BinaryOp::Sub => self.numeric_op(&left_val, &right_val, |a, b| a - b),
            BinaryOp::Mul => self.numeric_op(&left_val, &right_val, |a, b| a * b),
            BinaryOp::Div => match (left_val.as_number(), right_val.as_number()) {
                (Some(a), Some(b)) => {
                    if b == 0.0 {
                        CellValue::Error(CellError::DivisionByZero)
                    } else {
                        CellValue::Number(a / b)
                    }
                }
                _ => CellValue::Error(CellError::InvalidValue),
            },
            BinaryOp::Pow => self.numeric_op(&left_val, &right_val, |a, b| a.powf(b)),

            BinaryOp::Concat => {
                let left_str = left_val.as_text();
                let right_str = right_val.as_text();
                CellValue::Text(format!("{}{}", left_str, right_str))
            }

            BinaryOp::Eq => CellValue::Boolean(self.compare_values(&left_val, &right_val) == 0),
            BinaryOp::Ne => CellValue::Boolean(self.compare_values(&left_val, &right_val) != 0),
            BinaryOp::Lt => CellValue::Boolean(self.compare_values(&left_val, &right_val) < 0),
            BinaryOp::Gt => CellValue::Boolean(self.compare_values(&left_val, &right_val) > 0),
            BinaryOp::Le => CellValue::Boolean(self.compare_values(&left_val, &right_val) <= 0),
            BinaryOp::Ge => CellValue::Boolean(self.compare_values(&left_val, &right_val) >= 0),
        }
    }

    fn numeric_op(
        &self,
        left: &CellValue,
        right: &CellValue,
        op: impl Fn(f64, f64) -> f64,
    ) -> CellValue {
        match (left.as_number(), right.as_number()) {
            (Some(a), Some(b)) => {
                let result = op(a, b);
                if result.is_nan() || result.is_infinite() {
                    CellValue::Error(CellError::NumError)
                } else {
                    CellValue::Number(result)
                }
            }
            _ => CellValue::Error(CellError::InvalidValue),
        }
    }

    fn compare_values(&self, left: &CellValue, right: &CellValue) -> i8 {
        match (left, right) {
            (CellValue::Number(a), CellValue::Number(b)) => {
                if a < b {
                    -1
                } else if a > b {
                    1
                } else {
                    0
                }
            }
            (CellValue::Text(a), CellValue::Text(b)) => {
                a.to_lowercase().cmp(&b.to_lowercase()) as i8
            }
            (CellValue::Boolean(a), CellValue::Boolean(b)) => (*a as i8) - (*b as i8),
            _ => {
                let a_str = left.as_text();
                let b_str = right.as_text();
                a_str.cmp(&b_str) as i8
            }
        }
    }

    fn evaluate_unary(&self, op: UnaryOp, operand: &Expr) -> CellValue {
        let value = self.evaluate(operand);

        if let CellValue::Error(e) = &value {
            return CellValue::Error(e.clone());
        }

        match op {
            UnaryOp::Neg => match value.as_number() {
                Some(n) => CellValue::Number(-n),
                None => CellValue::Error(CellError::InvalidValue),
            },
            UnaryOp::Pos => match value.as_number() {
                Some(n) => CellValue::Number(n),
                None => CellValue::Error(CellError::InvalidValue),
            },
            UnaryOp::Percent => match value.as_number() {
                Some(n) => CellValue::Number(n / 100.0),
                None => CellValue::Error(CellError::InvalidValue),
            },
        }
    }

    fn evaluate_function(&self, name: &str, args: &[Expr]) -> CellValue {
        // Collect values, expanding ranges
        let values: Vec<CellValue> = args
            .iter()
            .flat_map(|arg| self.expand_argument(arg))
            .collect();

        match name.to_uppercase().as_str() {
            // Math functions
            "SUM" => functions::math::sum(&values),
            "AVERAGE" | "AVG" => functions::math::average(&values),
            "COUNT" => functions::math::count(&values),
            "COUNTA" => functions::math::counta(&values),
            "MIN" => functions::math::min(&values),
            "MAX" => functions::math::max(&values),
            "ABS" => functions::math::abs(&values),
            "ROUND" => functions::math::round(&values),
            "FLOOR" => functions::math::floor(&values),
            "CEILING" | "CEIL" => functions::math::ceiling(&values),
            "SQRT" => functions::math::sqrt(&values),
            "POWER" | "POW" => functions::math::power(&values),

            // Conditional functions
            "COUNTIF" => {
                if args.len() < 2 {
                    return CellValue::Error(CellError::InvalidValue);
                }
                let range_values = self.expand_argument(&args[0]);
                let criteria = self.evaluate(&args[1]);
                functions::math::countif(&range_values, &criteria)
            }
            "SUMIF" => {
                if args.len() < 2 {
                    return CellValue::Error(CellError::InvalidValue);
                }
                let criteria_range = self.expand_argument(&args[0]);
                let criteria = self.evaluate(&args[1]);
                let sum_range = if args.len() > 2 {
                    Some(self.expand_argument(&args[2]))
                } else {
                    None
                };
                functions::math::sumif(&criteria_range, &criteria, sum_range.as_deref())
            }
            "AVERAGEIF" => {
                if args.len() < 2 {
                    return CellValue::Error(CellError::InvalidValue);
                }
                let criteria_range = self.expand_argument(&args[0]);
                let criteria = self.evaluate(&args[1]);
                let avg_range = if args.len() > 2 {
                    Some(self.expand_argument(&args[2]))
                } else {
                    None
                };
                functions::math::averageif(&criteria_range, &criteria, avg_range.as_deref())
            }

            // Logical functions
            "IF" => {
                let arg_values: Vec<CellValue> = args.iter().map(|a| self.evaluate(a)).collect();
                functions::logical::if_fn(&arg_values)
            }
            "AND" => functions::logical::and(&values),
            "OR" => functions::logical::or(&values),
            "NOT" => functions::logical::not(&values),
            "TRUE" => CellValue::Boolean(true),
            "FALSE" => CellValue::Boolean(false),

            // Text functions
            "CONCAT" | "CONCATENATE" => functions::text::concat(&values),
            "LEN" => functions::text::len(&values),
            "UPPER" => functions::text::upper(&values),
            "LOWER" => functions::text::lower(&values),
            "TRIM" => functions::text::trim(&values),
            "LEFT" => functions::text::left(&values),
            "RIGHT" => functions::text::right(&values),
            "MID" => functions::text::mid(&values),

            // Date/Time functions
            "TODAY" => functions::datetime::today(&values),
            "NOW" => functions::datetime::now(&values),
            "DATE" => functions::datetime::date(&values),
            "TIME" => functions::datetime::time(&values),
            "YEAR" => functions::datetime::year(&values),
            "MONTH" => functions::datetime::month(&values),
            "DAY" => functions::datetime::day(&values),
            "HOUR" => functions::datetime::hour(&values),
            "MINUTE" => functions::datetime::minute(&values),
            "SECOND" => functions::datetime::second(&values),
            "DATEDIF" => functions::datetime::datedif(&values),

            // Lookup functions
            "MATCH" => {
                if args.len() < 2 {
                    return CellValue::Error(CellError::InvalidValue);
                }
                let lookup_value = self.evaluate(&args[0]);
                let lookup_array = self.expand_argument(&args[1]);
                let match_type = if args.len() > 2 {
                    match self.evaluate(&args[2]) {
                        CellValue::Number(n) => n as i32,
                        _ => 1,
                    }
                } else {
                    1
                };
                functions::lookup::match_fn(&lookup_value, &lookup_array, match_type)
            }

            "VLOOKUP" => {
                if args.len() < 3 {
                    return CellValue::Error(CellError::InvalidValue);
                }

                let lookup_value = self.evaluate(&args[0]);

                // For table_array, we need dimensions
                let (table_data, num_rows, num_cols) = self.expand_range_with_dimensions(&args[1]);

                let col_index = match self.evaluate(&args[2]) {
                    CellValue::Number(n) => n as usize,
                    _ => return CellValue::Error(CellError::InvalidValue),
                };

                let approximate = if args.len() > 3 {
                    match self.evaluate(&args[3]) {
                        CellValue::Boolean(b) => b,
                        CellValue::Number(n) => n != 0.0,
                        _ => true,
                    }
                } else {
                    true
                };

                functions::lookup::vlookup(
                    &lookup_value,
                    &table_data,
                    num_rows,
                    num_cols,
                    col_index,
                    approximate,
                )
            }

            "HLOOKUP" => {
                if args.len() < 3 {
                    return CellValue::Error(CellError::InvalidValue);
                }

                let lookup_value = self.evaluate(&args[0]);

                // For table_array, we need dimensions
                let (table_data, num_rows, num_cols) = self.expand_range_with_dimensions(&args[1]);

                let row_index = match self.evaluate(&args[2]) {
                    CellValue::Number(n) => n as usize,
                    _ => return CellValue::Error(CellError::InvalidValue),
                };

                let approximate = if args.len() > 3 {
                    match self.evaluate(&args[3]) {
                        CellValue::Boolean(b) => b,
                        CellValue::Number(n) => n != 0.0,
                        _ => true,
                    }
                } else {
                    true
                };

                functions::lookup::hlookup(
                    &lookup_value,
                    &table_data,
                    num_rows,
                    num_cols,
                    row_index,
                    approximate,
                )
            }

            _ => CellValue::Error(CellError::InvalidName),
        }
    }

    /// Expand an argument, handling ranges and sheet references
    fn expand_argument(&self, expr: &Expr) -> Vec<CellValue> {
        match expr {
            Expr::Range { start, end } => {
                self.expand_range(start, end, self.current_sheet.as_deref())
            }
            Expr::SheetRef {
                sheet_name,
                reference,
            } => match reference.as_ref() {
                Expr::Range { start, end } => self.expand_range(start, end, Some(sheet_name)),
                Expr::CellRef { row, col, .. } => {
                    vec![(self.get_cell_value)(Some(sheet_name), *row, *col)]
                }
                _ => vec![self.evaluate(expr)],
            },
            _ => vec![self.evaluate(expr)],
        }
    }

    /// Expand a range into a vector of cell values
    fn expand_range(&self, start: &Expr, end: &Expr, sheet: Option<&str>) -> Vec<CellValue> {
        if let (
            Expr::CellRef {
                row: r1, col: c1, ..
            },
            Expr::CellRef {
                row: r2, col: c2, ..
            },
        ) = (start, end)
        {
            let min_row = (*r1).min(*r2);
            let max_row = (*r1).max(*r2);
            let min_col = (*c1).min(*c2);
            let max_col = (*c1).max(*c2);

            let mut values = Vec::new();
            for row in min_row..=max_row {
                for col in min_col..=max_col {
                    values.push((self.get_cell_value)(sheet, row, col));
                }
            }
            values
        } else {
            vec![CellValue::Error(CellError::InvalidReference)]
        }
    }

    /// Expand a range with dimensions needed for VLOOKUP/HLOOKUP
    fn expand_range_with_dimensions(&self, expr: &Expr) -> (Vec<CellValue>, usize, usize) {
        match expr {
            Expr::Range { start, end } => {
                let (start_row, start_col) = self.get_cell_coords_from_expr(start);
                let (end_row, end_col) = self.get_cell_coords_from_expr(end);

                let min_row = start_row.min(end_row);
                let max_row = start_row.max(end_row);
                let min_col = start_col.min(end_col);
                let max_col = start_col.max(end_col);

                let num_rows = (max_row - min_row + 1) as usize;
                let num_cols = (max_col - min_col + 1) as usize;

                let mut values = Vec::with_capacity(num_rows * num_cols);
                for row in min_row..=max_row {
                    for col in min_col..=max_col {
                        values.push((self.get_cell_value)(
                            self.current_sheet.as_deref(),
                            row,
                            col,
                        ));
                    }
                }

                (values, num_rows, num_cols)
            }
            Expr::SheetRef {
                sheet_name,
                reference,
            } => match reference.as_ref() {
                Expr::Range { start, end } => {
                    let (start_row, start_col) = self.get_cell_coords_from_expr(start);
                    let (end_row, end_col) = self.get_cell_coords_from_expr(end);

                    let min_row = start_row.min(end_row);
                    let max_row = start_row.max(end_row);
                    let min_col = start_col.min(end_col);
                    let max_col = start_col.max(end_col);

                    let num_rows = (max_row - min_row + 1) as usize;
                    let num_cols = (max_col - min_col + 1) as usize;

                    let mut values = Vec::with_capacity(num_rows * num_cols);
                    for row in min_row..=max_row {
                        for col in min_col..=max_col {
                            values.push((self.get_cell_value)(Some(sheet_name), row, col));
                        }
                    }

                    (values, num_rows, num_cols)
                }
                _ => {
                    let value = self.evaluate(expr);
                    (vec![value], 1, 1)
                }
            },
            _ => {
                let value = self.evaluate(expr);
                (vec![value], 1, 1)
            }
        }
    }

    /// Get cell coordinates from a CellRef expression
    fn get_cell_coords_from_expr(&self, expr: &Expr) -> (u32, u32) {
        match expr {
            Expr::CellRef { col, row, .. } => (*row, *col),
            _ => (0, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::lexer::Lexer;
    use crate::formula::parser::Parser;

    fn eval(input: &str) -> CellValue {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let evaluator = Evaluator::new(|_row, _col| CellValue::Empty);
        evaluator.evaluate(&ast)
    }

    fn eval_with_cells<F>(input: &str, get_cell: F) -> CellValue
    where
        F: Fn(u32, u32) -> CellValue,
    {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let ast = parser.parse().unwrap();

        let evaluator = Evaluator::new(get_cell);
        evaluator.evaluate(&ast)
    }

    #[test]
    fn test_arithmetic() {
        assert_eq!(eval("1 + 2"), CellValue::Number(3.0));
        assert_eq!(eval("10 - 3"), CellValue::Number(7.0));
        assert_eq!(eval("4 * 5"), CellValue::Number(20.0));
        assert_eq!(eval("20 / 4"), CellValue::Number(5.0));
        assert_eq!(eval("2 ^ 3"), CellValue::Number(8.0));
    }

    #[test]
    fn test_precedence() {
        assert_eq!(eval("1 + 2 * 3"), CellValue::Number(7.0));
        assert_eq!(eval("(1 + 2) * 3"), CellValue::Number(9.0));
    }

    #[test]
    fn test_division_by_zero() {
        assert!(matches!(
            eval("1 / 0"),
            CellValue::Error(CellError::DivisionByZero)
        ));
    }

    #[test]
    fn test_comparison() {
        assert_eq!(eval("5 > 3"), CellValue::Boolean(true));
        assert_eq!(eval("5 < 3"), CellValue::Boolean(false));
        assert_eq!(eval("5 = 5"), CellValue::Boolean(true));
    }

    #[test]
    fn test_concat() {
        assert_eq!(
            eval("\"Hello\" & \" \" & \"World\""),
            CellValue::Text("Hello World".to_string())
        );
    }

    #[test]
    fn test_cell_reference() {
        let result = eval_with_cells("A1 + B1", |row, col| {
            if row == 0 && col == 0 {
                CellValue::Number(10.0)
            } else if row == 0 && col == 1 {
                CellValue::Number(20.0)
            } else {
                CellValue::Empty
            }
        });
        assert_eq!(result, CellValue::Number(30.0));
    }

    #[test]
    fn test_sum_function() {
        let result = eval_with_cells("SUM(A1:A3)", |row, col| {
            if col == 0 && row < 3 {
                CellValue::Number((row + 1) as f64)
            } else {
                CellValue::Empty
            }
        });
        assert_eq!(result, CellValue::Number(6.0)); // 1 + 2 + 3
    }

    #[test]
    fn test_if_function() {
        assert_eq!(eval("IF(TRUE, 1, 2)"), CellValue::Number(1.0));
        assert_eq!(eval("IF(FALSE, 1, 2)"), CellValue::Number(2.0));
        assert_eq!(
            eval("IF(5 > 3, \"yes\", \"no\")"),
            CellValue::Text("yes".to_string())
        );
    }

    #[test]
    fn test_countif() {
        let result = eval_with_cells("COUNTIF(A1:A4, \">3\")", |row, col| {
            if col == 0 && row < 4 {
                CellValue::Number((row + 1) as f64) // 1, 2, 3, 4
            } else {
                CellValue::Empty
            }
        });
        assert_eq!(result, CellValue::Number(1.0)); // Only 4 > 3
    }

    #[test]
    fn test_sumif() {
        let result = eval_with_cells("SUMIF(A1:A4, \">=2\")", |row, col| {
            if col == 0 && row < 4 {
                CellValue::Number((row + 1) as f64) // 1, 2, 3, 4
            } else {
                CellValue::Empty
            }
        });
        assert_eq!(result, CellValue::Number(9.0)); // 2 + 3 + 4
    }

    #[test]
    fn test_averageif() {
        let result = eval_with_cells("AVERAGEIF(A1:A4, \">=2\")", |row, col| {
            if col == 0 && row < 4 {
                CellValue::Number((row + 1) as f64) // 1, 2, 3, 4
            } else {
                CellValue::Empty
            }
        });
        assert_eq!(result, CellValue::Number(3.0)); // (2 + 3 + 4) / 3
    }

    fn eval_cross_sheet<F>(input: &str, get_cell: F) -> CellValue
    where
        F: Fn(Option<&str>, u32, u32) -> CellValue,
    {
        use crate::formula::parser_nom::NomParser;
        let parser = NomParser::new();
        let ast = parser.parse(input).unwrap();

        let evaluator = CrossSheetEvaluator::new(get_cell);
        evaluator.evaluate(&ast)
    }

    #[test]
    fn test_cross_sheet_reference() {
        let result = eval_cross_sheet("Sheet2!A1", |sheet, row, col| {
            if sheet == Some("Sheet2") && row == 0 && col == 0 {
                CellValue::Number(42.0)
            } else {
                CellValue::Empty
            }
        });
        assert_eq!(result, CellValue::Number(42.0));
    }

    #[test]
    fn test_cross_sheet_sum() {
        let result = eval_cross_sheet("SUM(Sheet2!A1:A3)", |sheet, row, col| {
            if sheet == Some("Sheet2") && col == 0 && row < 3 {
                CellValue::Number((row + 1) as f64)
            } else {
                CellValue::Empty
            }
        });
        assert_eq!(result, CellValue::Number(6.0)); // 1 + 2 + 3
    }

    #[test]
    fn test_cross_sheet_invalid() {
        let result = eval_cross_sheet("InvalidSheet!A1", |sheet, _row, _col| {
            if sheet == Some("InvalidSheet") {
                CellValue::Error(CellError::InvalidReference)
            } else {
                CellValue::Empty
            }
        });
        assert!(matches!(
            result,
            CellValue::Error(CellError::InvalidReference)
        ));
    }

    #[test]
    fn test_vlookup() {
        // Create a lookup table:
        // | ID | Name  | Score |
        // | 10 | Alice | 85    |
        // | 20 | Bob   | 90    |
        // | 30 | Carol | 75    |
        let result = eval_with_cells("VLOOKUP(20, A1:C3, 2, FALSE)", |row, col| {
            match (row, col) {
                // Row 0: 10, Alice, 85
                (0, 0) => CellValue::Number(10.0),
                (0, 1) => CellValue::Text("Alice".to_string()),
                (0, 2) => CellValue::Number(85.0),
                // Row 1: 20, Bob, 90
                (1, 0) => CellValue::Number(20.0),
                (1, 1) => CellValue::Text("Bob".to_string()),
                (1, 2) => CellValue::Number(90.0),
                // Row 2: 30, Carol, 75
                (2, 0) => CellValue::Number(30.0),
                (2, 1) => CellValue::Text("Carol".to_string()),
                (2, 2) => CellValue::Number(75.0),
                _ => CellValue::Empty,
            }
        });
        assert_eq!(result, CellValue::Text("Bob".to_string()));
    }

    #[test]
    fn test_vlookup_approximate() {
        // Grade lookup table (sorted):
        // | Grade | Letter |
        // | 0     | F      |
        // | 60    | D      |
        // | 70    | C      |
        // | 80    | B      |
        // | 90    | A      |
        let result = eval_with_cells("VLOOKUP(85, A1:B5, 2, TRUE)", |row, col| match (row, col) {
            (0, 0) => CellValue::Number(0.0),
            (0, 1) => CellValue::Text("F".to_string()),
            (1, 0) => CellValue::Number(60.0),
            (1, 1) => CellValue::Text("D".to_string()),
            (2, 0) => CellValue::Number(70.0),
            (2, 1) => CellValue::Text("C".to_string()),
            (3, 0) => CellValue::Number(80.0),
            (3, 1) => CellValue::Text("B".to_string()),
            (4, 0) => CellValue::Number(90.0),
            (4, 1) => CellValue::Text("A".to_string()),
            _ => CellValue::Empty,
        });
        assert_eq!(result, CellValue::Text("B".to_string()));
    }

    #[test]
    fn test_hlookup() {
        // Horizontal lookup table:
        // | Product A | Product B | Product C |
        // | 100       | 200       | 300       |
        // | In Stock  | Sold Out  | In Stock  |
        let result = eval_with_cells("HLOOKUP(\"Product B\", A1:C3, 2, FALSE)", |row, col| {
            match (row, col) {
                // Row 0: Product A, Product B, Product C
                (0, 0) => CellValue::Text("Product A".to_string()),
                (0, 1) => CellValue::Text("Product B".to_string()),
                (0, 2) => CellValue::Text("Product C".to_string()),
                // Row 1: 100, 200, 300
                (1, 0) => CellValue::Number(100.0),
                (1, 1) => CellValue::Number(200.0),
                (1, 2) => CellValue::Number(300.0),
                // Row 2: In Stock, Sold Out, In Stock
                (2, 0) => CellValue::Text("In Stock".to_string()),
                (2, 1) => CellValue::Text("Sold Out".to_string()),
                (2, 2) => CellValue::Text("In Stock".to_string()),
                _ => CellValue::Empty,
            }
        });
        assert_eq!(result, CellValue::Number(200.0));
    }

    #[test]
    fn test_match() {
        let result = eval_with_cells("MATCH(20, A1:A3, 0)", |row, col| {
            if col == 0 && row < 3 {
                CellValue::Number((row + 1) as f64 * 10.0) // 10, 20, 30
            } else {
                CellValue::Empty
            }
        });
        assert_eq!(result, CellValue::Number(2.0)); // Found at position 2
    }
}
