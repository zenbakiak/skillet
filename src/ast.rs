#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    StringLit(String),
    Null,
    Unary(UnaryOp, Box<Expr>),
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Variable(String),
    FunctionCall { name: String, args: Vec<Expr> },
    Spread(Box<Expr>),
    Array(Vec<Expr>),
    MethodCall { target: Box<Expr>, name: String, args: Vec<Expr>, predicate: bool },
    Index { target: Box<Expr>, index: Box<Expr> },
    Slice { target: Box<Expr>, start: Option<Box<Expr>>, end: Option<Box<Expr>> },
    TypeCast { expr: Box<Expr>, ty: TypeName },
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}
