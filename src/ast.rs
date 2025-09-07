use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    StringLit(String),
    Null,
    Unary(UnaryOp, Rc<Expr>),
    Binary(Rc<Expr>, BinaryOp, Rc<Expr>),
    Variable(String),
    PropertyAccess { target: Rc<Expr>, property: String },
    SafePropertyAccess { target: Rc<Expr>, property: String },
    SafeMethodCall { target: Rc<Expr>, name: String, args: Vec<Expr> },
    FunctionCall { name: String, args: Vec<Expr> },
    Spread(Rc<Expr>),
    Array(Vec<Expr>),
    ObjectLiteral(Vec<(String, Expr)>),
    MethodCall { target: Rc<Expr>, name: String, args: Vec<Expr>, predicate: bool },
    Index { target: Rc<Expr>, index: Rc<Expr> },
    Slice { target: Rc<Expr>, start: Option<Rc<Expr>>, end: Option<Rc<Expr>> },
    TypeCast { expr: Rc<Expr>, ty: TypeName },
    Assignment { variable: String, value: Rc<Expr> },
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
