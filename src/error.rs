use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportError {
    message: String,
}

impl ReportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(message)
    }

    pub fn csv(message: impl Into<String>) -> Self {
        Self::new(message)
    }
}

impl Display for ReportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ReportError {}
