use crate::error::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    Identifier(String),
    String(String),
    True,
    False,
    Null,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Bang,
    Dot,
    SafeNavigation, // &.
    Ellipsis,
    LParen,
    RParen,
    Comma,
    Colon,
    DoubleColon,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Greater,
    Less,
    Ge,
    Le,
    EqEq,
    NotEq,
    And,
    Or,
    AndAnd,
    OrOr,
    QMark,
    Semicolon,
    ColonEquals,
    Eof,
}

#[derive(Clone)]
pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
    last_start: usize,
    last_end: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
            last_start: 0,
            last_end: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn number(&mut self, first: u8) -> Result<Token, Error> {
        let start = self.pos - 1;
        let mut end = self.pos;
        let mut has_dot = first == b'.';
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' => {
                    end += 1;
                    self.pos += 1;
                }
                b'.' if !has_dot => {
                    // Only consume the dot if it's followed by a digit (for decimals like 1.23)
                    // Don't consume it if it's followed by a letter (for method calls like 1.abs)
                    if let Some(&next) = self.input.get(self.pos + 1) {
                        if matches!(next, b'0'..=b'9') {
                            has_dot = true;
                            end += 1;
                            self.pos += 1;
                        } else {
                            // Don't consume the dot - it's likely a method call
                            break;
                        }
                    } else {
                        // End of input, consume the dot as it could be a valid decimal like "1."
                        has_dot = true;
                        end += 1;
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
        let s = std::str::from_utf8(&self.input[start..end]).unwrap();
        let n: f64 = s
            .parse()
            .map_err(|_| Error::new("Invalid number", Some(start)))?;
        self.last_start = start;
        self.last_end = end;
        Ok(Token::Number(n))
    }

    fn identifier(&mut self, _first: u8) -> Result<Token, Error> {
        let start = self.pos - 1;
        let mut end = self.pos;
        while let Some(c) = self.peek() {
            match c {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' => {
                    end += 1;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        let s = std::str::from_utf8(&self.input[start..end])
            .unwrap()
            .to_string();
        let up = s.to_uppercase();
        self.last_start = start;
        self.last_end = end;
        Ok(match up.as_str() {
            "TRUE" => Token::True,
            "FALSE" => Token::False,
            "NULL" => Token::Null,
            _ => Token::Identifier(s),
        })
    }

    fn string(&mut self, quote: u8) -> Result<Token, Error> {
        let start0 = self.pos - 1;
        // consume until matching quote, support escapes \" \\ \n \t; preserve UTF-8 bytes
        let mut buf: Vec<u8> = Vec::new();
        while let Some(c) = self.bump() {
            if c == quote {
                self.last_start = start0;
                self.last_end = self.pos;
                return Ok(Token::String(String::from_utf8(buf).map_err(|_| {
                    Error::new("Invalid UTF-8 in string", Some(self.pos))
                })?));
            }
            if c == b'\\' {
                match self.bump() {
                    Some(b'\\') => buf.push(b'\\'),
                    Some(b'"') => buf.push(b'"'),
                    Some(b'\'') => buf.push(b'\''),
                    Some(b'n') => buf.push(b'\n'),
                    Some(b't') => buf.push(b'\t'),
                    Some(x) => buf.push(x),
                    None => {
                        return Err(Error::new("Unterminated escape in string", Some(self.pos)))
                    }
                }
            } else {
                buf.push(c);
            }
        }
        Err(Error::new("Unterminated string literal", Some(self.pos)))
    }

    pub fn next_token(&mut self) -> Result<Token, Error> {
        self.skip_ws();
        let ch = match self.bump() {
            Some(c) => c,
            None => return Ok(Token::Eof),
        };

        let tok = match ch {
            b'0'..=b'9' => return self.number(ch),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => return self.identifier(ch),
            b'.' => {
                // Ellipsis '...'
                if matches!(self.peek(), Some(b'.')) {
                    let save = self.pos;
                    self.bump();
                    if matches!(self.peek(), Some(b'.')) {
                        self.bump();
                        self.last_start = self.pos - 3;
                        self.last_end = self.pos;
                        Token::Ellipsis
                    } else {
                        // Was just two dots, backtrack second
                        self.pos = save;
                        self.last_start = self.pos - 1;
                        self.last_end = self.pos;
                        Token::Dot
                    }
                } else if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return self.number(ch);
                } else {
                    self.last_start = self.pos - 1;
                    self.last_end = self.pos;
                    Token::Dot
                }
            }
            b'+' => Token::Plus,
            b'-' => Token::Minus,
            b'*' => Token::Star,
            b'/' => Token::Slash,
            b'%' => Token::Percent,
            b'^' => Token::Caret,
            b'"' => return self.string(ch),
            b'\'' => return self.string(ch),
            b'!' => {
                if matches!(self.peek(), Some(b'=')) {
                    self.bump();
                    Token::NotEq
                } else {
                    Token::Bang
                }
            }
            b'?' => Token::QMark,
            b'(' => Token::LParen,
            b')' => Token::RParen,
            b'[' => Token::LBracket,
            b']' => Token::RBracket,
            b'{' => Token::LBrace,
            b'}' => Token::RBrace,
            b',' => Token::Comma,
            b':' => {
                if matches!(self.peek(), Some(b':')) {
                    self.bump();
                    Token::DoubleColon
                } else if matches!(self.peek(), Some(b'=')) {
                    self.bump();
                    Token::ColonEquals
                } else {
                    Token::Colon
                }
            }
            b'>' => {
                if matches!(self.peek(), Some(b'=')) {
                    self.bump();
                    Token::Ge
                } else {
                    Token::Greater
                }
            }
            b'<' => {
                if matches!(self.peek(), Some(b'=')) {
                    self.bump();
                    Token::Le
                } else {
                    Token::Less
                }
            }
            b'=' => {
                // Both '=' and '==' are valid for equality (leading '=' is stripped earlier)
                if matches!(self.peek(), Some(b'=')) {
                    self.bump();
                    Token::EqEq
                } else {
                    Token::EqEq
                }
            }
            b'&' => {
                if matches!(self.peek(), Some(b'&')) {
                    self.bump();
                    Token::AndAnd
                } else if matches!(self.peek(), Some(b'.')) {
                    self.bump();
                    Token::SafeNavigation
                } else {
                    return Err(Error::new("Unexpected '&'", Some(self.pos - 1)));
                }
            }
            b'|' => {
                if matches!(self.peek(), Some(b'|')) {
                    self.bump();
                    Token::OrOr
                } else {
                    return Err(Error::new("Unexpected '|'", Some(self.pos - 1)));
                }
            }
            b';' => Token::Semicolon,
            _ => return Err(Error::new("Unexpected character", Some(self.pos - 1))),
        };
        // For single-char tokens not handled above, mark last positions
        if matches!(
            tok,
            Token::Plus
                | Token::Minus
                | Token::Star
                | Token::Slash
                | Token::Percent
                | Token::Caret
                | Token::Bang
                | Token::QMark
                | Token::LParen
                | Token::RParen
                | Token::LBracket
                | Token::RBracket
                | Token::LBrace
                | Token::RBrace
                | Token::Comma
                | Token::Colon
                | Token::Greater
                | Token::Less
                | Token::Semicolon
        ) {
            self.last_start = self.pos - 1;
            self.last_end = self.pos;
        } else if matches!(
            tok,
            Token::ColonEquals
                | Token::DoubleColon
                | Token::Ge
                | Token::Le
                | Token::EqEq
                | Token::NotEq
                | Token::AndAnd
                | Token::OrOr
                | Token::SafeNavigation
        ) {
            self.last_start = self.pos - 2;
            self.last_end = self.pos;
        }
        Ok(tok)
    }

    pub fn last_start(&self) -> usize {
        self.last_start
    }
    pub fn last_end(&self) -> usize {
        self.last_end
    }
}
