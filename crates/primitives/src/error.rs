use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrimitivesError {
    #[error("Invalid RLP encoding: {0}")]
    RlpError(String),

    #[error("Invalid signature: {0}")]
    SignatureError(String),

    #[error("Invalid hash: expected {expected}, got {got}")]
    HashMismatch { expected: String, got: String },

    #[error("Invalid block: {0}")]
    InvalidBlock(String),

    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}
