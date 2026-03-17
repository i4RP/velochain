use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvmError {
    #[error("EVM execution reverted: {0}")]
    Revert(String),

    #[error("EVM execution halted: {0}")]
    Halt(String),

    #[error("Out of gas: used {used}, limit {limit}")]
    OutOfGas { used: u64, limit: u64 },

    #[error("Invalid transaction for EVM: {0}")]
    InvalidTransaction(String),

    #[error("State error: {0}")]
    State(String),

    #[error("Internal EVM error: {0}")]
    Internal(String),
}
