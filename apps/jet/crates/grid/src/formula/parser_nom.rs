//! Nom-based formula parser.
//!
//! This module provides a robust parser for spreadsheet formulas using nom combinators.
//! It parses directly from strings to AST, bypassing the lexer.

use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while, take_while1},
    character::complete::{char, multispace0, one_of},
    combinator::{map, opt, recognize, value},
    multi::{fold_many0, many0, separated_list0},
    sequence::{delimited, pair, tuple},
    IResult,
};

use crate::core::CellError;
use crate::formula::ast::{BinaryOp, Expr, UnaryOp};

// =============================================================================
// Error Type
// =============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Parse error at position {}: {}",
            self.position, self.message
        )
    }
}

impl std::error::Error for ParseError {}

// =============================================================================
// Helper Combinators
// =============================================================================

/// Skip whitespace
fn ws<'a, F, O>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(multispace0, inner, multispace0)
}

/// Parse a decimal number (integer or float)
fn parse_number(input: &str) -> IResult<&str, Expr> {
    let (input, num_str) = recognize(tuple((
        opt(char('-')),
        take_while1(|c: char| c.is_ascii_digit()),
        opt(pair(char('.'), take_while(|c: char| c.is_ascii_digit()))),
        opt(tuple((
            one_of("eE"),
            opt(one_of("+-")),
            take_while1(|c: char| c.is_ascii_digit()),
        ))),
    )))(input)?;

    let num: f64 = num_str.parse().unwrap_or(f64::NAN);
    Ok((input, Expr::Number(num)))
}

/// Parse a string literal (double-quoted)
fn parse_string(input: &str) -> IResult<&str, Expr> {
    let (input, _) = char('"')(input)?;
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    let mut consumed = 0;

    loop {
        match chars.next() {
            Some('"') => {
                // Check for escaped quote
                if chars.peek() == Some(&'"') {
                    result.push('"');
                    chars.next();
                    consumed += 2;
                } else {
                    consumed += 1;
                    break;
                }
            }
            Some(c) => {
                result.push(c);
                consumed += c.len_utf8();
            }
            None => {
                return Err(nom::Err::Error(nom::error::Error::new(
                    input,
                    nom::error::ErrorKind::Char,
                )));
            }
        }
    }

    Ok((&input[consumed..], Expr::String(result)))
}

/// Parse a boolean literal
fn parse_boolean(input: &str) -> IResult<&str, Expr> {
    alt((
        value(Expr::Boolean(true), tag_no_case("TRUE")),
        value(Expr::Boolean(false), tag_no_case("FALSE")),
    ))(input)
}

/// Parse an error literal
fn parse_error_literal(input: &str) -> IResult<&str, Expr> {
    alt((
        value(Expr::Error(CellError::DivisionByZero), tag("#DIV/0!")),
        value(Expr::Error(CellError::InvalidValue), tag("#VALUE!")),
        value(Expr::Error(CellError::InvalidReference), tag("#REF!")),
        value(Expr::Error(CellError::InvalidName), tag("#NAME?")),
        value(Expr::Error(CellError::NotAvailable), tag("#N/A")),
        value(Expr::Error(CellError::NullError), tag("#NULL!")),
        value(Expr::Error(CellError::NumError), tag("#NUM!")),
    ))(input)
}

/// Convert column letters to column index (A=0, B=1, ..., Z=25, AA=26, ...)
fn col_letters_to_index(letters: &str) -> u32 {
    let mut result: u32 = 0;
    for c in letters.chars() {
        let val = (c.to_ascii_uppercase() as u32) - ('A' as u32) + 1;
        result = result * 26 + val;
    }
    result.saturating_sub(1)
}

/// Parse a cell reference (e.g., A1, $B$2, AA10)
fn parse_cell_ref(input: &str) -> IResult<&str, Expr> {
    let (input, abs_col) = opt(char('$'))(input)?;
    let (input, col_letters) = take_while1(|c: char| c.is_ascii_alphabetic())(input)?;
    let (input, abs_row) = opt(char('$'))(input)?;
    let (input, row_digits) = take_while1(|c: char| c.is_ascii_digit())(input)?;

    let col = col_letters_to_index(col_letters);
    let row: u32 = row_digits.parse::<u32>().unwrap_or(1).saturating_sub(1);

    Ok((
        input,
        Expr::CellRef {
            col,
            row,
            abs_col: abs_col.is_some(),
            abs_row: abs_row.is_some(),
        },
    ))
}

/// Parse an identifier (function name)
fn parse_identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_ascii_alphanumeric() || c == '_')(input)
}

/// Parse a sheet name (quoted or unquoted)
/// Examples: Sheet1, 'Sheet Name', 'Sheet''s Data'
fn parse_sheet_name(input: &str) -> IResult<&str, String> {
    alt((
        // Quoted sheet name: 'Sheet Name' or 'Sheet''s Data'
        map(
            delimited(
                char('\''),
                recognize(many0(alt((
                    take_while1(|c: char| c != '\''),
                    map(tag("''"), |_| "'"), // escaped quote
                )))),
                char('\''),
            ),
            |s: &str| s.replace("''", "'"),
        ),
        // Unquoted sheet name: alphanumeric and underscores
        map(
            take_while1(|c: char| c.is_ascii_alphanumeric() || c == '_'),
            |s: &str| s.to_string(),
        ),
    ))(input)
}

// =============================================================================
// Operator Parsers
// =============================================================================

fn parse_comparison_op(input: &str) -> IResult<&str, BinaryOp> {
    alt((
        value(BinaryOp::Le, tag("<=")),
        value(BinaryOp::Ge, tag(">=")),
        value(BinaryOp::Ne, tag("<>")),
        value(BinaryOp::Lt, tag("<")),
        value(BinaryOp::Gt, tag(">")),
        value(BinaryOp::Eq, tag("=")),
    ))(input)
}

fn parse_additive_op(input: &str) -> IResult<&str, BinaryOp> {
    alt((
        value(BinaryOp::Add, char('+')),
        value(BinaryOp::Sub, char('-')),
    ))(input)
}

fn parse_multiplicative_op(input: &str) -> IResult<&str, BinaryOp> {
    alt((
        value(BinaryOp::Mul, char('*')),
        value(BinaryOp::Div, char('/')),
    ))(input)
}

fn parse_concat_op(input: &str) -> IResult<&str, BinaryOp> {
    value(BinaryOp::Concat, char('&'))(input)
}

// =============================================================================
// Expression Parsers (Precedence Climbing)
// =============================================================================

/// Parse a primary expression (literals, cell refs, function calls, parentheses)
fn parse_primary(input: &str) -> IResult<&str, Expr> {
    let (input, _) = multispace0(input)?;

    alt((
        // Parenthesized expression
        map(delimited(char('('), parse_expression, char(')')), |e| {
            Expr::Grouped(Box::new(e))
        }),
        // Error literal
        parse_error_literal,
        // Boolean (before identifier to avoid conflict)
        parse_boolean,
        // String literal
        parse_string,
        // Number (try before cell ref because negative numbers start with -)
        parse_number,
        // Function call or cell reference
        parse_cell_ref_or_function,
    ))(input)
}

/// Parse either a cell reference, range, sheet reference, or function call
fn parse_cell_ref_or_function(input: &str) -> IResult<&str, Expr> {
    // First, try to parse as sheet reference (Sheet1!A1 or 'Sheet Name'!A1:B2)
    if let Ok((remaining, sheet_name)) = parse_sheet_name(input) {
        // Check for ! after sheet name
        if let Ok((remaining, _)) = char::<&str, nom::error::Error<&str>>('!')(remaining) {
            // Parse the cell reference or range after !
            if let Ok((remaining, cell_ref)) = parse_cell_ref(remaining) {
                // Check if followed by a colon (range)
                let (remaining, _) = multispace0(remaining)?;
                if let Ok((remaining, _)) = char::<&str, nom::error::Error<&str>>(':')(remaining) {
                    let (remaining, _) = multispace0(remaining)?;
                    let (remaining, end_ref) = parse_cell_ref(remaining)?;
                    return Ok((
                        remaining,
                        Expr::SheetRef {
                            sheet_name,
                            reference: Box::new(Expr::Range {
                                start: Box::new(cell_ref),
                                end: Box::new(end_ref),
                            }),
                        },
                    ));
                }
                return Ok((
                    remaining,
                    Expr::SheetRef {
                        sheet_name,
                        reference: Box::new(cell_ref),
                    },
                ));
            }
        }
    }

    // Try to parse as cell reference first
    if let Ok((remaining, cell_ref)) = parse_cell_ref(input) {
        // Check if followed by a colon (range)
        let (remaining, _) = multispace0(remaining)?;
        if let Ok((remaining, _)) = char::<&str, nom::error::Error<&str>>(':')(remaining) {
            let (remaining, _) = multispace0(remaining)?;
            let (remaining, end_ref) = parse_cell_ref(remaining)?;
            return Ok((
                remaining,
                Expr::Range {
                    start: Box::new(cell_ref),
                    end: Box::new(end_ref),
                },
            ));
        }
        return Ok((remaining, cell_ref));
    }

    // Try to parse as function call
    let (input, name) = parse_identifier(input)?;
    let (input, _) = multispace0(input)?;

    // Check for opening paren
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?;

    // Parse arguments
    let (input, args) = separated_list0(ws(alt((char(','), char(';')))), parse_expression)(input)?;

    let (input, _) = multispace0(input)?;
    let (input, _) = char(')')(input)?;

    Ok((
        input,
        Expr::FunctionCall {
            name: name.to_uppercase(),
            args,
        },
    ))
}

/// Parse a postfix expression (percent)
fn parse_postfix(input: &str) -> IResult<&str, Expr> {
    let (input, expr) = parse_primary(input)?;
    let (input, _) = multispace0(input)?;

    let (input, percents) = many0(char('%'))(input)?;

    let result = percents.into_iter().fold(expr, |acc, _| Expr::Unary {
        op: UnaryOp::Percent,
        operand: Box::new(acc),
    });

    Ok((input, result))
}

/// Parse a unary expression (prefix - or +)
fn parse_unary(input: &str) -> IResult<&str, Expr> {
    let (input, _) = multispace0(input)?;

    alt((
        map(pair(char('-'), parse_unary), |(_, e)| Expr::Unary {
            op: UnaryOp::Neg,
            operand: Box::new(e),
        }),
        map(pair(char('+'), parse_unary), |(_, e)| Expr::Unary {
            op: UnaryOp::Pos,
            operand: Box::new(e),
        }),
        parse_postfix,
    ))(input)
}

/// Parse power expressions (right-associative)
fn parse_power(input: &str) -> IResult<&str, Expr> {
    let (input, base) = parse_unary(input)?;
    let (input, _) = multispace0(input)?;

    if let Ok((input, _)) = char::<&str, nom::error::Error<&str>>('^')(input) {
        let (input, _) = multispace0(input)?;
        let (input, exp) = parse_power(input)?; // Right-associative recursion
        Ok((
            input,
            Expr::Binary {
                left: Box::new(base),
                op: BinaryOp::Pow,
                right: Box::new(exp),
            },
        ))
    } else {
        Ok((input, base))
    }
}

/// Parse multiplicative expressions (*, /)
fn parse_multiplicative(input: &str) -> IResult<&str, Expr> {
    let (input, init) = parse_power(input)?;

    fold_many0(
        pair(ws(parse_multiplicative_op), parse_power),
        move || init.clone(),
        |acc, (op, val)| Expr::Binary {
            left: Box::new(acc),
            op,
            right: Box::new(val),
        },
    )(input)
}

/// Parse additive expressions (+, -)
fn parse_additive(input: &str) -> IResult<&str, Expr> {
    let (input, init) = parse_multiplicative(input)?;

    fold_many0(
        pair(ws(parse_additive_op), parse_multiplicative),
        move || init.clone(),
        |acc, (op, val)| Expr::Binary {
            left: Box::new(acc),
            op,
            right: Box::new(val),
        },
    )(input)
}

/// Parse concatenation expressions (&)
fn parse_concat(input: &str) -> IResult<&str, Expr> {
    let (input, init) = parse_additive(input)?;

    fold_many0(
        pair(ws(parse_concat_op), parse_additive),
        move || init.clone(),
        |acc, (op, val)| Expr::Binary {
            left: Box::new(acc),
            op,
            right: Box::new(val),
        },
    )(input)
}

/// Parse comparison expressions (=, <>, <, >, <=, >=)
fn parse_comparison(input: &str) -> IResult<&str, Expr> {
    let (input, init) = parse_concat(input)?;

    fold_many0(
        pair(ws(parse_comparison_op), parse_concat),
        move || init.clone(),
        |acc, (op, val)| Expr::Binary {
            left: Box::new(acc),
            op,
            right: Box::new(val),
        },
    )(input)
}

/// Parse a complete expression
pub fn parse_expression(input: &str) -> IResult<&str, Expr> {
    let (input, _) = multispace0(input)?;
    parse_comparison(input)
}

// =============================================================================
// Public API
// =============================================================================

/// Parser struct for compatibility with existing code
pub struct NomParser;

impl NomParser {
    pub fn new() -> Self {
        NomParser
    }

    /// Parse a formula string into an AST
    pub fn parse(&self, input: &str) -> Result<Expr, ParseError> {
        // Strip leading '=' if present
        let input = input.strip_prefix('=').unwrap_or(input);

        match parse_expression(input) {
            Ok((remaining, expr)) => {
                // Check that all input was consumed
                let remaining = remaining.trim();
                if remaining.is_empty() {
                    Ok(expr)
                } else {
                    Err(ParseError {
                        message: format!("Unexpected input: '{}'", remaining),
                        position: input.len() - remaining.len(),
                    })
                }
            }
            Err(e) => Err(ParseError {
                message: format!("Parse error: {:?}", e),
                position: 0,
            }),
        }
    }
}

impl Default for NomParser {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<Expr, ParseError> {
        NomParser::new().parse(input)
    }

    #[test]
    fn test_number() {
        assert_eq!(parse("123"), Ok(Expr::Number(123.0)));
        assert_eq!(parse("3.14"), Ok(Expr::Number(3.14)));
        assert_eq!(
            parse("-5"),
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(Expr::Number(5.0)),
            })
        );
        assert_eq!(parse("1e10"), Ok(Expr::Number(1e10)));
        assert_eq!(parse("1.5e-3"), Ok(Expr::Number(1.5e-3)));
    }

    #[test]
    fn test_string() {
        assert_eq!(parse("\"hello\""), Ok(Expr::String("hello".to_string())));
        assert_eq!(parse("\"\""), Ok(Expr::String("".to_string())));
        assert_eq!(
            parse("\"say \"\"hi\"\"\""),
            Ok(Expr::String("say \"hi\"".to_string()))
        );
    }

    #[test]
    fn test_boolean() {
        assert_eq!(parse("TRUE"), Ok(Expr::Boolean(true)));
        assert_eq!(parse("FALSE"), Ok(Expr::Boolean(false)));
        assert_eq!(parse("true"), Ok(Expr::Boolean(true)));
        assert_eq!(parse("false"), Ok(Expr::Boolean(false)));
    }

    #[test]
    fn test_cell_reference() {
        assert_eq!(
            parse("A1"),
            Ok(Expr::CellRef {
                col: 0,
                row: 0,
                abs_col: false,
                abs_row: false
            })
        );
        assert_eq!(
            parse("B2"),
            Ok(Expr::CellRef {
                col: 1,
                row: 1,
                abs_col: false,
                abs_row: false
            })
        );
        assert_eq!(
            parse("AA10"),
            Ok(Expr::CellRef {
                col: 26,
                row: 9,
                abs_col: false,
                abs_row: false
            })
        );
        assert_eq!(
            parse("$A$1"),
            Ok(Expr::CellRef {
                col: 0,
                row: 0,
                abs_col: true,
                abs_row: true
            })
        );
        assert_eq!(
            parse("A$1"),
            Ok(Expr::CellRef {
                col: 0,
                row: 0,
                abs_col: false,
                abs_row: true
            })
        );
        assert_eq!(
            parse("$A1"),
            Ok(Expr::CellRef {
                col: 0,
                row: 0,
                abs_col: true,
                abs_row: false
            })
        );
    }

    #[test]
    fn test_range() {
        let result = parse("A1:B2");
        assert!(matches!(result, Ok(Expr::Range { .. })));

        if let Ok(Expr::Range { start, end }) = result {
            assert_eq!(
                *start,
                Expr::CellRef {
                    col: 0,
                    row: 0,
                    abs_col: false,
                    abs_row: false
                }
            );
            assert_eq!(
                *end,
                Expr::CellRef {
                    col: 1,
                    row: 1,
                    abs_col: false,
                    abs_row: false
                }
            );
        }
    }

    #[test]
    fn test_arithmetic() {
        // Test precedence: 1 + 2 * 3 = 1 + (2 * 3) = 7
        let result = parse("1 + 2 * 3");
        assert!(result.is_ok());

        if let Ok(Expr::Binary { left, op, right }) = result {
            assert_eq!(op, BinaryOp::Add);
            assert_eq!(*left, Expr::Number(1.0));
            if let Expr::Binary {
                left: l2,
                op: op2,
                right: r2,
            } = *right
            {
                assert_eq!(op2, BinaryOp::Mul);
                assert_eq!(*l2, Expr::Number(2.0));
                assert_eq!(*r2, Expr::Number(3.0));
            }
        }
    }

    #[test]
    fn test_parentheses() {
        // (1 + 2) * 3
        let result = parse("(1 + 2) * 3");
        assert!(result.is_ok());

        if let Ok(Expr::Binary { left, op, right }) = result {
            assert_eq!(op, BinaryOp::Mul);
            assert!(matches!(*left, Expr::Grouped(_)));
            assert_eq!(*right, Expr::Number(3.0));
        }
    }

    #[test]
    fn test_function_call() {
        let result = parse("SUM(A1:A10)");
        assert!(result.is_ok());

        if let Ok(Expr::FunctionCall { name, args }) = result {
            assert_eq!(name, "SUM");
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], Expr::Range { .. }));
        }
    }

    #[test]
    fn test_nested_function() {
        let result = parse("SUM(A1, MAX(B1:B10))");
        assert!(result.is_ok());

        if let Ok(Expr::FunctionCall { name, args }) = result {
            assert_eq!(name, "SUM");
            assert_eq!(args.len(), 2);
            assert!(matches!(&args[1], Expr::FunctionCall { .. }));
        }
    }

    #[test]
    fn test_comparison() {
        let result = parse("A1 > 0");
        assert!(result.is_ok());

        if let Ok(Expr::Binary { left, op, right }) = result {
            assert_eq!(op, BinaryOp::Gt);
            assert!(matches!(*left, Expr::CellRef { .. }));
            assert_eq!(*right, Expr::Number(0.0));
        }
    }

    #[test]
    fn test_complex_formula() {
        // IF(A1>0, A1*2, 0) would need to be a function call
        let result = parse("IF(A1>0, A1*2, 0)");
        assert!(result.is_ok());

        if let Ok(Expr::FunctionCall { name, args }) = result {
            assert_eq!(name, "IF");
            assert_eq!(args.len(), 3);
        }
    }

    #[test]
    fn test_power_right_associative() {
        // 2^3^2 should be 2^(3^2) = 2^9 = 512, not (2^3)^2 = 8^2 = 64
        let result = parse("2^3^2");
        assert!(result.is_ok());

        if let Ok(Expr::Binary { left, op, right }) = result {
            assert_eq!(op, BinaryOp::Pow);
            assert_eq!(*left, Expr::Number(2.0));
            // right should be 3^2
            if let Expr::Binary {
                left: l2,
                op: op2,
                right: r2,
            } = *right
            {
                assert_eq!(op2, BinaryOp::Pow);
                assert_eq!(*l2, Expr::Number(3.0));
                assert_eq!(*r2, Expr::Number(2.0));
            }
        }
    }

    #[test]
    fn test_percent() {
        let result = parse("50%");
        assert!(result.is_ok());

        if let Ok(Expr::Unary { op, operand }) = result {
            assert_eq!(op, UnaryOp::Percent);
            assert_eq!(*operand, Expr::Number(50.0));
        }
    }

    #[test]
    fn test_concat() {
        let result = parse("\"Hello\" & \" \" & \"World\"");
        assert!(result.is_ok());

        if let Ok(Expr::Binary { op, .. }) = result {
            assert_eq!(op, BinaryOp::Concat);
        }
    }

    #[test]
    fn test_with_leading_equals() {
        assert_eq!(parse("=123"), Ok(Expr::Number(123.0)));
        assert_eq!(
            parse("=A1"),
            Ok(Expr::CellRef {
                col: 0,
                row: 0,
                abs_col: false,
                abs_row: false
            })
        );
    }

    #[test]
    fn test_sheet_reference() {
        let result = parse("Sheet1!A1");
        assert!(result.is_ok());
        if let Ok(Expr::SheetRef {
            sheet_name,
            reference,
        }) = result
        {
            assert_eq!(sheet_name, "Sheet1");
            assert!(matches!(*reference, Expr::CellRef { col: 0, row: 0, .. }));
        } else {
            panic!("Expected SheetRef");
        }
    }

    #[test]
    fn test_sheet_reference_range() {
        let result = parse("Sheet2!A1:B5");
        assert!(result.is_ok());
        if let Ok(Expr::SheetRef {
            sheet_name,
            reference,
        }) = result
        {
            assert_eq!(sheet_name, "Sheet2");
            assert!(matches!(*reference, Expr::Range { .. }));
        } else {
            panic!("Expected SheetRef with Range");
        }
    }

    #[test]
    fn test_quoted_sheet_reference() {
        let result = parse("'My Sheet'!C3");
        assert!(result.is_ok());
        if let Ok(Expr::SheetRef {
            sheet_name,
            reference,
        }) = result
        {
            assert_eq!(sheet_name, "My Sheet");
            assert!(matches!(*reference, Expr::CellRef { col: 2, row: 2, .. }));
        } else {
            panic!("Expected SheetRef");
        }
    }

    #[test]
    fn test_sheet_in_function() {
        let result = parse("SUM(Sheet1!A1:A10)");
        assert!(result.is_ok());
        if let Ok(Expr::FunctionCall { name, args }) = result {
            assert_eq!(name, "SUM");
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], Expr::SheetRef { .. }));
        } else {
            panic!("Expected FunctionCall with SheetRef");
        }
    }
}
