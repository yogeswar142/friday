use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Platform capture error: {0}")]
    CaptureError(String),

    #[error("Platform injection error: {0}")]
    InjectionError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Display query error: {0}")]
    DisplayError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

pub type Result<T> = std::result::Result<T, AgentError>;
