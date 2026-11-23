use std::rc::Rc;

/// A parsed expression ([`crate::parse`]). `#[non_exhaustive]` (since 0.7.0): new syntax adds
/// variants, so a `match` outside this crate needs a `_` arm.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Expr {
    Number(f64),
    StringLit(String),
    Null,
    Unary(UnaryOp, Rc<Expr>),
    Binary(Rc<Expr>, BinaryOp, Rc<Expr>),
    Variable(String),
    PropertyAccess {
        target: Rc<Expr>,
        property: String,
    },
    SafePropertyAccess {
        target: Rc<Expr>,
        property: String,
    },
    SafeMethodCall {
        target: Rc<Expr>,
        name: String,
        args: Vec<Expr>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expr>,
    },
    Spread(Rc<Expr>),
    Array(Vec<Expr>),
    ObjectLiteral(Vec<(String, Expr)>),
    MethodCall {
        target: Rc<Expr>,
        name: String,
        args: Vec<Expr>,
        predicate: bool,
    },
    Index {
        target: Rc<Expr>,
        index: Rc<Expr>,
    },
    Slice {
        target: Rc<Expr>,
        start: Option<Rc<Expr>>,
        end: Option<Rc<Expr>>,
    },
    TypeCast {
        expr: Rc<Expr>,
        ty: TypeName,
    },
    Assignment {
        variable: String,
        value: Rc<Expr>,
    },
    Sequence(Vec<Expr>),
}

/// The target of a `::Type` cast. `#[non_exhaustive]` (since 0.7.0), like [`Expr`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypeName {
    Integer,
    Float,
    String,
    Boolean,
    Array,
    Currency,
    DateTime,
    Json,
}

/// A prefix operator. `#[non_exhaustive]` (since 0.7.0), like [`Expr`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
}

/// An infix operator. `#[non_exhaustive]` (since 0.7.0), like [`Expr`]: `Concat` was added in
/// 0.7.0 (ADR-0034).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Gt,
    Lt,
    Ge,
    Le,
    Eq,
    Ne,
    And,
    Or,
    /// `a & b`: both operands' text joined, as `CONCAT(a, b)` (ADR-0034 §6).
    Concat,
}
