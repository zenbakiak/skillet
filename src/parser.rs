use crate::ast::{BinaryOp, Expr, TypeName, UnaryOp};
use crate::error::Error;
use crate::lexer::{Lexer, Token};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    lookahead: Token,
    look_pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let lookahead = lexer.next_token().unwrap_or(Token::Eof);
        let look_pos = lexer.last_start();
        Self { lexer, lookahead, look_pos }
    }

    fn bump(&mut self) -> Result<(), Error> {
        self.lookahead = self.lexer.next_token()?;
        self.look_pos = self.lexer.last_start();
        Ok(())
    }

    #[allow(dead_code)]
    fn expect(&mut self, tok: Token) -> Result<(), Error> {
        if self.lookahead == tok {
            self.bump()
        } else {
            Err(Error::new("Unexpected token", Some(self.look_pos)))
        }
    }

    fn err_here<T>(&self, msg: &str) -> Result<T, Error> { Err(Error::new(msg, Some(self.look_pos))) }

    pub fn parse(&mut self) -> Result<Expr, Error> {
        let expr = self.parse_expr()?;
        // allow trailing whitespace and EOF
        Ok(expr)
    }

    fn parse_expr(&mut self) -> Result<Expr, Error> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<Expr, Error> {
        let cond = self.parse_or()?;
        if let Token::QMark = self.lookahead {
            self.bump()?; // '?'
            let then_e = self.parse_expr()?;
            match self.lookahead {
                Token::Colon => {
                    self.bump()?;
                    let else_e = self.parse_expr()?;
                    // Represent ternary as IF function for now: IF(cond, then, else) but we don't have IF.
                    // We'll encode as Binary with Or? Instead, we can't, so we return a function call node.
                    // Use a special function name handled in runtime: __TERNARY__.
                    Ok(Expr::FunctionCall { name: "__TERNARY__".to_string(), args: vec![cond, then_e, else_e] })
                }
                _ => self.err_here("Expected ':' in ternary"),
            }
        } else {
            Ok(cond)
        }
    }

    fn parse_or(&mut self) -> Result<Expr, Error> {
        let mut node = self.parse_and()?;
        loop {
            match self.lookahead {
                Token::Or | Token::OrOr => {
                    self.bump()?;
                    let rhs = self.parse_and()?;
                    node = Expr::Binary(Box::new(node), BinaryOp::Or, Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_and(&mut self) -> Result<Expr, Error> {
        let mut node = self.parse_equality()?;
        loop {
            match self.lookahead {
                Token::And | Token::AndAnd => {
                    self.bump()?;
                    let rhs = self.parse_equality()?;
                    node = Expr::Binary(Box::new(node), BinaryOp::And, Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_equality(&mut self) -> Result<Expr, Error> {
        let mut node = self.parse_relational()?;
        loop {
            match self.lookahead {
                Token::EqEq => { self.bump()?; let rhs = self.parse_relational()?; node = Expr::Binary(Box::new(node), BinaryOp::Eq, Box::new(rhs)); }
                Token::NotEq => { self.bump()?; let rhs = self.parse_relational()?; node = Expr::Binary(Box::new(node), BinaryOp::Ne, Box::new(rhs)); }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_relational(&mut self) -> Result<Expr, Error> {
        let mut node = self.parse_additive()?;
        loop {
            match self.lookahead {
                Token::Greater => { self.bump()?; let rhs = self.parse_additive()?; node = Expr::Binary(Box::new(node), BinaryOp::Gt, Box::new(rhs)); }
                Token::Less => { self.bump()?; let rhs = self.parse_additive()?; node = Expr::Binary(Box::new(node), BinaryOp::Lt, Box::new(rhs)); }
                Token::Ge => { self.bump()?; let rhs = self.parse_additive()?; node = Expr::Binary(Box::new(node), BinaryOp::Ge, Box::new(rhs)); }
                Token::Le => { self.bump()?; let rhs = self.parse_additive()?; node = Expr::Binary(Box::new(node), BinaryOp::Le, Box::new(rhs)); }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_additive(&mut self) -> Result<Expr, Error> {
        let mut node = self.parse_multiplicative()?;
        loop {
            match self.lookahead {
                Token::Plus => {
                    self.bump()?;
                    let rhs = self.parse_multiplicative()?;
                    node = Expr::Binary(Box::new(node), BinaryOp::Add, Box::new(rhs));
                }
                Token::Minus => {
                    self.bump()?;
                    let rhs = self.parse_multiplicative()?;
                    node = Expr::Binary(Box::new(node), BinaryOp::Sub, Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, Error> {
        let mut node = self.parse_unary()?;
        loop {
            match self.lookahead {
                Token::Star => {
                    self.bump()?;
                    let rhs = self.parse_unary()?;
                    node = Expr::Binary(Box::new(node), BinaryOp::Mul, Box::new(rhs));
                }
                Token::Slash => {
                    self.bump()?;
                    let rhs = self.parse_unary()?;
                    node = Expr::Binary(Box::new(node), BinaryOp::Div, Box::new(rhs));
                }
                Token::Percent => {
                    self.bump()?;
                    let rhs = self.parse_unary()?;
                    node = Expr::Binary(Box::new(node), BinaryOp::Mod, Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_power(&mut self) -> Result<Expr, Error> {
        // Right associative with higher precedence than unary
        let left = self.parse_cast()?;
        if let Token::Caret = self.lookahead {
            self.bump()?;
            let right = self.parse_unary()?; // exponent can be unary like -2
            Ok(Expr::Binary(Box::new(left), BinaryOp::Pow, Box::new(right)))
        } else {
            Ok(left)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, Error> {
        match self.lookahead {
            Token::Plus => {
                self.bump()?;
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Plus, Box::new(expr)))
            }
            Token::Minus => {
                self.bump()?;
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Minus, Box::new(expr)))
            }
            Token::Bang => {
                self.bump()?;
                let expr = self.parse_unary()?;
                Ok(Expr::Unary(UnaryOp::Not, Box::new(expr)))
            }
            _ => self.parse_power(),
        }
    }

    fn parse_atom(&mut self) -> Result<Expr, Error> {
        match self.lookahead.clone() {
            Token::Number(n) => {
                self.bump()?;
                Ok(Expr::Number(n))
            }
            Token::LParen => {
                self.bump()?;
                let expr = self.parse_expr()?;
                match self.lookahead {
                    Token::RParen => {
                        self.bump()?;
                        Ok(expr)
                    }
                    _ => self.err_here("Expected ')'"),
                }
            }
            Token::String(s) => { self.bump()?; Ok(Expr::StringLit(s)) }
            Token::Null => { self.bump()?; Ok(Expr::Null) }
            Token::Colon => {
                // Variable: ':' identifier
                self.bump()?; // consume ':'
                match self.lookahead.clone() {
                    Token::Identifier(name) => {
                        self.bump()?;
                        Ok(Expr::Variable(name))
                    }
                    _ => self.err_here("Expected variable name after ':'"),
                }
            }
            Token::True => { self.bump()?; Ok(Expr::FunctionCall { name: "__CONST_TRUE__".to_string(), args: vec![] }) }
            Token::False => { self.bump()?; Ok(Expr::FunctionCall { name: "__CONST_FALSE__".to_string(), args: vec![] }) }
            Token::Identifier(name) => {
                // Function call: IDENT '(' args? ')'
                let func_name = name;
                self.bump()?; // consume ident
                match self.lookahead {
                    Token::LParen => {
                        self.bump()?; // '('
                        let mut args = Vec::new();
                        if let Token::RParen = self.lookahead {
                            // empty args
                        } else {
                            loop {
                                let arg = if let Token::Ellipsis = self.lookahead { self.bump()?; Expr::Spread(Box::new(self.parse_expr()?)) } else { self.parse_expr()? };
                                args.push(arg);
                                match self.lookahead {
                                    Token::Comma => { self.bump()?; }
                                    Token::RParen => break,
                                    _ => return self.err_here("Expected ',' or ')' in argument list"),
                                }
                            }
                        }
                        self.bump()?; // consume ')'
                        let final_name = func_name.to_uppercase();
                        Ok(Expr::FunctionCall { name: final_name, args })
                    }
                    _ => self.err_here("Unexpected identifier (expected function call)"),
                }
            }
            Token::LBracket => {
                // Array literal: [ expr (, expr)* ]
                self.bump()?; // consume '['
                let mut items = Vec::new();
                if let Token::RBracket = self.lookahead {
                    // empty
                } else {
                    loop {
                        let item = self.parse_expr()?;
                        items.push(item);
                        match self.lookahead {
                            Token::Comma => { self.bump()?; }
                            Token::RBracket => break,
                            _ => return self.err_here("Expected ',' or ']' in array"),
                        }
                    }
                }
                self.bump()?; // consume ']'
                Ok(Expr::Array(items))
            }
            other => Err(Error::new(format!("Unexpected token: {:?}", other), Some(self.look_pos))),
        }
    }

    fn parse_cast(&mut self) -> Result<Expr, Error> {
        let mut node = self.parse_postfix()?;
        if let Token::DoubleColon = self.lookahead {
            self.bump()?; // '::'
            let tname = match self.lookahead.clone() {
                Token::Identifier(s) => {
                    self.bump()?;
                    match s.to_lowercase().as_str() {
                        "integer" | "int" => TypeName::Integer,
                        "float" | "number" => TypeName::Float,
                        "string" => TypeName::String,
                        "boolean" | "bool" => TypeName::Boolean,
                        "array" => TypeName::Array,
                        "currency" => TypeName::Currency,
                        "datetime" | "date" => TypeName::DateTime,
                        "json" => TypeName::Json,
                        _ => return Err(Error::new("Unknown cast type", None)),
                    }
                }
                _ => return Err(Error::new("Expected type name after '::'", None)),
            };
            node = Expr::TypeCast { expr: Box::new(node), ty: tname };
        }
        Ok(node)
    }

    fn parse_postfix(&mut self) -> Result<Expr, Error> {
        let mut node = self.parse_atom()?;
        loop {
            match self.lookahead {
                Token::Dot => {
                    self.bump()?; // '.'
                    let name = match self.lookahead.clone() {
                        Token::Identifier(s) => { self.bump()?; s }
                        _ => return self.err_here("Expected method name after '.'"),
                    };
                    // Predicate style: name?
                    if let Token::QMark = self.lookahead {
                        self.bump()?; // consume '?'
                        node = Expr::MethodCall { target: Box::new(node), name: name.to_lowercase(), args: vec![], predicate: true };
                        continue;
                    }
                    // Otherwise expect '(' args ')' for methods
                    match self.lookahead {
                        Token::LParen => {
                            self.bump()?; // '('
                            let mut args = Vec::new();
                            if let Token::RParen = self.lookahead {
                                // empty
                            } else {
                                loop {
                                    let arg = if let Token::Ellipsis = self.lookahead { self.bump()?; Expr::Spread(Box::new(self.parse_expr()?)) } else { self.parse_expr()? };
                                    args.push(arg);
                                    match self.lookahead {
                                        Token::Comma => { self.bump()?; }
                                        Token::RParen => break,
                                        _ => return self.err_here("Expected ',' or ')' in method args"),
                                    }
                            }
                        }
                        self.bump()?; // ')'
                        node = Expr::MethodCall { target: Box::new(node), name: name.to_lowercase(), args, predicate: false };
                    }
                    _ => return self.err_here("Expected '(' or '?' after method name"),
                }
            }
            Token::LBracket => {
                // Indexing or slicing
                self.bump()?; // '['
                // Cases: [expr], [start:end], [:end], [start:]
                let mut start: Option<Expr> = None;
                let mut end: Option<Expr> = None;
                match self.lookahead {
                    Token::RBracket => { self.bump()?; return self.err_here("Empty index '[]' not allowed"); }
                    Token::Colon => { /* [:end] */ }
                    _ => {
                        // parse first expr
                        let first = self.parse_expr()?;
                        match self.lookahead {
                            Token::Colon => { start = Some(first); }
                            Token::RBracket => {
                                self.bump()?; // ']'
                                node = Expr::Index { target: Box::new(node), index: Box::new(first) };
                                continue;
                            }
                            _ => return self.err_here("Expected ':' or ']' in indexing"),
                        }
                    }
                }
                // At this point, we saw ':' (slice)
                if let Token::Colon = self.lookahead { self.bump()?; }
                // Optional end expression
                if let Token::RBracket = self.lookahead {
                    // [start:]
                } else {
                    end = Some(self.parse_expr()?);
                }
                match self.lookahead {
                    Token::RBracket => { self.bump()?; }
                    _ => return self.err_here("Expected ']' to close slice"),
                }
                node = Expr::Slice { target: Box::new(node), start: start.map(Box::new), end: end.map(Box::new) };
            }
            _ => break,
        }
    }
    Ok(node)
}
}
