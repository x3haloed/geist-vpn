//! Error types for the Geist VPN application


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

    #[error("Memory allocation failed: {message}")]
    MemoryError { message: String },

    #[error("String encoding error: {message}")]
    EncodingError { message: String },

    #[error("SoftEther error code {code}: {message}")]
    SoftEtherError { code: i32, message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_yaml::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Nul byte in string")]
    NulError(#[from] std::ffi::NulError),

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
        let message = match code {
            // Success codes (should not create errors)
            0 => panic!("Success code should not create error"),

            // Connection errors
            1 => "Connection to the server has failed",
            2 => "The destination server is not a VPN server",
            3 => "The connection has been interrupted",
            4 => "Protocol error",
            5 => "Connecting client is not a VPN client",
            6 => "User cancel",

            // Authentication errors
            7 => "Specified authentication method is not supported",
            8 => "The HUB does not exist",
            9 => "Authentication failure",
            19 => "Failed to authenticate on the proxy server",

            // Session/HUB errors
            10 => "HUB is stopped",
            11 => "Session has been deleted",
            12 => "Access denied",
            13 => "Session times out",
            14 => "Protocol is invalid",
            15 => "Too many connections",
            16 => "Too many sessions of the HUB",
            20 => "Too many sessions of the same user",

            // Proxy errors
            17 => "Connection to the proxy server fails",
            18 => "Proxy Error",

            // License/Device errors
            21 => "License error",
            22 => "Device driver error",
            23 => "Internal error",

            // Secure device errors
            24 => "The secure device cannot be opened",
            25 => "PIN code is incorrect",
            26 => "Specified certificate is not stored",
            27 => "Specified private key is not stored",
            28 => "Write failure",
            39 => "Error has occurred in the operation of the secure device",
            40 => "Secure device is not specified",

            // Object/Account errors
            29 => "Specified object can not be found",
            34 => "Account already exists",
            35 => "Account is operating",
            36 => "Specified account doesn't exist",
            37 => "Account is offline",

            // Virtual LAN errors
            30 => "Virtual LAN card with the specified name already exists",
            31 => "Specified virtual LAN card cannot be created",
            32 => "Specified name of the virtual LAN card is invalid",
            41 => "Virtual LAN card in use by account",
            42 => "Virtual LAN card of the account can not be found",
            43 => "Virtual LAN card of the account is already in use",
            44 => "Virtual LAN card of the account is disabled",

            // Parameter/Value errors
            38 => "Parameter is invalid",
            45 => "Value is invalid",

            // Farm/Controller errors
            46 => "Not a farm controller",
            47 => "Attempting to connect",
            48 => "Failed to connect to the farm controller",

            // Generic errors
            33 => "Unsupported",
            _ => "Unknown error",
        };

        Self::SoftEtherError {
            code,
            message: message.into(),
        }
    }

    /// Create a memory allocation error
    pub fn memory_error<S: Into<String>>(message: S) -> Self {
        Self::MemoryError { message: message.into() }
    }

    /// Create a string encoding error
    pub fn encoding_error<S: Into<String>>(message: S) -> Self {
        Self::EncodingError { message: message.into() }
    }
}

/// Convert from anyhow::Error for compatibility
impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.to_string())
    }
}
