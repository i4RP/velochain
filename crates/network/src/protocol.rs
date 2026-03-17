//! Network protocol message definitions.

use serde::{Deserialize, Serialize};
use velochain_primitives::{Block, SignedTransaction};

/// Topic names for gossipsub.
pub mod topics {
    pub const BLOCKS: &str = "/velochain/blocks/1.0.0";
    pub const TRANSACTIONS: &str = "/velochain/txs/1.0.0";
    pub const GAME_STATE: &str = "/velochain/gamestate/1.0.0";
}

/// Network messages exchanged between peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// A new block announcement.
    NewBlock(Block),
    /// A new transaction announcement.
    NewTransaction(SignedTransaction),
    /// Request a block by number.
    GetBlock { number: u64 },
    /// Response with a requested block.
    BlockResponse(Option<Block>),
    /// Request the current chain head.
    GetHead,
    /// Response with current chain head info.
    HeadResponse {
        number: u64,
        hash: [u8; 32],
        game_tick: u64,
    },
    /// Request a range of block headers.
    GetHeaders {
        start_block: u64,
        max_headers: u64,
    },
    /// Response with a batch of block headers.
    HeadersResponse(Vec<velochain_primitives::BlockHeader>),
    /// Request block bodies by hashes.
    GetBodies {
        hashes: Vec<[u8; 32]>,
    },
    /// Response with block bodies.
    BodiesResponse(Vec<velochain_primitives::BlockBody>),
    /// Status message exchanged on peer connect.
    Status {
        chain_id: u64,
        best_block: u64,
        best_hash: [u8; 32],
        genesis_hash: [u8; 32],
    },
    /// Ping/pong for liveness.
    Ping(u64),
    Pong(u64),
}
