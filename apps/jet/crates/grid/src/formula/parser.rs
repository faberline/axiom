use crate::core::col_from_label;
use crate::formula::ast::{BinaryOp, Expr, UnaryOp};
use crate::formula::lexer::Token;

/// Parser for formula expressions
pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    /// Parse the token stream into an AST
    pub fn parse(&mut self) -> Result<Expr, String> {
        let expr = self.parse_expression()?;

        if !self.is_at_end() {
            return Err(format!("Unexpected token: {:?}", self.peek()));
        }

        Ok(expr)
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&Token::EOF)
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.position += 1;
        }
        self.tokens.get(self.position - 1).unwrap_or(&Token::EOF)
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), Token::EOF)
    }

    fn check(&self, token: &Token) -> bool {
        std::mem::discriminant(self.peek()) == std::mem::discriminant(token)
    }

    fn consume(&mut self, expected: &Token, message: &str) -> Result<&Token, String> {
        if self.check(expected) {
            Ok(self.advance())
        } else {
            Err(format!("{}: got {:?}", message, self.peek()))
        }
    }

    /// Parse expression with operator precedence
    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_concat()?;

        loop {
            let op = match self.peek() {
                Token::Equal => BinaryOp::Eq,
                Token::NotEqual => BinaryOp::Ne,
                Token::LessThan => BinaryOp::Lt,
                Token::GreaterThan => BinaryOp::Gt,
                Token::LessEqual => BinaryOp::Le,
                Token::GreaterEqual => BinaryOp::Ge,
                _ => break,
            };

            self.advance();
            let right = self.parse_concat()?;
            left = Expr::binary(left, op, right);
        }

        Ok(left)
    }

    fn parse_concat(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;

        while matches!(self.peek(), Token::Concat) {
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::binary(left, BinaryOp::Concat, right);
        }

        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let op = match self.peek() {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                _ => break,
            };

            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::binary(left, op, right);
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_power()?;

        loop {
            let op = match self.peek() {
                Token::Multiply => BinaryOp::Mul,
                Token::Divide => BinaryOp::Div,
                _ => break,
            };

            self.advance();
            let right = self.parse_power()?;
            left = Expr::binary(left, op, right);
        }

        Ok(left)
    }

    fn parse_power(&mut self) -> Result<Expr, String> {
        let left = self.parse_unary()?;

        if matches!(self.peek(), Token::Power) {
            self.advance();
            // Power is right-associative
            let right = self.parse_power()?;
            Ok(Expr::binary(left, BinaryOp::Pow, right))
        } else {
            Ok(left)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Token::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::unary(UnaryOp::Neg, operand))
            }
            Token::Plus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::unary(UnaryOp::Pos, operand))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        // Check for percent postfix
        if matches!(self.peek(), Token::Percent) {
            self.advance();
            expr = Expr::unary(UnaryOp::Percent, expr);
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            Token::Number(n) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Token::String(s) => {
                self.advance();
                Ok(Expr::String(s))
            }
            Token::Boolean(b) => {
                self.advance();
                Ok(Expr::Boolean(b))
            }
            Token::CellRef(ref_str) => {
                self.advance();
                self.parse_cell_ref_or_range(&ref_str)
            }
            Token::Identifier(name) => {
                self.advance();

                // Check if it's a function call
                if matches!(self.peek(), Token::LeftParen) {
                    self.parse_function_call(name)
                } else {
                    // Treat as cell reference or error
                    Err(format!("Unexpected identifier: {}", name))
                }
            }
            Token::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.consume(&Token::RightParen, "Expected ')'")?;
                Ok(Expr::Grouped(Box::new(expr)))
            }
            Token::Error(e) => {
                self.advance();
                Ok(Expr::Error(e))
            }
            _ => Err(format!("Unexpected token: {:?}", self.peek())),
        }
    }

    fn parse_cell_ref_or_range(&mut self, ref_str: &str) -> Result<Expr, String> {
        let cell_ref = parse_cell_reference(ref_str)?;

        // Check for range operator
        if matches!(self.peek(), Token::Colon) {
            self.advance();

            // Expect another cell reference
            if let Token::CellRef(end_ref) = self.peek().clone() {
                self.advance();
                let end_cell = parse_cell_reference(&end_ref)?;
                Ok(Expr::range(cell_ref, end_cell))
            } else {
                Err("Expected cell reference after ':'".to_string())
            }
        } else {
            Ok(cell_ref)
        }
    }

    fn parse_function_call(&mut self, name: String) -> Result<Expr, String> {
        self.consume(&Token::LeftParen, "Expected '(' after function name")?;

        let mut args = Vec::new();

        if !matches!(self.peek(), Token::RightParen) {
            loop {
                // Parse argument (could be a range or expression)
                let arg = self.parse_expression()?;
                args.push(arg);

                match self.peek() {
                    Token::Comma | Token::Semicolon => {
                        self.advance();
                    }
                    Token::RightParen => break,
                    _ => {
                        return Err(format!(
                            "Expected ',' or ')' in function call, got {:?}",
                            self.peek()
                        ))
                    }
                }
            }
        }

        self.consume(&Token::RightParen, "Expected ')' after function arguments")?;

        Ok(Expr::FunctionCall {
            name: name.to_uppercase(),
            args,
        })
    }
}

/// Parse a cell reference string (e.g., "A1", "$B$2") into an Expr::CellRef
fn parse_cell_reference(ref_str: &str) -> Result<Expr, String> {
    let mut chars = ref_str.chars().peekable();

    // Check for absolute column marker
    let abs_col = if chars.peek() == Some(&'$') {
        chars.next();
        true
    } else {
        false
    };

    // Read column letters
    let mut col_str = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            col_str.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if col_str.is_empty() {
        return Err(format!("Invalid cell reference: {}", ref_str));
    }

    // Check for absolute row marker
    let abs_row = if chars.peek() == Some(&'$') {
        chars.next();
        true
    } else {
        false
    };

    // Read row number
    let row_str: String = chars.collect();
    if row_str.is_empty() {
        return Err(format!("Invalid cell reference: {}", ref_str));
    }

    let col = col_from_label(&col_str).ok_or_else(|| format!("Invalid column: {}", col_str))?;

    let row: u32 = row_str
        .parse()
        .map_err(|_| format!("Invalid row: {}", row_str))?;

    if row == 0 {
        return Err("Row number must be >= 1".to_string());
    }

    Ok(Expr::CellRef {
        col,
        row: row - 1, // Convert to 0-indexed
        abs_col,
        abs_row,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::lexer::Lexer;

    fn parse(input: &str) -> Result<Expr, String> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    #[test]
    fn test_number() {
        let expr = parse("42").unwrap();
        assert_eq!(expr, Expr::Number(42.0));
    }

    #[test]
    fn test_arithmetic() {
        let expr = parse("1 + 2 * 3").unwrap();
        // Should be 1 + (2 * 3) due to precedence
        assert!(matches!(
            expr,
            Expr::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
    }

    #[test]
    fn test_cell_reference() {
        let expr = parse("A1").unwrap();
        assert!(matches!(expr, Expr::CellRef { col: 0, row: 0, .. }));
    }

    #[test]
    fn test_range() {
        let expr = parse("A1:B2").unwrap();
        assert!(matches!(expr, Expr::Range { .. }));
    }

    #[test]
    fn test_function_call() {
        let expr = parse("SUM(A1:A10)").unwrap();
        if let Expr::FunctionCall { name, args } = expr {
            assert_eq!(name, "SUM");
            assert_eq!(args.len(), 1);
            assert!(matches!(args[0], Expr::Range { .. }));
        } else {
            panic!("Expected function call");
        }
    }

    #[test]
    fn test_nested_function() {
        let expr = parse("SUM(A1, MAX(B1:B10))").unwrap();
        if let Expr::FunctionCall { name, args } = expr {
            assert_eq!(name, "SUM");
            assert_eq!(args.len(), 2);
        } else {
            panic!("Expected function call");
        }
    }

    #[test]
    fn test_comparison() {
        let expr = parse("A1 > 10").unwrap();
        assert!(matches!(
            expr,
            Expr::Binary {
                op: BinaryOp::Gt,
                ..
            }
        ));
    }

    #[test]
    fn test_parentheses() {
        let expr = parse("(1 + 2) * 3").unwrap();
        assert!(matches!(
            expr,
            Expr::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    }
}
