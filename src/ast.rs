#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    StringLit(String),
    Null,
    Unary(UnaryOp, Box<Expr>),
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Variable(String),
    PropertyAccess { target: Box<Expr>, property: String },
    SafePropertyAccess { target: Box<Expr>, property: String },
    FunctionCall { name: String, args: Vec<Expr> },
    Spread(Box<Expr>),
    Array(Vec<Expr>),
    ObjectLiteral(Vec<(String, Expr)>),
    MethodCall { target: Box<Expr>, name: String, args: Vec<Expr>, predicate: bool },
    Index { target: Box<Expr>, index: Box<Expr> },
    Slice { target: Box<Expr>, start: Option<Box<Expr>>, end: Option<Box<Expr>> },
    TypeCast { expr: Box<Expr>, ty: TypeName },
    Assignment { variable: String, value: Box<Expr> },
    Sequence(Vec<Expr>),
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
