//! Chain reorganization detection and handling.
//!
//! When a peer provides a longer valid chain, the node must "reorg" by
//! unwinding the current chain back to the common ancestor and then
//! applying the new blocks from the canonical chain.

use std::sync::Arc;
use tracing::{info, warn};
use velochain_primitives::{Block, BlockHeader};
use velochain_storage::Database;

use crate::chain::Chain;
use crate::error::NodeError;

/// Result of a chain reorganization.
#[derive(Debug)]
pub struct ReorgOutcome {
    /// The common ancestor block number.
    pub common_ancestor: u64,
    /// Number of blocks unwound from the old chain.
    pub blocks_reverted: usize,
    /// Number of blocks applied from the new chain.
    pub blocks_applied: usize,
    /// New chain head block number.
    pub new_head: u64,
}

/// Detect whether a reorganization is needed by comparing the incoming
/// block's parent chain with our current canonical chain.
///
/// Returns `Some(fork_point)` if a reorg is needed (the fork block number),
/// or `None` if the block extends the current chain normally.
pub fn detect_fork(
    db: &Arc<Database>,
    current_head: &BlockHeader,
    incoming_header: &BlockHeader,
) -> Result<Option<u64>, NodeError> {
    // If the incoming block extends our head, no reorg needed
    if incoming_header.parent_hash == current_head.hash() {
        return Ok(None);
    }

    // If incoming block number is not greater than our head, skip it
    if incoming_header.number <= current_head.number {
        return Ok(None);
    }

    // Walk back the incoming chain to find a common ancestor
    // For now, we check if the incoming block's parent is in our canonical chain
    let parent_hash = incoming_header.parent_hash;
    let parent_hash_bytes: [u8; 32] = parent_hash.0;

    if let Some(parent_header) = db.get_header(&parent_hash_bytes)? {
        // Check if the parent is on our canonical chain
        if let Some(canonical_hash) = db.get_block_hash_by_number(parent_header.number)? {
            if canonical_hash == parent_hash_bytes {
                // Parent is on our canonical chain - this is a simple extension
                // (but we might have a different block at the same height)
                return Ok(None);
            }
        }
        // Parent exists but is not on canonical chain - fork detected
        // Walk back to find common ancestor
        let fork_point = find_common_ancestor(db, current_head.number, &parent_hash_bytes)?;
        return Ok(Some(fork_point));
    }

    // Parent not found in our DB - we might need to sync
    Ok(None)
}

/// Find the common ancestor between the current canonical chain and a
/// diverging chain identified by a block hash.
fn find_common_ancestor(
    db: &Arc<Database>,
    current_height: u64,
    diverging_hash: &[u8; 32],
) -> Result<u64, NodeError> {
    // Walk the diverging chain backwards
    let mut check_hash = *diverging_hash;

    for _ in 0..current_height {
        if let Some(header) = db.get_header(&check_hash)? {
            // Check if this block is on the canonical chain
            if let Some(canonical_hash) = db.get_block_hash_by_number(header.number)? {
                if canonical_hash == check_hash {
                    return Ok(header.number);
                }
            }
            check_hash = header.parent_hash.0;
        } else {
            break;
        }
    }

    // If we can't find a common ancestor, return genesis
    Ok(0)
}

/// Execute a chain reorganization.
///
/// 1. Identify blocks to revert (from current head back to fork point)
/// 2. Identify blocks to apply (from fork point to new chain tip)
/// 3. Revert old blocks (return transactions to pool)
/// 4. Apply new blocks
pub fn execute_reorg(
    chain: &Arc<Chain>,
    fork_point: u64,
    new_blocks: &[Block],
) -> Result<ReorgOutcome, NodeError> {
    let current_head = chain.block_number();

    if fork_point > current_head {
        return Err(NodeError::Internal(
            "Fork point is ahead of current head".into(),
        ));
    }

    let blocks_to_revert = (current_head - fork_point) as usize;
    info!(
        "Executing chain reorg: fork_point={}, reverting {} blocks, applying {} new blocks",
        fork_point,
        blocks_to_revert,
        new_blocks.len()
    );

    // 1. Collect transactions from reverted blocks and return to pool
    for block_num in (fork_point + 1..=current_head).rev() {
        if let Some(block) = chain.get_block_by_number(block_num)? {
            // Return transactions from reverted block back to the txpool
            for tx in &block.body.transactions {
                if let Err(e) = chain.txpool().add_transaction(tx.clone()) {
                    warn!(
                        "Failed to return tx {} to pool during reorg: {}",
                        tx.hash, e
                    );
                }
            }
            info!(
                "Reverted block {} ({} txs returned to pool)",
                block_num,
                block.body.transactions.len()
            );
        }
    }

    // 2. Reset chain head to the fork point
    // The fork point block is the common ancestor, so we restore head to it
    if let Some(fork_block) = chain.get_block_by_number(fork_point)? {
        chain.set_head(fork_block.header)?;
    }

    // 3. Apply new blocks on top of the fork point
    let mut applied = 0;
    for block in new_blocks {
        match chain.apply_block(block) {
            Ok(()) => {
                applied += 1;
                info!("Applied reorg block: number={}", block.number());
            }
            Err(e) => {
                warn!("Failed to apply reorg block {}: {}", block.number(), e);
                break;
            }
        }
    }

    let new_head = chain.block_number();
    info!(
        "Chain reorg complete: reverted={}, applied={}, new_head={}",
        blocks_to_revert, applied, new_head
    );

    Ok(ReorgOutcome {
        common_ancestor: fork_point,
        blocks_reverted: blocks_to_revert,
        blocks_applied: applied,
        new_head,
    })
}
