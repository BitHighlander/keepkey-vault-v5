// Standard error handling for the application
use std::fmt;
use std::error::Error as StdError;

// Generic error type for the application
#[derive(Debug)]
pub enum Error {
    // IO errors
    Io(std::io::Error),
    
    // Transport related errors
    TransportError(String),
    
    // Device communication errors
    DeviceError(String),
    
    // General errors
    General(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => write!(f, "IO error: {}", err),
            Error::TransportError(msg) => write!(f, "Transport error: {}", msg),
            Error::DeviceError(msg) => write!(f, "Device error: {}", msg),
            Error::General(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

// Implement conversions from common error types
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::General(err)
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Error::General(err.to_string())
    }
}
