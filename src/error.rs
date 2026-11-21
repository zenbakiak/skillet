use std::fmt::{Display, Formatter};

/// A parse or evaluation error.
///
/// `Display` appends the position when there is one.
///
/// ```rust
/// let err = skillet::evaluate("2 +* 3").unwrap_err();
/// assert_eq!(err.position, Some(3));
/// assert_eq!(err.to_string(), format!("{} at position 3", err.message));
///
/// let err = skillet::evaluate(":missing").unwrap_err();
/// assert_eq!(err.to_string(), "Missing variable: :missing");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    /// Human-readable description.
    pub message: String,
    /// Byte offset in the expression text, when the error points at one token.
    pub position: Option<usize>,
}

impl Error {
    pub fn new<M: Into<String>>(message: M, position: Option<usize>) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.position {
            Some(pos) => write!(f, "{} at position {}", self.message, pos),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for Error {}
