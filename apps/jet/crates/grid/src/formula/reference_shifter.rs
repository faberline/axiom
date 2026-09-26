use crate::formula::ast::Expr;
use crate::formula::parser_nom::NomParser;

/// Shift formula references when rows are inserted/deleted.
///
/// # Arguments
/// * `formula` - The formula string (e.g., "=A1+B2")
/// * `at_row` - The row where insertion/deletion starts (0-indexed)
/// * `delta` - Positive for insert (shift down), negative for delete (shift up)
///
/// # Returns
/// The modified formula string, or `None` if a reference becomes invalid (deleted)
///
/// # Examples
///
/// Insert rows:
/// ```
/// use jet_grid::formula::shift_formula_rows;
///
/// let formula = "=A1+B3";
/// let result = shift_formula_rows(formula, 2, 2);
/// assert_eq!(result, Some("=A1+B5".to_string()));
/// ```
///
/// Delete rows:
/// ```
/// use jet_grid::formula::shift_formula_rows;
///
/// let formula = "=A1+B3";
/// let result = shift_formula_rows(formula, 1, -1);
/// assert_eq!(result, Some("=A1+B2".to_string()));
/// ```
///
/// Absolute references don't shift:
/// ```
/// use jet_grid::formula::shift_formula_rows;
///
/// let formula = "=$A$1+B3";
/// let result = shift_formula_rows(formula, 0, 2);
/// assert_eq!(result, Some("=$A$1+B5".to_string()));
/// ```
///
/// Deleted reference returns None:
/// ```
/// use jet_grid::formula::shift_formula_rows;
///
/// let formula = "=A1+B3";
/// let result = shift_formula_rows(formula, 2, -2);
/// assert_eq!(result, None);
/// ```
pub fn shift_formula_rows(formula: &str, at_row: u32, delta: i32) -> Option<String> {
    // Parse the formula
    let parser = NomParser::new();
    let ast = parser.parse(formula).ok()?;

    // Shift the AST
    let shifted_ast = shift_expr_rows(&ast, at_row, delta)?;

    // Convert back to string
    Some(format!("={}", shifted_ast))
}

/// Shift formula references when columns are inserted/deleted.
///
/// # Arguments
/// * `formula` - The formula string (e.g., "=A1+B2")
/// * `at_col` - The column where insertion/deletion starts (0-indexed, 0=A, 1=B, etc.)
/// * `delta` - Positive for insert (shift right), negative for delete (shift left)
///
/// # Returns
/// The modified formula string, or `None` if a reference becomes invalid (deleted)
///
/// # Examples
///
/// Insert columns:
/// ```
/// use jet_grid::formula::shift_formula_cols;
///
/// let formula = "=A1+B1";
/// let result = shift_formula_cols(formula, 1, 2);
/// assert_eq!(result, Some("=A1+D1".to_string()));
/// ```
///
/// Delete columns:
/// ```
/// use jet_grid::formula::shift_formula_cols;
///
/// let formula = "=A1+C1";
/// let result = shift_formula_cols(formula, 1, -1);
/// assert_eq!(result, Some("=A1+B1".to_string()));
/// ```
///
/// Absolute column references don't shift:
/// ```
/// use jet_grid::formula::shift_formula_cols;
///
/// let formula = "=$A1+B1";
/// let result = shift_formula_cols(formula, 0, 2);
/// assert_eq!(result, Some("=$A1+D1".to_string()));
/// ```
pub fn shift_formula_cols(formula: &str, at_col: u32, delta: i32) -> Option<String> {
    // Parse the formula
    let parser = NomParser::new();
    let ast = parser.parse(formula).ok()?;

    // Shift the AST
    let shifted_ast = shift_expr_cols(&ast, at_col, delta)?;

    // Convert back to string
    Some(format!("={}", shifted_ast))
}

/// Recursively shift row references in an expression
fn shift_expr_rows(expr: &Expr, at_row: u32, delta: i32) -> Option<Expr> {
    match expr {
        Expr::CellRef {
            col,
            row,
            abs_col,
            abs_row,
        } => {
            // Only shift non-absolute row references at or after at_row
            if !abs_row && *row >= at_row {
                // Check if reference was in deleted range (delta < 0)
                if delta < 0 {
                    let delete_count = (-delta) as u32;
                    let delete_end = at_row + delete_count - 1;
                    if *row >= at_row && *row <= delete_end {
                        return None;
                    }
                }

                let new_row = (*row as i32) + delta;

                // Check if reference becomes invalid
                if new_row < 0 {
                    return None;
                }

                Some(Expr::CellRef {
                    col: *col,
                    row: new_row as u32,
                    abs_col: *abs_col,
                    abs_row: *abs_row,
                })
            } else {
                Some(expr.clone())
            }
        }
        Expr::Range { start, end } => {
            let shifted_start = shift_expr_rows(start, at_row, delta)?;
            let shifted_end = shift_expr_rows(end, at_row, delta)?;
            Some(Expr::Range {
                start: Box::new(shifted_start),
                end: Box::new(shifted_end),
            })
        }
        Expr::Binary { left, op, right } => {
            let shifted_left = shift_expr_rows(left, at_row, delta)?;
            let shifted_right = shift_expr_rows(right, at_row, delta)?;
            Some(Expr::Binary {
                left: Box::new(shifted_left),
                op: *op,
                right: Box::new(shifted_right),
            })
        }
        Expr::Unary { op, operand } => {
            let shifted_operand = shift_expr_rows(operand, at_row, delta)?;
            Some(Expr::Unary {
                op: *op,
                operand: Box::new(shifted_operand),
            })
        }
        Expr::FunctionCall { name, args } => {
            let mut shifted_args = Vec::new();
            for arg in args {
                shifted_args.push(shift_expr_rows(arg, at_row, delta)?);
            }
            Some(Expr::FunctionCall {
                name: name.clone(),
                args: shifted_args,
            })
        }
        Expr::Grouped(inner) => {
            let shifted_inner = shift_expr_rows(inner, at_row, delta)?;
            Some(Expr::Grouped(Box::new(shifted_inner)))
        }
        Expr::SheetRef {
            sheet_name,
            reference,
        } => {
            let shifted_ref = shift_expr_rows(reference, at_row, delta)?;
            Some(Expr::SheetRef {
                sheet_name: sheet_name.clone(),
                reference: Box::new(shifted_ref),
            })
        }
        // Literals don't contain references
        _ => Some(expr.clone()),
    }
}

/// Recursively shift column references in an expression
fn shift_expr_cols(expr: &Expr, at_col: u32, delta: i32) -> Option<Expr> {
    match expr {
        Expr::CellRef {
            col,
            row,
            abs_col,
            abs_row,
        } => {
            // Only shift non-absolute column references at or after at_col
            if !abs_col && *col >= at_col {
                // Check if reference was in deleted range (delta < 0)
                if delta < 0 {
                    let delete_count = (-delta) as u32;
                    let delete_end = at_col + delete_count - 1;
                    if *col >= at_col && *col <= delete_end {
                        return None;
                    }
                }

                let new_col = (*col as i32) + delta;

                // Check if reference becomes invalid
                if new_col < 0 {
                    return None;
                }

                Some(Expr::CellRef {
                    col: new_col as u32,
                    row: *row,
                    abs_col: *abs_col,
                    abs_row: *abs_row,
                })
            } else {
                Some(expr.clone())
            }
        }
        Expr::Range { start, end } => {
            let shifted_start = shift_expr_cols(start, at_col, delta)?;
            let shifted_end = shift_expr_cols(end, at_col, delta)?;
            Some(Expr::Range {
                start: Box::new(shifted_start),
                end: Box::new(shifted_end),
            })
        }
        Expr::Binary { left, op, right } => {
            let shifted_left = shift_expr_cols(left, at_col, delta)?;
            let shifted_right = shift_expr_cols(right, at_col, delta)?;
            Some(Expr::Binary {
                left: Box::new(shifted_left),
                op: *op,
                right: Box::new(shifted_right),
            })
        }
        Expr::Unary { op, operand } => {
            let shifted_operand = shift_expr_cols(operand, at_col, delta)?;
            Some(Expr::Unary {
                op: *op,
                operand: Box::new(shifted_operand),
            })
        }
        Expr::FunctionCall { name, args } => {
            let mut shifted_args = Vec::new();
            for arg in args {
                shifted_args.push(shift_expr_cols(arg, at_col, delta)?);
            }
            Some(Expr::FunctionCall {
                name: name.clone(),
                args: shifted_args,
            })
        }
        Expr::Grouped(inner) => {
            let shifted_inner = shift_expr_cols(inner, at_col, delta)?;
            Some(Expr::Grouped(Box::new(shifted_inner)))
        }
        Expr::SheetRef {
            sheet_name,
            reference,
        } => {
            let shifted_ref = shift_expr_cols(reference, at_col, delta)?;
            Some(Expr::SheetRef {
                sheet_name: sheet_name.clone(),
                reference: Box::new(shifted_ref),
            })
        }
        // Literals don't contain references
        _ => Some(expr.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_rows_down_insert() {
        // Insert 2 rows at row 2, should shift B3 to B5
        let formula = "=A1+B3";
        let result = shift_formula_rows(formula, 2, 2);
        assert_eq!(result, Some("=A1+B5".to_string()));
    }

    #[test]
    fn test_shift_rows_up_delete() {
        // Delete 1 row at row 1, should shift B3 to B2
        let formula = "=A1+B3";
        let result = shift_formula_rows(formula, 1, -1);
        assert_eq!(result, Some("=A1+B2".to_string()));
    }

    #[test]
    fn test_absolute_row_no_shift() {
        // Absolute row reference should not shift
        let formula = "=A$1+B3";
        let result = shift_formula_rows(formula, 0, 2);
        assert_eq!(result, Some("=A$1+B5".to_string()));
    }

    #[test]
    fn test_mixed_reference_shift_relative_only() {
        // Mixed reference $A1 should shift row but not column
        let formula = "=$A1+B3";
        let result = shift_formula_rows(formula, 0, 2);
        assert_eq!(result, Some("=$A3+B5".to_string()));
    }

    #[test]
    fn test_fully_absolute_no_shift() {
        // Fully absolute reference should not shift
        let formula = "=$A$1+B3";
        let result = shift_formula_rows(formula, 0, 2);
        assert_eq!(result, Some("=$A$1+B5".to_string()));
    }

    #[test]
    fn test_range_shift_both_ends() {
        // Range A1:B10 should shift to A3:B12 when inserting 2 rows at row 0
        let formula = "=SUM(A1:B10)";
        let result = shift_formula_rows(formula, 0, 2);
        assert_eq!(result, Some("=SUM(A3:B12)".to_string()));
    }

    #[test]
    fn test_deleted_reference_returns_none() {
        // Deleting row 2-3, B3 should become invalid
        let formula = "=A1+B3";
        let result = shift_formula_rows(formula, 2, -2);
        assert_eq!(result, None);
    }

    #[test]
    fn test_deleted_reference_in_range() {
        // Deleting row 1 (0-indexed), A1 and B1 are both at row 0 so not affected
        // But if we delete starting at row 1 (1-indexed row 2), then row 1 (0-indexed) shifts up
        // Let's delete row 2 (1-indexed), which is row 1 (0-indexed)
        let formula = "=A2+B2";
        let result = shift_formula_rows(formula, 1, -1);
        assert_eq!(result, None);
    }

    #[test]
    fn test_negative_row_returns_none() {
        // Deleting rows that would make reference negative
        let formula = "=A1";
        let result = shift_formula_rows(formula, 0, -2);
        assert_eq!(result, None);
    }

    #[test]
    fn test_shift_cols_right_insert() {
        // Insert 2 columns at column 1 (B), should shift B1 to D1
        let formula = "=A1+B1";
        let result = shift_formula_cols(formula, 1, 2);
        assert_eq!(result, Some("=A1+D1".to_string()));
    }

    #[test]
    fn test_shift_cols_left_delete() {
        // Delete 1 column at column 1 (B), should shift C1 to B1
        let formula = "=A1+C1";
        let result = shift_formula_cols(formula, 1, -1);
        assert_eq!(result, Some("=A1+B1".to_string()));
    }

    #[test]
    fn test_absolute_col_no_shift() {
        // Absolute column reference should not shift
        let formula = "=$A1+B1";
        let result = shift_formula_cols(formula, 0, 2);
        assert_eq!(result, Some("=$A1+D1".to_string()));
    }

    #[test]
    fn test_mixed_col_reference() {
        // Mixed reference A$1 should shift column but not row
        let formula = "=A$1+B1";
        let result = shift_formula_cols(formula, 0, 2);
        assert_eq!(result, Some("=C$1+D1".to_string()));
    }

    #[test]
    fn test_deleted_col_reference_returns_none() {
        // Deleting column 1 (B), B1 should become invalid
        let formula = "=A1+B1";
        let result = shift_formula_cols(formula, 1, -1);
        assert_eq!(result, None);
    }

    #[test]
    fn test_complex_formula_shift() {
        // Complex formula with multiple references
        let formula = "=SUM(A1:B10)+C5*D6";
        let result = shift_formula_rows(formula, 3, 2);
        // A1, A2 don't shift (row < 3), B10, C5, D6 shift
        assert_eq!(result, Some("=SUM(A1:B12)+C7*D8".to_string()));
    }

    #[test]
    fn test_nested_functions() {
        // Nested function calls
        let formula = "=SUM(A1:A10,MAX(B1:B10))";
        let result = shift_formula_rows(formula, 0, 1);
        assert_eq!(result, Some("=SUM(A2:A11,MAX(B2:B11))".to_string()));
    }

    #[test]
    fn test_grouped_expression() {
        // Grouped expression with parentheses
        let formula = "=(A1+B1)*C1";
        let result = shift_formula_rows(formula, 0, 1);
        assert_eq!(result, Some("=(A2+B2)*C2".to_string()));
    }
}
