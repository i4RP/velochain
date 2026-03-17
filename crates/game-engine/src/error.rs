use thiserror::Error;

#[derive(Debug, Error)]
pub enum GameEngineError {
    #[error("Entity not found: {0}")]
    EntityNotFound(u64),

    #[error("Invalid action: {0}")]
    InvalidAction(String),

    #[error("World not initialized")]
    WorldNotInitialized,

    #[error("Physics error: {0}")]
    Physics(String),

    #[error("Terrain error: {0}")]
    Terrain(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Determinism violation: {0}")]
    DeterminismViolation(String),

    #[error("State error: {0}")]
    State(#[from] velochain_state::StateError),
}
