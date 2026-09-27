use thiserror::Error;

#[derive(Error, Debug, PartialEq, Clone)]
pub enum CoreError {
    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Invalid packet format or opcode: {0}")]
    InvalidPacket(u8),

    #[error("Edge router error: {0}")]
    RouterError(String),

    #[error("Invalid coordinate normalization: ({0}, {1}) out of bounds")]
    InvalidCoordinates(f32, f32),

    #[error("Device capability mismatch: {0}")]
    CapabilityError(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
