use crate::core::CellError;

/// Token types for formula parsing
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Number(f64),
    String(String),
    Boolean(bool),

    // References
    CellRef(String), // A1, B2, $A$1, etc.

    // Operators
    Plus,
    Minus,
    Multiply,
    Divide,
    Power,   // ^
    Percent, // %
    Concat,  // &

    // Comparison
    Equal,        // =
    NotEqual,     // <>
    LessThan,     // <
    GreaterThan,  // >
    LessEqual,    // <=
    GreaterEqual, // >=

    // Delimiters
    LeftParen,
    RightParen,
    Comma,
    Colon,
    Semicolon,
    Exclaim, // ! for sheet references

    // Identifier (function names)
    Identifier(String),

    // Error token
    Error(CellError),

    // End of input
    EOF,
}

/// Lexer for tokenizing formula expressions
pub struct Lexer {
    input: Vec<char>,
    position: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
        }
    }

    /// Tokenize the entire input
    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace();
            if self.position >= self.input.len() {
                break;
            }

            let token = self.next_token()?;
            if matches!(token, Token::EOF) {
                break;
            }
            tokens.push(token);
        }

        tokens.push(Token::EOF);
        Ok(tokens)
    }

    fn skip_whitespace(&mut self) {
        while self.position < self.input.len() && self.input[self.position].is_whitespace() {
            self.position += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.position).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        self.position += 1;
        c
    }

    fn next_token(&mut self) -> Result<Token, String> {
        let c = match self.peek() {
            Some(c) => c,
            None => return Ok(Token::EOF),
        };

        match c {
            // Operators
            '+' => {
                self.advance();
                Ok(Token::Plus)
            }
            '-' => {
                self.advance();
                Ok(Token::Minus)
            }
            '*' => {
                self.advance();
                Ok(Token::Multiply)
            }
            '/' => {
                self.advance();
                Ok(Token::Divide)
            }
            '^' => {
                self.advance();
                Ok(Token::Power)
            }
            '%' => {
                self.advance();
                Ok(Token::Percent)
            }
            '&' => {
                self.advance();
                Ok(Token::Concat)
            }

            // Comparison
            '=' => {
                self.advance();
                Ok(Token::Equal)
            }
            '<' => {
                self.advance();
                match self.peek() {
                    Some('>') => {
                        self.advance();
                        Ok(Token::NotEqual)
                    }
                    Some('=') => {
                        self.advance();
                        Ok(Token::LessEqual)
                    }
                    _ => Ok(Token::LessThan),
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::GreaterEqual)
                } else {
                    Ok(Token::GreaterThan)
                }
            }

            // Delimiters
            '(' => {
                self.advance();
                Ok(Token::LeftParen)
            }
            ')' => {
                self.advance();
                Ok(Token::RightParen)
            }
            ',' => {
                self.advance();
                Ok(Token::Comma)
            }
            ':' => {
                self.advance();
                Ok(Token::Colon)
            }
            ';' => {
                self.advance();
                Ok(Token::Semicolon)
            }
            '!' => {
                self.advance();
                Ok(Token::Exclaim)
            }

            // String literal
            '"' => self.read_string(),

            // Number
            '0'..='9' | '.' => self.read_number(),

            // Cell reference or identifier (function name)
            'A'..='Z' | 'a'..='z' | '_' | '$' => self.read_identifier_or_ref(),

            _ => Err(format!("Unexpected character: {}", c)),
        }
    }

    fn read_string(&mut self) -> Result<Token, String> {
        self.advance(); // Skip opening quote
        let mut s = String::new();

        while let Some(c) = self.peek() {
            if c == '"' {
                // Check for escaped quote
                if self.peek_next() == Some('"') {
                    s.push('"');
                    self.advance();
                    self.advance();
                } else {
                    self.advance(); // Skip closing quote
                    return Ok(Token::String(s));
                }
            } else {
                s.push(c);
                self.advance();
            }
        }

        Err("Unterminated string".to_string())
    }

    fn read_number(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        let mut has_dot = false;
        let mut has_e = false;

        while let Some(c) = self.peek() {
            match c {
                '0'..='9' => {
                    s.push(c);
                    self.advance();
                }
                '.' if !has_dot && !has_e => {
                    has_dot = true;
                    s.push(c);
                    self.advance();
                }
                'e' | 'E' if !has_e => {
                    has_e = true;
                    s.push(c);
                    self.advance();
                    // Optional sign after E
                    if let Some('+') | Some('-') = self.peek() {
                        s.push(self.advance().unwrap());
                    }
                }
                _ => break,
            }
        }

        s.parse::<f64>()
            .map(Token::Number)
            .map_err(|_| format!("Invalid number: {}", s))
    }

    fn read_identifier_or_ref(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        let mut has_dollar = false;
        let mut has_letter = false;
        let mut has_digit = false;

        while let Some(c) = self.peek() {
            match c {
                'A'..='Z' | 'a'..='z' | '_' => {
                    has_letter = true;
                    s.push(c.to_ascii_uppercase());
                    self.advance();
                }
                '0'..='9' => {
                    has_digit = true;
                    s.push(c);
                    self.advance();
                }
                '$' => {
                    has_dollar = true;
                    s.push(c);
                    self.advance();
                }
                _ => break,
            }
        }

        // Check for boolean
        match s.as_str() {
            "TRUE" => return Ok(Token::Boolean(true)),
            "FALSE" => return Ok(Token::Boolean(false)),
            _ => {}
        }

        // Determine if this is a cell reference or identifier
        // Cell refs have format: [[$]COL[[$]ROW]] like A1, $A$1, B2, AA100
        if is_cell_reference(&s) || has_dollar {
            Ok(Token::CellRef(s))
        } else if has_letter && !has_digit {
            // Pure letters = function name or identifier
            Ok(Token::Identifier(s))
        } else if has_letter && has_digit {
            // Mixed = could be cell ref
            Ok(Token::CellRef(s))
        } else {
            Ok(Token::Identifier(s))
        }
    }
}

/// Check if a string looks like a cell reference
fn is_cell_reference(s: &str) -> bool {
    // Remove $ signs for checking
    let clean: String = s.chars().filter(|c| *c != '$').collect();

    // Should have letters followed by numbers
    let mut chars = clean.chars().peekable();

    // Must start with letters
    let mut has_letters = false;
    while let Some(c) = chars.peek() {
        if c.is_ascii_alphabetic() {
            has_letters = true;
            chars.next();
        } else {
            break;
        }
    }

    if !has_letters {
        return false;
    }

    // Must end with numbers
    let mut has_numbers = false;
    while let Some(c) = chars.peek() {
        if c.is_ascii_digit() {
            has_numbers = true;
            chars.next();
        } else {
            return false; // Invalid character
        }
    }

    has_numbers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let mut lexer = Lexer::new("1 + 2 * 3");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Number(1.0),
                Token::Plus,
                Token::Number(2.0),
                Token::Multiply,
                Token::Number(3.0),
                Token::EOF,
            ]
        );
    }

    #[test]
    fn test_cell_references() {
        let mut lexer = Lexer::new("A1 + B2 + $C$3");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::CellRef("A1".to_string()),
                Token::Plus,
                Token::CellRef("B2".to_string()),
                Token::Plus,
                Token::CellRef("$C$3".to_string()),
                Token::EOF,
            ]
        );
    }

    #[test]
    fn test_function_call() {
        let mut lexer = Lexer::new("SUM(A1:A10)");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::Identifier("SUM".to_string()),
                Token::LeftParen,
                Token::CellRef("A1".to_string()),
                Token::Colon,
                Token::CellRef("A10".to_string()),
                Token::RightParen,
                Token::EOF,
            ]
        );
    }

    #[test]
    fn test_string_literal() {
        let mut lexer = Lexer::new("\"Hello World\"");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![Token::String("Hello World".to_string()), Token::EOF,]
        );
    }

    #[test]
    fn test_comparison_operators() {
        let mut lexer = Lexer::new("A1 <> B1");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(
            tokens,
            vec![
                Token::CellRef("A1".to_string()),
                Token::NotEqual,
                Token::CellRef("B1".to_string()),
                Token::EOF,
            ]
        );
    }
}
