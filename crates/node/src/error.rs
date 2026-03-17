use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("Storage error: {0}")]
    Storage(#[from] velochain_storage::StorageError),

    #[error("State error: {0}")]
    State(#[from] velochain_state::StateError),

    #[error("Consensus error: {0}")]
    Consensus(#[from] velochain_consensus::ConsensusError),

    #[error("Network error: {0}")]
    Network(#[from] velochain_network::NetworkError),

    #[error("EVM error: {0}")]
    Evm(#[from] velochain_evm::EvmError),

    #[error("Game engine error: {0}")]
    GameEngine(#[from] velochain_game_engine::GameEngineError),

    #[error("RPC error: {0}")]
    Rpc(#[from] velochain_rpc::RpcError),

    #[error("TxPool error: {0}")]
    TxPool(#[from] velochain_txpool::TxPoolError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Node already running")]
    AlreadyRunning,

    #[error("Node not initialized")]
    NotInitialized,
}
