use thiserror::Error;

#[derive(Debug, Error)]
pub enum TxPoolError {
    #[error("Transaction already in pool: {0}")]
    AlreadyExists(String),

    #[error("Pool is full: capacity={capacity}, current={current}")]
    PoolFull { capacity: usize, current: usize },

    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("Nonce too low: expected {expected}, got {got}")]
    NonceTooLow { expected: u64, got: u64 },

    #[error("Insufficient funds for gas")]
    InsufficientFunds,
}
