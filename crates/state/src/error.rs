use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("Storage error: {0}")]
    Storage(#[from] velochain_storage::StorageError),

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Insufficient balance: has {has}, needs {needs}")]
    InsufficientBalance { has: String, needs: String },

    #[error("Nonce mismatch: expected {expected}, got {got}")]
    NonceMismatch { expected: u64, got: u64 },

    #[error("State root mismatch: expected {expected}, got {got}")]
    StateRootMismatch { expected: String, got: String },

    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}
