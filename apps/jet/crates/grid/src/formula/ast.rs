use crate::core::CellError;

/// Abstract Syntax Tree for formula expressions
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // Literals
    Number(f64),
    String(String),
    Boolean(bool),
    Error(CellError),

    // Cell reference (e.g., A1, $B$2)
    CellRef {
        col: u32,
        row: u32,
        abs_col: bool, // $A1 vs A1
        abs_row: bool, // A$1 vs A1
    },

    // Range reference (e.g., A1:B10)
    Range {
        start: Box<Expr>, // CellRef
        end: Box<Expr>,   // CellRef
    },

    // Sheet reference (e.g., Sheet1!A1)
    SheetRef {
        sheet_name: String,
        reference: Box<Expr>,
    },

    // Binary operation
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },

    // Unary operation
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    // Function call (e.g., SUM(A1:A10))
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },

    // Parenthesized expression
    Grouped(Box<Expr>),
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Pow,

    // String
    Concat,

    // Comparison
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

impl BinaryOp {
    /// Get the precedence of this operator (higher = binds tighter)
    pub fn precedence(&self) -> u8 {
        match self {
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::Le
            | BinaryOp::Ge => 1,
            BinaryOp::Concat => 2,
            BinaryOp::Add | BinaryOp::Sub => 3,
            BinaryOp::Mul | BinaryOp::Div => 4,
            BinaryOp::Pow => 5,
        }
    }

    /// Check if this operator is right-associative
    pub fn is_right_associative(&self) -> bool {
        matches!(self, BinaryOp::Pow)
    }
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,     // -
    Pos,     // +
    Percent, // %
}

impl Expr {
    /// Create a number expression
    pub fn number(n: f64) -> Self {
        Expr::Number(n)
    }

    /// Create a string expression
    pub fn string(s: impl Into<String>) -> Self {
        Expr::String(s.into())
    }

    /// Create a cell reference expression
    pub fn cell_ref(col: u32, row: u32) -> Self {
        Expr::CellRef {
            col,
            row,
            abs_col: false,
            abs_row: false,
        }
    }

    /// Create a binary expression
    pub fn binary(left: Expr, op: BinaryOp, right: Expr) -> Self {
        Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    /// Create a unary expression
    pub fn unary(op: UnaryOp, operand: Expr) -> Self {
        Expr::Unary {
            op,
            operand: Box::new(operand),
        }
    }

    /// Create a function call expression
    pub fn function(name: impl Into<String>, args: Vec<Expr>) -> Self {
        Expr::FunctionCall {
            name: name.into(),
            args,
        }
    }

    /// Create a range expression
    pub fn range(start: Expr, end: Expr) -> Self {
        Expr::Range {
            start: Box::new(start),
            end: Box::new(end),
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Number(n) => {
                // Format numbers without unnecessary decimals
                if n.fract() == 0.0 && n.is_finite() {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            Expr::String(s) => write!(f, "\"{}\"", s.replace('"', "\"\"")),
            Expr::Boolean(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            Expr::Error(e) => write!(f, "{:?}", e),
            Expr::CellRef {
                col,
                row,
                abs_col,
                abs_row,
            } => {
                use crate::core::col_to_label;
                let col_str = col_to_label(*col);
                let row_str = row + 1; // Convert back to 1-indexed
                write!(
                    f,
                    "{}{}{}{}",
                    if *abs_col { "$" } else { "" },
                    col_str,
                    if *abs_row { "$" } else { "" },
                    row_str
                )
            }
            Expr::Range { start, end } => write!(f, "{}:{}", start, end),
            Expr::SheetRef {
                sheet_name,
                reference,
            } => {
                // Quote sheet name if it contains spaces or special chars
                if sheet_name.contains(' ') || sheet_name.contains('!') {
                    write!(f, "'{}'!{}", sheet_name, reference)
                } else {
                    write!(f, "{}!{}", sheet_name, reference)
                }
            }
            Expr::Binary { left, op, right } => {
                write!(f, "{}{}{}", left, op, right)
            }
            Expr::Unary { op, operand } => match op {
                UnaryOp::Neg => write!(f, "-{}", operand),
                UnaryOp::Pos => write!(f, "+{}", operand),
                UnaryOp::Percent => write!(f, "{}%", operand),
            },
            Expr::FunctionCall { name, args } => {
                write!(f, "{}(", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expr::Grouped(inner) => write!(f, "({})", inner),
        }
    }
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryOp::Add => write!(f, "+"),
            BinaryOp::Sub => write!(f, "-"),
            BinaryOp::Mul => write!(f, "*"),
            BinaryOp::Div => write!(f, "/"),
            BinaryOp::Pow => write!(f, "^"),
            BinaryOp::Concat => write!(f, "&"),
            BinaryOp::Eq => write!(f, "="),
            BinaryOp::Ne => write!(f, "<>"),
            BinaryOp::Lt => write!(f, "<"),
            BinaryOp::Gt => write!(f, ">"),
            BinaryOp::Le => write!(f, "<="),
            BinaryOp::Ge => write!(f, ">="),
        }
    }
}
