use thiserror::Error;

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid params: {0}")]
    InvalidParams(String),

    #[error("Transaction pool error: {0}")]
    TxPool(String),

    #[error("Block not found: {0}")]
    BlockNotFound(String),

    #[error("Server error: {0}")]
    Server(String),
}
