use thiserror::Error;

/// Main error type for the Aurora security assessment platform
#[derive(Error, Debug)]
pub enum AuroraError {
    #[error("Cryptographic operation failed: {0}")]
    Crypto(#[from] CryptoError),

    #[error("Network operation failed: {0}")]
    Network(#[from] NetworkError),

    #[error("Session management error: {0}")]
    Session(#[from] SessionError),

    #[error("File system operation failed: {0}")]
    FileSystem(#[from] FileSystemError),

    #[error("Plugin system error: {0}")]
    Plugin(#[from] PluginError),

    #[error("Legal compliance violation: {0}")]
    Compliance(#[from] ComplianceError),

    #[error("Database operation failed: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Generic error: {0}")]
    Generic(#[from] anyhow::Error),
}

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Key generation failed")]
    KeyGeneration,

    #[error("Encryption failed")]
    Encryption,

    #[error("Decryption failed")]
    Decryption,

    #[error("Key exchange failed")]
    KeyExchange,

    #[error("Invalid key format")]
    InvalidKey,

    #[error("HSM operation failed: {0}")]
    HsmOperation(String),

    #[error("Unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),

    #[error("Serialization failed")]
    Serialization,
}

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed")]
    ConnectionFailed,

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Proxy configuration error")]
    ProxyConfig,

    #[error("Traffic analysis detected")]
    TrafficAnalysis,

    #[error("Stealth mode violation")]
    StealthViolation,
}

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("Session expired")]
    Expired,

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Session limit exceeded")]
    LimitExceeded,

    #[error("Heartbeat timeout")]
    HeartbeatTimeout,
}

#[derive(Error, Debug)]
pub enum FileSystemError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Cache operation failed")]
    CacheError,

    #[error("File monitoring error")]
    MonitoringError,
}

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Plugin load failed: {0}")]
    LoadFailed(String),

    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),

    #[error("WASM runtime error: {0}")]
    WasmRuntime(String),
}

#[derive(Error, Debug)]
pub enum ComplianceError {
    #[error("Legal authorization required")]
    AuthorizationRequired,

    #[error("Jurisdiction violation: {0}")]
    JurisdictionViolation(String),

    #[error("Warrant expired or invalid")]
    InvalidWarrant,

    #[error("Target validation failed")]
    TargetValidationFailed,

    #[error("Audit trail corruption")]
    AuditTrailCorruption,
}

/// Result type alias for Aurora operations
pub type AuroraResult<T> = Result<T, AuroraError>;

/// Convenience macro for creating Aurora errors
#[macro_export]
macro_rules! aurora_error {
    ($kind:expr, $msg:expr) => {
        AuroraError::Generic(anyhow::anyhow!("{}: {}", stringify!($kind), $msg))
    };
}