//! Chain state snapshot export and import.
//!
//! Provides the ability to export the entire chain state (blocks, game world,
//! account state) to a portable binary format and restore from it.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use velochain_game_engine::GameWorld;
use velochain_primitives::{Block, BlockHeader};
use velochain_state::WorldState;
use velochain_storage::Database;

use crate::error::NodeError;

/// Magic bytes identifying a VeloChain snapshot file.
const SNAPSHOT_MAGIC: &[u8; 8] = b"VLCSNAP\x01";

/// Snapshot file version.
const SNAPSHOT_VERSION: u32 = 1;

/// Snapshot metadata header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    /// Snapshot format version.
    pub version: u32,
    /// Chain ID.
    pub chain_id: u64,
    /// Block number at which the snapshot was taken.
    pub block_number: u64,
    /// Block hash at the snapshot point.
    pub block_hash: String,
    /// Game tick at the snapshot point.
    pub game_tick: u64,
    /// Number of blocks included.
    pub block_count: u64,
    /// Timestamp when the snapshot was created (unix seconds).
    pub created_at: u64,
}

/// A single block entry in the snapshot (header + body serialized together).
#[derive(Debug, Serialize, Deserialize)]
struct SnapshotBlock {
    header: BlockHeader,
    body: velochain_primitives::BlockBody,
}

/// Export the chain state to a snapshot file.
///
/// The snapshot includes:
/// - All block headers and bodies from genesis to current head
/// - The current game world state
/// - Snapshot metadata
pub fn export_snapshot(
    db: &Arc<Database>,
    game_world: &Arc<GameWorld>,
    chain_id: u64,
    output_path: &Path,
) -> Result<SnapshotMeta, NodeError> {
    let latest_block = db
        .get_latest_block_number()?
        .ok_or(NodeError::NotInitialized)?;

    info!("Exporting snapshot up to block {}", latest_block);

    let mut file = std::fs::File::create(output_path)
        .map_err(|e| NodeError::Internal(format!("Failed to create snapshot file: {e}")))?;

    // Write magic bytes
    file.write_all(SNAPSHOT_MAGIC)
        .map_err(|e| NodeError::Internal(format!("Write error: {e}")))?;

    // Collect blocks
    let mut blocks: Vec<SnapshotBlock> = Vec::new();
    let mut head_hash = String::new();
    let mut head_game_tick = 0u64;

    for num in 0..=latest_block {
        let hash = db
            .get_block_hash_by_number(num)?
            .ok_or_else(|| NodeError::Internal(format!("Missing block hash for number {num}")))?;
        let header = db
            .get_header(&hash)?
            .ok_or_else(|| NodeError::Internal(format!("Missing header for block {num}")))?;
        let body = db
            .get_body(&hash)?
            .unwrap_or_else(|| velochain_primitives::BlockBody {
                transactions: vec![],
            });

        if num == latest_block {
            head_hash = format!("0x{}", hex::encode(hash));
            head_game_tick = header.game_tick;
        }

        blocks.push(SnapshotBlock { header, body });
    }

    // Serialize game world
    let game_state = game_world
        .serialize_state()
        .map_err(|e| NodeError::Internal(format!("Game world serialization: {e}")))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let meta = SnapshotMeta {
        version: SNAPSHOT_VERSION,
        chain_id,
        block_number: latest_block,
        block_hash: head_hash,
        game_tick: head_game_tick,
        block_count: blocks.len() as u64,
        created_at: now,
    };

    // Write meta
    let meta_bytes = serde_json::to_vec(&meta)
        .map_err(|e| NodeError::Internal(format!("Meta serialization: {e}")))?;
    let meta_len = meta_bytes.len() as u32;
    file.write_all(&meta_len.to_le_bytes())
        .map_err(|e| NodeError::Internal(format!("Write error: {e}")))?;
    file.write_all(&meta_bytes)
        .map_err(|e| NodeError::Internal(format!("Write error: {e}")))?;

    // Write blocks
    let blocks_bytes = bincode::serialize(&blocks)
        .map_err(|e| NodeError::Internal(format!("Block serialization: {e}")))?;
    let blocks_len = blocks_bytes.len() as u64;
    file.write_all(&blocks_len.to_le_bytes())
        .map_err(|e| NodeError::Internal(format!("Write error: {e}")))?;
    file.write_all(&blocks_bytes)
        .map_err(|e| NodeError::Internal(format!("Write error: {e}")))?;

    // Write game state
    let game_len = game_state.len() as u64;
    file.write_all(&game_len.to_le_bytes())
        .map_err(|e| NodeError::Internal(format!("Write error: {e}")))?;
    file.write_all(&game_state)
        .map_err(|e| NodeError::Internal(format!("Write error: {e}")))?;

    file.flush()
        .map_err(|e| NodeError::Internal(format!("Flush error: {e}")))?;

    info!(
        "Snapshot exported: {} blocks, game_tick={}, file={:?}",
        meta.block_count, meta.game_tick, output_path
    );

    Ok(meta)
}

/// Read snapshot metadata without importing.
pub fn read_snapshot_meta(path: &Path) -> Result<SnapshotMeta, NodeError> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| NodeError::Internal(format!("Failed to open snapshot: {e}")))?;

    // Verify magic
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;
    if &magic != SNAPSHOT_MAGIC {
        return Err(NodeError::Internal("Invalid snapshot file format".into()));
    }

    // Read meta
    let mut meta_len_bytes = [0u8; 4];
    file.read_exact(&mut meta_len_bytes)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;
    let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;

    let mut meta_bytes = vec![0u8; meta_len];
    file.read_exact(&mut meta_bytes)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;

    let meta: SnapshotMeta = serde_json::from_slice(&meta_bytes)
        .map_err(|e| NodeError::Internal(format!("Meta deserialization: {e}")))?;

    Ok(meta)
}

/// Import a snapshot into the database and game world.
pub fn import_snapshot(
    db: &Arc<Database>,
    state: &Arc<WorldState>,
    path: &Path,
    world_seed: u64,
) -> Result<(SnapshotMeta, GameWorld), NodeError> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| NodeError::Internal(format!("Failed to open snapshot: {e}")))?;

    // Verify magic
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;
    if &magic != SNAPSHOT_MAGIC {
        return Err(NodeError::Internal("Invalid snapshot file format".into()));
    }

    // Read meta
    let mut meta_len_bytes = [0u8; 4];
    file.read_exact(&mut meta_len_bytes)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;
    let meta_len = u32::from_le_bytes(meta_len_bytes) as usize;
    let mut meta_bytes = vec![0u8; meta_len];
    file.read_exact(&mut meta_bytes)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;
    let meta: SnapshotMeta = serde_json::from_slice(&meta_bytes)
        .map_err(|e| NodeError::Internal(format!("Meta deserialization: {e}")))?;

    info!(
        "Importing snapshot: {} blocks, chain_id={}, block_number={}",
        meta.block_count, meta.chain_id, meta.block_number
    );

    // Read blocks
    let mut blocks_len_bytes = [0u8; 8];
    file.read_exact(&mut blocks_len_bytes)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;
    let blocks_len = u64::from_le_bytes(blocks_len_bytes) as usize;
    let mut blocks_bytes = vec![0u8; blocks_len];
    file.read_exact(&mut blocks_bytes)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;
    let blocks: Vec<SnapshotBlock> = bincode::deserialize(&blocks_bytes)
        .map_err(|e| NodeError::Internal(format!("Block deserialization: {e}")))?;

    // Read game state
    let mut game_len_bytes = [0u8; 8];
    file.read_exact(&mut game_len_bytes)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;
    let game_len = u64::from_le_bytes(game_len_bytes) as usize;
    let mut game_bytes = vec![0u8; game_len];
    file.read_exact(&mut game_bytes)
        .map_err(|e| NodeError::Internal(format!("Read error: {e}")))?;

    // Import blocks into database
    for snap_block in &blocks {
        let block = Block::new(
            snap_block.header.clone(),
            snap_block.body.transactions.clone(),
        );
        db.put_block(&block)?;
    }

    // Set latest block number
    if let Some(last) = blocks.last() {
        db.put_latest_block_number(last.header.number)?;
    }

    // Store game state
    db.put_game_state(b"world", &game_bytes)?;

    // Commit state
    let _ = state.commit()?;

    // Restore game world
    let game_world = GameWorld::from_state(&game_bytes, world_seed)
        .map_err(|e| NodeError::Internal(format!("Game world restore: {e}")))?;

    info!(
        "Snapshot imported successfully: {} blocks restored",
        blocks.len()
    );

    Ok((meta, game_world))
}
