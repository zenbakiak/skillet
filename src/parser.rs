use crate::ast::{BinaryOp, Expr, TypeName, UnaryOp};
use crate::error::Error;
use crate::lexer::{Lexer, Token};
use crate::limits::{nesting_error, MAX_NESTING_DEPTH};
use std::rc::Rc;

/// A parsed sub-expression and the depth of its AST (leaves are 0, a node is one more
/// than its deepest child). Tracking depth while building keeps every accepted AST within
/// `MAX_NESTING_DEPTH`, so evaluation and `Drop` of it can't exhaust the stack (BL-017).
type Parsed = Result<(Expr, usize), Error>;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    lookahead: Token,
    lookahead2: Option<Token>,
    look_pos: usize,
    /// Current recursion level of `parse_expr` / unary operands (bounds the parser's stack;
    /// parentheses add recursion without adding AST nodes).
    nesting: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let lookahead = lexer.next_token().unwrap_or(Token::Eof);
        let look_pos = lexer.last_start();
        Self {
            lexer,
            lookahead,
            lookahead2: None,
            look_pos,
            nesting: 0,
        }
    }

    fn bump(&mut self) -> Result<(), Error> {
        if let Some(next) = self.lookahead2.take() {
            self.lookahead = next;
        } else {
            self.lookahead = self.lexer.next_token()?;
        }
        self.look_pos = self.lexer.last_start();
        Ok(())
    }

    fn peek_ahead2(&mut self) -> Result<(Token, Token), Error> {
        // Use a temporary lexer clone for deeper lookahead since we need 3 tokens
        let mut temp_lexer = self.lexer.clone();
        let next1 = temp_lexer.next_token().unwrap_or(Token::Eof);
        let next2 = temp_lexer.next_token().unwrap_or(Token::Eof);
        Ok((next1, next2))
    }

    #[allow(dead_code)]
    fn expect(&mut self, tok: Token) -> Result<(), Error> {
        if self.lookahead == tok {
            self.bump()
        } else {
            Err(Error::new("Unexpected token", Some(self.look_pos)))
        }
    }

    fn err_here<T>(&self, msg: &str) -> Result<T, Error> {
        Err(Error::new(msg, Some(self.look_pos)))
    }

    /// Depth of a node whose deepest child has depth `max_child` (`None`: no children).
    fn node_depth(&self, max_child: Option<usize>) -> Result<usize, Error> {
        match max_child {
            None => Ok(0),
            Some(d) if d >= MAX_NESTING_DEPTH => Err(nesting_error(Some(self.look_pos))),
            Some(d) => Ok(d + 1),
        }
    }

    /// Run `f` one recursion level deeper, failing once `MAX_NESTING_DEPTH` is exceeded.
    fn nested(&mut self, f: impl FnOnce(&mut Self) -> Parsed) -> Parsed {
        if self.nesting >= MAX_NESTING_DEPTH {
            return Err(nesting_error(Some(self.look_pos)));
        }
        self.nesting += 1;
        let result = f(self);
        self.nesting -= 1;
        result
    }

    pub fn parse(&mut self) -> Result<Expr, Error> {
        let mut exprs = Vec::new();
        let mut max_depth = 0;

        // Parse first expression (top level: not counted as nesting)
        let (first, d) = self.parse_assignment()?;
        exprs.push(first);
        max_depth = max_depth.max(d);

        // Parse semicolon-separated expressions
        while matches!(self.lookahead, Token::Semicolon) {
            self.bump()?; // consume ';'
            if matches!(self.lookahead, Token::Eof) {
                break; // Allow trailing semicolon
            }
            let (e, d) = self.parse_assignment()?;
            exprs.push(e);
            max_depth = max_depth.max(d);
        }

        // If only one expression, return it directly; otherwise wrap in sequence
        if exprs.len() == 1 {
            exprs
                .into_iter()
                .next()
                .ok_or_else(|| Error::new("Expected expression but none found", None))
        } else {
            self.node_depth(Some(max_depth))?;
            Ok(Expr::Sequence(exprs))
        }
    }

    /// A nested expression (inside parentheses, brackets, arguments, ternary branches).
    fn parse_expr(&mut self) -> Parsed {
        self.nested(|p| p.parse_assignment())
    }

    fn parse_assignment(&mut self) -> Parsed {
        // Check for assignment pattern: :variable_name := expression
        if matches!(self.lookahead, Token::Colon) {
            // Look ahead to see if this is an assignment - use efficient 2-token lookahead
            let (next1, next2) = self.peek_ahead2()?;

            if matches!((next1, next2), (Token::Identifier(_), Token::ColonEquals)) {
                // This is an assignment
                self.bump()?; // consume ':'
                if let Token::Identifier(var_name) = &self.lookahead.clone() {
                    let var_name = var_name.clone();
                    self.bump()?; // consume identifier
                    if matches!(self.lookahead, Token::ColonEquals) {
                        self.bump()?; // consume ':='
                        let (value, d) = self.parse_ternary()?;
                        let depth = self.node_depth(Some(d))?;
                        return Ok((
                            Expr::Assignment {
                                variable: var_name,
                                value: Rc::new(value),
                            },
                            depth,
                        ));
                    }
                }
                return self.err_here("Invalid assignment syntax");
            }
        }

        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Parsed {
        let (cond, dc) = self.parse_or()?;
        if let Token::QMark = self.lookahead {
            self.bump()?; // '?'
            let (then_e, dt) = self.parse_expr()?;
            match self.lookahead {
                Token::Colon => {
                    self.bump()?;
                    let (else_e, de) = self.parse_expr()?;
                    let depth = self.node_depth(Some(dc.max(dt).max(de)))?;
                    // Ternary is encoded as a special function call handled in the runtime.
                    Ok((
                        Expr::FunctionCall {
                            name: "__TERNARY__".to_string(),
                            args: vec![cond, then_e, else_e],
                        },
                        depth,
                    ))
                }
                _ => self.err_here("Expected ':' in ternary"),
            }
        } else {
            Ok((cond, dc))
        }
    }

    /// Left-associative binary level: `next (op next)*`. Each iteration wraps the tree one
    /// level deeper, so the chain length counts against the depth limit.
    fn parse_binary_level(
        &mut self,
        next: fn(&mut Self) -> Parsed,
        op_for: fn(&Token) -> Option<BinaryOp>,
    ) -> Parsed {
        let (mut node, mut depth) = next(self)?;
        while let Some(op) = op_for(&self.lookahead) {
            self.bump()?;
            let (rhs, dr) = next(self)?;
            depth = self.node_depth(Some(depth.max(dr)))?;
            node = Expr::Binary(Rc::new(node), op, Rc::new(rhs));
        }
        Ok((node, depth))
    }

    fn parse_or(&mut self) -> Parsed {
        self.parse_binary_level(Self::parse_and, |t| match t {
            Token::Or | Token::OrOr => Some(BinaryOp::Or),
            _ => None,
        })
    }

    fn parse_and(&mut self) -> Parsed {
        self.parse_binary_level(Self::parse_equality, |t| match t {
            Token::And | Token::AndAnd => Some(BinaryOp::And),
            _ => None,
        })
    }

    fn parse_equality(&mut self) -> Parsed {
        self.parse_binary_level(Self::parse_relational, |t| match t {
            Token::EqEq => Some(BinaryOp::Eq),
            Token::NotEq => Some(BinaryOp::Ne),
            _ => None,
        })
    }

    fn parse_relational(&mut self) -> Parsed {
        self.parse_binary_level(Self::parse_additive, |t| match t {
            Token::Greater => Some(BinaryOp::Gt),
            Token::Less => Some(BinaryOp::Lt),
            Token::Ge => Some(BinaryOp::Ge),
            Token::Le => Some(BinaryOp::Le),
            _ => None,
        })
    }

    fn parse_additive(&mut self) -> Parsed {
        self.parse_binary_level(Self::parse_multiplicative, |t| match t {
            Token::Plus => Some(BinaryOp::Add),
            Token::Minus => Some(BinaryOp::Sub),
            _ => None,
        })
    }

    fn parse_multiplicative(&mut self) -> Parsed {
        self.parse_binary_level(Self::parse_unary, |t| match t {
            Token::Star => Some(BinaryOp::Mul),
            Token::Slash => Some(BinaryOp::Div),
            Token::Percent => Some(BinaryOp::Mod),
            _ => None,
        })
    }

    fn parse_power(&mut self) -> Parsed {
        // Right associative with higher precedence than unary
        let (left, dl) = self.parse_cast()?;
        if let Token::Caret = self.lookahead {
            self.bump()?;
            // exponent can be unary like -2
            let (right, dr) = self.nested(|p| p.parse_unary())?;
            let depth = self.node_depth(Some(dl.max(dr)))?;
            Ok((
                Expr::Binary(Rc::new(left), BinaryOp::Pow, Rc::new(right)),
                depth,
            ))
        } else {
            Ok((left, dl))
        }
    }

    fn parse_unary(&mut self) -> Parsed {
        let op = match self.lookahead {
            Token::Plus => UnaryOp::Plus,
            Token::Minus => UnaryOp::Minus,
            Token::Bang => UnaryOp::Not,
            _ => return self.parse_power(),
        };
        self.bump()?;
        let (expr, d) = self.nested(|p| p.parse_unary())?;
        let depth = self.node_depth(Some(d))?;
        Ok((Expr::Unary(op, Rc::new(expr)), depth))
    }

    /// Comma-separated arguments after an already consumed `(`, through the closing `)`.
    /// Returns the arguments and the depth of the deepest one.
    fn parse_call_args(&mut self, err: &str) -> Result<(Vec<Expr>, Option<usize>), Error> {
        let mut args = Vec::new();
        let mut max_child = None;
        if !matches!(self.lookahead, Token::RParen) {
            loop {
                let (arg, d) = if let Token::Ellipsis = self.lookahead {
                    self.bump()?;
                    let (inner, d) = self.parse_expr()?;
                    (Expr::Spread(Rc::new(inner)), self.node_depth(Some(d))?)
                } else {
                    self.parse_expr()?
                };
                args.push(arg);
                max_child = max_child.max(Some(d));
                match self.lookahead {
                    Token::Comma => {
                        self.bump()?;
                    }
                    Token::RParen => break,
                    _ => return self.err_here(err),
                }
            }
        }
        self.bump()?; // consume ')'
        Ok((args, max_child))
    }

    fn parse_atom(&mut self) -> Parsed {
        match self.lookahead.clone() {
            Token::Number(n) => {
                self.bump()?;
                Ok((Expr::Number(n), 0))
            }
            Token::LParen => {
                self.bump()?;
                let parsed = self.parse_expr()?;
                match self.lookahead {
                    Token::RParen => {
                        self.bump()?;
                        Ok(parsed)
                    }
                    _ => self.err_here("Expected ')'"),
                }
            }
            Token::String(s) => {
                self.bump()?;
                Ok((Expr::StringLit(s), 0))
            }
            Token::Null => {
                self.bump()?;
                Ok((Expr::Null, 0))
            }
            Token::Colon => {
                // Variable: ':' identifier (let postfix handle property chains and method calls)
                self.bump()?; // consume ':'
                match self.lookahead.clone() {
                    Token::Identifier(name) => {
                        self.bump()?;
                        Ok((Expr::Variable(name), 0))
                    }
                    _ => self.err_here("Expected variable name after ':'"),
                }
            }
            Token::True => {
                self.bump()?;
                Ok((
                    Expr::FunctionCall {
                        name: "__CONST_TRUE__".to_string(),
                        args: vec![],
                    },
                    0,
                ))
            }
            Token::False => {
                self.bump()?;
                Ok((
                    Expr::FunctionCall {
                        name: "__CONST_FALSE__".to_string(),
                        args: vec![],
                    },
                    0,
                ))
            }
            Token::Identifier(name) => {
                // Function call: IDENT '(' args? ')'
                self.bump()?; // consume ident
                match self.lookahead {
                    Token::LParen => {
                        self.bump()?; // '('
                        let (args, max_child) =
                            self.parse_call_args("Expected ',' or ')' in argument list")?;
                        let depth = self.node_depth(max_child)?;
                        Ok((
                            Expr::FunctionCall {
                                name: name.to_uppercase(),
                                args,
                            },
                            depth,
                        ))
                    }
                    _ => self.err_here("Unexpected identifier (expected function call)"),
                }
            }
            Token::LBracket => self.parse_array_literal(),
            Token::LBrace => self.parse_object_literal(),
            other => Err(Error::new(
                format!("Unexpected token: {:?}", other),
                Some(self.look_pos),
            )),
        }
    }

    /// Array literal: `[ expr (, expr)* ]`
    fn parse_array_literal(&mut self) -> Parsed {
        self.bump()?; // consume '['
        let mut items = Vec::new();
        let mut max_child = None;
        if !matches!(self.lookahead, Token::RBracket) {
            loop {
                let (item, d) = self.parse_expr()?;
                items.push(item);
                max_child = max_child.max(Some(d));
                match self.lookahead {
                    Token::Comma => {
                        self.bump()?;
                    }
                    Token::RBracket => break,
                    _ => return self.err_here("Expected ',' or ']' in array"),
                }
            }
        }
        self.bump()?; // consume ']'
        let depth = self.node_depth(max_child)?;
        Ok((Expr::Array(items), depth))
    }

    /// Object literal: `{ key: value, ... }`
    fn parse_object_literal(&mut self) -> Parsed {
        self.bump()?; // consume '{'
        let mut pairs = Vec::new();
        let mut max_child = None;
        if !matches!(self.lookahead, Token::RBrace) {
            loop {
                // Parse key (can be identifier or string)
                let key = match self.lookahead.clone() {
                    Token::Identifier(s) | Token::String(s) => {
                        self.bump()?;
                        s
                    }
                    _ => {
                        return self.err_here("Expected identifier or string key in object literal")
                    }
                };

                // Expect colon
                if matches!(self.lookahead, Token::Colon) {
                    self.bump()?; // consume ':'
                } else {
                    return self.err_here("Expected ':' after object key");
                }

                // Parse value
                let (value, d) = self.parse_expr()?;
                pairs.push((key, value));
                max_child = max_child.max(Some(d));

                match self.lookahead {
                    Token::Comma => {
                        self.bump()?;
                        // Allow trailing comma
                        if matches!(self.lookahead, Token::RBrace) {
                            break;
                        }
                    }
                    Token::RBrace => break,
                    _ => return self.err_here("Expected ',' or '}' in object literal"),
                }
            }
        }
        self.bump()?; // consume '}'
        let depth = self.node_depth(max_child)?;
        Ok((Expr::ObjectLiteral(pairs), depth))
    }

    fn parse_cast(&mut self) -> Parsed {
        let (node, d) = self.parse_postfix()?;
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
            let depth = self.node_depth(Some(d))?;
            return Ok((
                Expr::TypeCast {
                    expr: Rc::new(node),
                    ty: tname,
                },
                depth,
            ));
        }
        Ok((node, d))
    }

    /// Postfix chain: `.name`, `.name?`, `.name(args)`, `&.name`, `&.name(args)`, `[i]`,
    /// `[a:b]`. Each link wraps the tree one level deeper and counts against the limit.
    fn parse_postfix(&mut self) -> Parsed {
        let (mut node, mut depth) = self.parse_atom()?;
        loop {
            let (next, max_child) = match self.lookahead {
                Token::Dot => self.parse_dot_suffix(node, depth)?,
                Token::SafeNavigation => self.parse_safe_nav_suffix(node, depth)?,
                Token::LBracket => self.parse_index_suffix(node, depth)?,
                _ => break,
            };
            depth = self.node_depth(Some(max_child))?;
            node = next;
        }
        Ok((node, depth))
    }

    /// The name after `.` or `&.`.
    fn parse_member_name(&mut self, err: &str) -> Result<String, Error> {
        match self.lookahead.clone() {
            Token::Identifier(s) => {
                self.bump()?;
                Ok(s)
            }
            _ => self.err_here(err),
        }
    }

    /// `.name`, `.name?` or `.name(args)`; returns the new node and its deepest child depth.
    fn parse_dot_suffix(&mut self, target: Expr, dt: usize) -> Result<(Expr, usize), Error> {
        self.bump()?; // '.'
        let name = self.parse_member_name("Expected property or method name after '.'")?;
        // Predicate style: name?
        if let Token::QMark = self.lookahead {
            self.bump()?; // consume '?'
            let node = Expr::MethodCall {
                target: Rc::new(target),
                name: name.to_lowercase(),
                args: vec![],
                predicate: true,
            };
            return Ok((node, dt));
        }
        // Check for method call: '(' args ')'
        if let Token::LParen = self.lookahead {
            self.bump()?; // '('
            let (args, max_child) = self.parse_call_args("Expected ',' or ')' in method args")?;
            let node = Expr::MethodCall {
                target: Rc::new(target),
                name: name.to_lowercase(),
                args,
                predicate: false,
            };
            return Ok((node, dt.max(max_child.unwrap_or(0))));
        }
        // No parentheses, treat as property access
        let node = Expr::PropertyAccess {
            target: Rc::new(target),
            property: name,
        };
        Ok((node, dt))
    }

    /// `&.name` or `&.name(args)`; returns the new node and its deepest child depth.
    fn parse_safe_nav_suffix(&mut self, target: Expr, dt: usize) -> Result<(Expr, usize), Error> {
        self.bump()?; // '&.'
        let name = self.parse_member_name("Expected property name after '&.'")?;
        // Check for method call after safe navigation
        if let Token::LParen = self.lookahead {
            self.bump()?; // '('
            let (args, max_child) =
                self.parse_call_args("Expected ',' or ')' in safe method args")?;
            let node = Expr::SafeMethodCall {
                target: Rc::new(target),
                name: name.to_lowercase(),
                args,
            };
            return Ok((node, dt.max(max_child.unwrap_or(0))));
        }
        let node = Expr::SafePropertyAccess {
            target: Rc::new(target),
            property: name,
        };
        Ok((node, dt))
    }

    /// `[expr]`, `[start:end]`, `[:end]` or `[start:]`; returns the new node and its deepest
    /// child depth.
    fn parse_index_suffix(&mut self, target: Expr, dt: usize) -> Result<(Expr, usize), Error> {
        self.bump()?; // '['
        let mut max_child = dt;
        let mut start: Option<Expr> = None;
        let mut end: Option<Expr> = None;
        match self.lookahead {
            Token::RBracket => {
                self.bump()?;
                return self.err_here("Empty index '[]' not allowed");
            }
            Token::Colon => { /* [:end] */ }
            _ => {
                // parse first expr
                let (first, d) = self.parse_expr()?;
                max_child = max_child.max(d);
                match self.lookahead {
                    Token::Colon => {
                        start = Some(first);
                    }
                    Token::RBracket => {
                        self.bump()?; // ']'
                        let node = Expr::Index {
                            target: Rc::new(target),
                            index: Rc::new(first),
                        };
                        return Ok((node, max_child));
                    }
                    _ => return self.err_here("Expected ':' or ']' in indexing"),
                }
            }
        }
        // At this point, we saw ':' (slice)
        if let Token::Colon = self.lookahead {
            self.bump()?;
        }
        // Optional end expression
        if !matches!(self.lookahead, Token::RBracket) {
            let (e, d) = self.parse_expr()?;
            max_child = max_child.max(d);
            end = Some(e);
        }
        match self.lookahead {
            Token::RBracket => {
                self.bump()?;
            }
            _ => return self.err_here("Expected ']' to close slice"),
        }
        let node = Expr::Slice {
            target: Rc::new(target),
            start: start.map(Rc::new),
            end: end.map(Rc::new),
        };
        Ok((node, max_child))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Expr, Error> {
        Parser::new(s).parse()
    }

    fn assert_too_deep(s: &str) {
        let err = parse(s).expect_err("expected nesting error");
        assert!(
            err.message.contains("nested too deeply"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn left_deep_chain_is_bounded_at_parse_time() {
        let at_limit = format!("{}1", "1+".repeat(MAX_NESTING_DEPTH));
        assert!(parse(&at_limit).is_ok());
        assert_too_deep(&format!("{}1", "1+".repeat(MAX_NESTING_DEPTH + 1)));
        assert_too_deep(&format!("{}1", "1+".repeat(100_000)));
    }

    #[test]
    fn postfix_chain_is_bounded_at_parse_time() {
        assert!(parse(&format!(":a{}", ".b".repeat(MAX_NESTING_DEPTH))).is_ok());
        assert_too_deep(&format!(":a{}", ".b".repeat(MAX_NESTING_DEPTH + 1)));
        assert_too_deep(&format!("[1]{}", "[0]".repeat(MAX_NESTING_DEPTH)));
    }

    #[test]
    fn nested_chains_add_up() {
        // A parenthesised chain used as the left operand of another chain: depths add.
        let tail = "+1".repeat(64);
        let two = format!("((1{tail}){tail})");
        assert!(parse(&two).is_ok());
        assert_too_deep(&format!("({two}{tail})"));
    }

    /// Debug parser frames are large (~20 KB per paren level), so this runs on a thread
    /// with the same 16 MB stack the server workers use rather than the 2 MB test default.
    #[test]
    fn paren_recursion_is_bounded() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let n = MAX_NESTING_DEPTH;
                assert!(parse(&format!("{}1{}", "(".repeat(n), ")".repeat(n))).is_ok());
                let n = MAX_NESTING_DEPTH + 1;
                assert_too_deep(&format!("{}1{}", "(".repeat(n), ")".repeat(n)));
                assert_too_deep(&format!("{}1", "(".repeat(100_000)));
            })
            .expect("spawn")
            .join()
            .expect("paren test thread panicked");
    }

    #[test]
    fn error_message_and_position() {
        let err = parse(&format!("{}1", "-".repeat(200))).expect_err("too deep");
        assert_eq!(err.message, "Expression nested too deeply (max 128)");
        assert!(err.position.is_some());
    }
}
