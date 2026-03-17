use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Peer disconnected: {0}")]
    PeerDisconnected(String),

    #[error("Message encoding error: {0}")]
    Encoding(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Channel closed")]
    ChannelClosed,
}
