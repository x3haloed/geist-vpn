//! Error types for the Geist VPN application

use std::fmt;

/// Result type alias for Geist VPN operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types that can occur in VPN operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Initialization failed: {message}")]
    InitializationFailed { message: String },

    #[error("Connection failed: {message}")]
    ConnectionFailed { message: String },

    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Profile error: {message}")]
    ProfileError { message: String },

    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("FFI error: {message}")]
    FfiError { message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_yaml::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Generic error: {0}")]
    Other(String),
}

impl Error {
    /// Create a new generic error
    pub fn new<S: Into<String>>(message: S) -> Self {
        Self::Other(message.into())
    }

    /// Create an FFI error with the given message
    pub fn ffi_error<S: Into<String>>(message: S) -> Self {
        Self::FfiError { message: message.into() }
    }

    /// Create a connection error with the given message
    pub fn connection_error<S: Into<String>>(message: S) -> Self {
        Self::ConnectionFailed { message: message.into() }
    }

    /// Convert from a SoftEtherVPN error code
    pub fn from_softether_error(code: i32) -> Self {
        match code {
            0 => panic!("Success code should not create error"),
            1 => Self::ConnectionFailed { message: "Connection timeout".into() },
            2 => Self::AuthenticationFailed { message: "Invalid credentials".into() },
            3 => Self::NetworkError { message: "Network unreachable".into() },
            _ => Self::FfiError { message: format!("SoftEther error code: {}", code) },
        }
    }
}

/// Convert from anyhow::Error for compatibility
impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.to_string())
    }
}
