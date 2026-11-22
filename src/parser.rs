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
    /// A lexer error on the first token, reported by `parse` (`new` cannot fail).
    first_error: Option<Error>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let (lookahead, first_error) = match lexer.next_token() {
            Ok(tok) => (tok, None),
            Err(e) => (Token::Eof, Some(e)),
        };
        let look_pos = lexer.last_start();
        Self {
            lexer,
            lookahead,
            lookahead2: None,
            look_pos,
            nesting: 0,
            first_error,
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

    /// Go one recursion level deeper, failing once `MAX_NESTING_DEPTH` is exceeded. The
    /// caller decrements `nesting` when it returns.
    fn enter(&mut self) -> Result<(), Error> {
        if self.nesting >= MAX_NESTING_DEPTH {
            return Err(nesting_error(Some(self.look_pos)));
        }
        self.nesting += 1;
        Ok(())
    }

    pub fn parse(&mut self) -> Result<Expr, Error> {
        if let Some(e) = self.first_error.take() {
            return Err(e);
        }
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

        // Everything must be consumed: `1 2` is an error, not `1` (BL-069).
        if !matches!(self.lookahead, Token::Eof) {
            return self.err_here("Unexpected token after end of expression");
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

    // Stack use (BL-029): every nesting level of the input runs the functions on the path
    // parse_expr -> parse_assignment -> parse_ternary -> parse_binary -> parse_unary ->
    // parse_postfix -> parse_atom -> (parse_paren | literal | call) -> parse_expr. Keep those
    // frames small: the branches a level does not always take live in `#[inline(never)]`
    // helpers, so their locals are not reserved on every level (debug builds give every
    // local its own stack slot).

    /// A nested expression (inside parentheses, brackets, arguments, ternary branches).
    fn parse_expr(&mut self) -> Parsed {
        self.enter()?;
        let result = self.parse_assignment();
        self.nesting -= 1;
        result
    }

    fn parse_assignment(&mut self) -> Parsed {
        // Check for assignment pattern: :variable_name := expression
        if matches!(self.lookahead, Token::Colon) && self.at_assignment()? {
            return self.parse_assignment_tail();
        }
        self.parse_ternary()
    }

    /// True when the tokens after the current `:` are `identifier :=`.
    #[inline(never)]
    fn at_assignment(&mut self) -> Result<bool, Error> {
        // Look ahead to see if this is an assignment - use efficient 2-token lookahead
        let (next1, next2) = self.peek_ahead2()?;
        Ok(matches!(
            (next1, next2),
            (Token::Identifier(_), Token::ColonEquals)
        ))
    }

    /// `:name := value`, starting at the `:`.
    #[inline(never)]
    fn parse_assignment_tail(&mut self) -> Parsed {
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
        self.err_here("Invalid assignment syntax")
    }

    fn parse_ternary(&mut self) -> Parsed {
        let cond = self.parse_binary(0)?;
        if let Token::QMark = self.lookahead {
            return self.parse_ternary_tail(cond);
        }
        Ok(cond)
    }

    /// `? then : else` after an already parsed condition.
    #[inline(never)]
    fn parse_ternary_tail(&mut self, (cond, dc): (Expr, usize)) -> Parsed {
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
    }

    /// Left-associative binary operators by precedence climbing, loosest first:
    /// `or` (1), `and` (2), prefix `NOT` (see `parse_not_keyword`), `== !=` (3),
    /// `> < >= <=` (4), `&` (5), `+ -` (6), `* / %` (7).
    /// Operands are `unary`, then postfix `%` and `^` (see `parse_power_rest`), so the full
    /// Excel order is: negation > `%` > `^` > `* /` > `+ -` > `&` > comparisons (D-10).
    /// Builds the same tree, in the same order, as one function per level would, but
    /// with one frame instead of two per level on the path to an operand. Each operator
    /// wraps the tree one level deeper, so chain length counts against the depth limit.
    fn parse_binary(&mut self, min_prec: u8) -> Parsed {
        // `NOT` starts an operand of `AND`/`OR` (the right one of `AND` is parsed at
        // `AND_PREC + 1`), never an operand of a comparison or arithmetic.
        let (mut node, mut depth) = if min_prec <= AND_PREC + 1 && self.at_not_keyword() {
            self.parse_not_keyword()?
        } else {
            let mut parsed = self.parse_unary()?;
            if matches!(self.lookahead, Token::Percent | Token::Caret) {
                parsed = self.parse_power_rest(parsed)?;
            }
            parsed
        };
        while let Some((op, prec)) = binary_op(&self.lookahead) {
            if prec < min_prec {
                break;
            }
            self.bump()?;
            let (rhs, dr) = self.parse_binary(prec + 1)?;
            depth = self.node_depth(Some(depth.max(dr)))?;
            node = match op {
                Some(op) => Expr::Binary(Rc::new(node), op, Rc::new(rhs)),
                None => Expr::FunctionCall {
                    name: CONCAT_OP.to_string(),
                    args: vec![node, rhs],
                },
            };
        }
        Ok((node, depth))
    }

    /// Prefix `-`, `+` and `!` bind tightest (Excel: `-2^2` is 4).
    fn parse_unary(&mut self) -> Parsed {
        let op = match self.lookahead {
            Token::Plus => UnaryOp::Plus,
            Token::Minus => UnaryOp::Minus,
            Token::Bang => UnaryOp::Not,
            _ => return self.parse_postfix(),
        };
        self.bump()?;
        self.enter()?;
        let operand = self.parse_unary();
        self.nesting -= 1;
        let (expr, d) = operand?;
        let depth = self.node_depth(Some(d))?;
        Ok((Expr::Unary(op, Rc::new(expr)), depth))
    }

    /// Is the lookahead the `NOT` keyword: a `NOT` name (any case) not followed directly by `(`
    /// (`NOT(x)` with no space is the builtin call)?
    fn at_not_keyword(&self) -> bool {
        matches!(&self.lookahead, Token::Identifier(name) if name.eq_ignore_ascii_case("NOT"))
            && !self.call_follows()
    }

    /// With a name as the lookahead: does `(` follow it directly, with no space?
    #[inline(never)]
    fn call_follows(&self) -> bool {
        let end = self.lexer.last_end();
        let mut next = self.lexer.clone();
        matches!(next.next_token(), Ok(Token::LParen)) && next.last_start() == end
    }

    /// `NOT operand` (ADR-0029): the `NOT` builtin applied to the operand, so it gives the same
    /// result as `NOT(operand)`. As in VB and SQL it binds looser than the comparisons and
    /// tighter than `AND`: the operand is everything up to the next `AND`/`OR` (or `&&`/`||`),
    /// so `NOT :x > 1` is `NOT (:x > 1)`. It starts an operand only where an `AND` operand
    /// can start; elsewhere (`1 + NOT x`) it needs parentheses.
    #[inline(never)]
    fn parse_not_keyword(&mut self) -> Parsed {
        self.bump()?; // 'NOT'
        self.enter()?;
        let operand = self.parse_binary(AND_PREC + 1);
        self.nesting -= 1;
        let (expr, d) = operand?;
        let depth = self.node_depth(Some(d))?;
        Ok((
            Expr::FunctionCall {
                name: "NOT".to_string(),
                args: vec![expr],
            },
            depth,
        ))
    }

    /// `atom link* cast*`: the postfix chain, then zero or more casts (ADR-0031 A1).
    fn parse_postfix(&mut self) -> Parsed {
        let atom = self.parse_atom()?;
        match self.lookahead {
            Token::Dot | Token::SafeNavigation | Token::LBracket | Token::DoubleColon => {
                self.parse_postfix_rest(atom)
            }
            _ => Ok(atom),
        }
    }

    /// The optional parts of `parse_postfix` after the atom.
    #[inline(never)]
    fn parse_postfix_rest(&mut self, mut parsed: (Expr, usize)) -> Parsed {
        if matches!(
            self.lookahead,
            Token::Dot | Token::SafeNavigation | Token::LBracket
        ) {
            parsed = self.parse_postfix_chain(parsed)?;
        }
        while let Token::DoubleColon = self.lookahead {
            parsed = self.parse_cast_tail(parsed)?;
        }
        Ok(parsed)
    }

    /// After a unary operand: postfix percent (`50%` is `50 / 100`), then a left-associative
    /// `^` chain whose operands are unary operands with their own percent suffix
    /// (`2^3^2` is 64, `2^50%` is `2^0.5`).
    #[inline(never)]
    fn parse_power_rest(&mut self, parsed: (Expr, usize)) -> Parsed {
        let (mut node, mut depth) = self.parse_percent_suffix(parsed)?;
        while let Token::Caret = self.lookahead {
            self.bump()?;
            self.enter()?;
            let exponent = self.parse_unary();
            self.nesting -= 1;
            let (right, dr) = self.parse_percent_suffix(exponent?)?;
            depth = self.node_depth(Some(depth.max(dr)))?;
            node = Expr::Binary(Rc::new(node), BinaryOp::Pow, Rc::new(right));
        }
        Ok((node, depth))
    }

    /// Zero or more postfix `%` after an operand. A `%` directly followed by something that
    /// starts an operand stays the binary modulo operator (`10 % 3`, `:a % :b`, `10 % (3)`);
    /// before anything else, including `+` and `-`, it is the percent suffix (`10%-3` is -2.9,
    /// BL-066).
    fn parse_percent_suffix(&mut self, (mut node, mut depth): (Expr, usize)) -> Parsed {
        while matches!(self.lookahead, Token::Percent) && !self.percent_is_modulo() {
            self.bump()?;
            depth = self.node_depth(Some(depth))?;
            node = Expr::Binary(Rc::new(node), BinaryOp::Div, Rc::new(Expr::Number(100.0)));
        }
        Ok((node, depth))
    }

    /// With `%` as the lookahead: is the next token the start of a right operand? A `:` only
    /// counts when a variable name follows it directly (`:x`), so `c ? 10% : 2` is a percent.
    fn percent_is_modulo(&self) -> bool {
        let mut next = self.lexer.clone();
        match next.next_token() {
            Ok(Token::Colon) => {
                let colon_end = next.last_end();
                matches!(next.next_token(), Ok(Token::Identifier(_)))
                    && next.last_start() == colon_end
            }
            Ok(tok) => matches!(
                tok,
                Token::Number(_)
                    | Token::String(_)
                    | Token::Identifier(_)
                    | Token::True
                    | Token::False
                    | Token::Null
                    | Token::LParen
                    | Token::LBracket
                    | Token::LBrace
                    | Token::Bang
            ),
            Err(_) => false,
        }
    }

    /// Comma-separated arguments after an already consumed `(`, through the closing `)`.
    /// Returns the arguments and the depth of the deepest one.
    fn parse_call_args(&mut self, err: &str) -> Result<(Vec<Expr>, Option<usize>), Error> {
        let mut args = Vec::new();
        let mut max_child = None;
        if !matches!(self.lookahead, Token::RParen) {
            loop {
                let (arg, d) = if let Token::Ellipsis = self.lookahead {
                    self.parse_spread()?
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

    /// `...expr` in an argument list.
    #[inline(never)]
    fn parse_spread(&mut self) -> Parsed {
        self.bump()?;
        let (inner, d) = self.parse_expr()?;
        Ok((Expr::Spread(Rc::new(inner)), self.node_depth(Some(d))?))
    }

    fn parse_atom(&mut self) -> Parsed {
        match self.lookahead {
            Token::LParen => self.parse_paren(),
            Token::LBracket => self.parse_array_literal(),
            Token::LBrace => self.parse_object_literal(),
            Token::Identifier(_) => self.parse_function_call(),
            _ => self.parse_leaf(),
        }
    }

    /// `( expr )`
    fn parse_paren(&mut self) -> Parsed {
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

    /// Function call: `IDENT ('.' IDENT)* '(' args? ')'`. The dotted form is Excel's
    /// (`STDEV.P(`); the parts must touch (no spaces around the dots). A bare identifier is
    /// only ever a function name (variables start with `:`), so this cannot clash with
    /// `:var.method()`.
    #[inline(never)]
    fn parse_function_call(&mut self) -> Parsed {
        let mut name = match &self.lookahead {
            Token::Identifier(name) => name.to_uppercase(),
            _ => return self.parse_leaf(),
        };
        let mut end = self.lexer.last_end();
        self.bump()?; // consume ident
        while matches!(self.lookahead, Token::Dot) && self.look_pos == end {
            let mut next = self.lexer.clone();
            match next.next_token()? {
                Token::Identifier(part) if next.last_start() == end + 1 => {
                    name.push('.');
                    name.push_str(&part.to_uppercase());
                    end = next.last_end();
                    self.bump()?; // '.'
                    self.bump()?; // part
                }
                _ => break,
            }
        }
        match self.lookahead {
            Token::LParen => {
                self.bump()?; // '('
                let (args, max_child) =
                    self.parse_call_args("Expected ',' or ')' in argument list")?;
                let depth = self.node_depth(max_child)?;
                Ok((Expr::FunctionCall { name, args }, depth))
            }
            _ if name == "NOT" => {
                self.err_here("NOT binds looser than comparisons here; write (NOT x)")
            }
            _ => self.err_here("Unexpected identifier (expected function call)"),
        }
    }

    /// Atoms that do not nest: literals and variables (and the unexpected-token error).
    #[inline(never)]
    fn parse_leaf(&mut self) -> Parsed {
        match self.lookahead.clone() {
            Token::Number(n) => {
                self.bump()?;
                Ok((Expr::Number(n), 0))
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

    /// `:: type` after an already parsed operand. Both errors point at the token after `::`.
    #[inline(never)]
    fn parse_cast_tail(&mut self, (node, d): (Expr, usize)) -> Parsed {
        self.bump()?; // '::'
        let name_pos = self.look_pos;
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
                    _ => return Err(Error::new("Unknown cast type", Some(name_pos))),
                }
            }
            _ => return self.err_here("Expected type name after '::'"),
        };
        let depth = self.node_depth(Some(d))?;
        Ok((
            Expr::TypeCast {
                expr: Rc::new(node),
                ty: tname,
            },
            depth,
        ))
    }

    /// Postfix chain: `.name`, `.name?`, `.name(args)`, `&.name`, `&.name(args)`, `[i]`,
    /// `[a:b]`. Each link wraps the tree one level deeper and counts against the limit.
    #[inline(never)]
    fn parse_postfix_chain(&mut self, (mut node, mut depth): (Expr, usize)) -> Parsed {
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

/// Internal function name the `&` operator parses to (two arguments, joined as text with
/// `CONCAT` rules). Like `__TERNARY__`, the evaluator handles it before custom functions.
pub(crate) const CONCAT_OP: &str = "__CONCAT__";

/// Precedence of `AND`/`&&`. The `NOT` keyword sits between it and the comparisons.
const AND_PREC: u8 = 2;

/// Binary operator and precedence (higher binds tighter) for a token, if it is one. The
/// operator is `None` for `&`, which has no `BinaryOp` and becomes a `CONCAT_OP` call.
fn binary_op(tok: &Token) -> Option<(Option<BinaryOp>, u8)> {
    let (op, prec) = match tok {
        Token::Or | Token::OrOr => (BinaryOp::Or, 1),
        Token::And | Token::AndAnd => (BinaryOp::And, AND_PREC),
        // The lexer reads `AND`/`OR` as names; after an operand they are the operators, before
        // one they stay the `AND(...)`/`OR(...)` builtins.
        Token::Identifier(name) if name.eq_ignore_ascii_case("OR") => (BinaryOp::Or, 1),
        Token::Identifier(name) if name.eq_ignore_ascii_case("AND") => (BinaryOp::And, AND_PREC),
        Token::EqEq => (BinaryOp::Eq, 3),
        Token::NotEq => (BinaryOp::Ne, 3),
        Token::Greater => (BinaryOp::Gt, 4),
        Token::Less => (BinaryOp::Lt, 4),
        Token::Ge => (BinaryOp::Ge, 4),
        Token::Le => (BinaryOp::Le, 4),
        Token::Ampersand => return Some((None, 5)),
        Token::Plus => (BinaryOp::Add, 6),
        Token::Minus => (BinaryOp::Sub, 6),
        Token::Star => (BinaryOp::Mul, 7),
        Token::Slash => (BinaryOp::Div, 7),
        Token::Percent => (BinaryOp::Mod, 7),
        _ => return None,
    };
    Some((Some(op), prec))
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
