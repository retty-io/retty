use std::fmt::{Display, Formatter};
use std::net::AddrParseError;
use std::string::FromUtf8Error;

/// Errors that arise from pipeline read or write
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Error {
    /// A list specifying general categories of [Error].
    pub kind: std::io::ErrorKind,
    /// A message describing error information
    pub message: String,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} , {}", self.kind, self.message)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error {
            kind: e.kind(),
            message: e.to_string(),
        }
    }
}

impl From<AddrParseError> for Error {
    fn from(e: AddrParseError) -> Self {
        Error {
            kind: std::io::ErrorKind::AddrNotAvailable,
            message: e.to_string(),
        }
    }
}

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Self {
        Error {
            kind: std::io::ErrorKind::Other,
            message: e.to_string(),
        }
    }
}

impl Error {
    /// Creates a new error from a known kind of error as well as a message.
    pub fn new(kind: std::io::ErrorKind, message: String) -> Self {
        Error { kind, message }
    }
}
