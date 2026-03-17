use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("Invalid block: {0}")]
    InvalidBlock(String),

    #[error("Invalid header: {0}")]
    InvalidHeader(String),

    #[error("Unknown validator: {0}")]
    UnknownValidator(String),

    #[error("Not a validator")]
    NotValidator,

    #[error("Block from the future: block timestamp {block_time}, current {current_time}")]
    FutureBlock { block_time: u64, current_time: u64 },

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Wrong turn: expected {expected}, got {got}")]
    WrongTurn { expected: String, got: String },

    #[error("Duplicate block at height {0}")]
    DuplicateBlock(u64),
}
