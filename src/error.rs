use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub message: String,
    pub position: Option<usize>,
}

impl Error {
    pub fn new<M: Into<String>>(message: M, position: Option<usize>) -> Self {
        Self { message: message.into(), position }
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
