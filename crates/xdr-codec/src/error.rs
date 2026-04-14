use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdrError {
    #[error("buffer too short: needed {needed} bytes, {available} available")]
    BufferTooShort { needed: usize, available: usize },

    #[error("invalid bool value: {0} (expected 0 or 1)")]
    InvalidBool(u32),

    #[error("invalid enum discriminant {discriminant} for type {type_name}")]
    InvalidEnum {
        discriminant: u32,
        type_name: &'static str,
    },

    #[error("string is not valid UTF-8: {0}")]
    StringNotUtf8(#[from] std::string::FromUtf8Error),

    #[error("length {actual} exceeds maximum {max}")]
    LengthExceeded { max: u32, actual: u32 },
}
