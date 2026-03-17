//! Unit tests for the transaction pool.

use super::*;
use alloy_primitives::U256;
use velochain_primitives::transaction::GameAction;
use velochain_primitives::{Keypair, Transaction, TxType};

fn make_signed_tx(
    kp: &Keypair,
    nonce: u64,
    gas_price: u128,
) -> velochain_primitives::SignedTransaction {
    let tx = Transaction {
        tx_type: TxType::Legacy,
        chain_id: 27181,
        nonce,
        gas_price: Some(gas_price),
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        gas_limit: 21_000,
        to: Some(alloy_primitives::Address::ZERO),
        value: U256::ZERO,
        input: Vec::new(),
        game_action: None,
    };
    tx.sign(kp).unwrap()
}

fn make_game_action_tx(kp: &Keypair, nonce: u64) -> velochain_primitives::SignedTransaction {
    let tx = Transaction::new_game_action(27181, nonce, GameAction::Respawn);
    tx.sign(kp).unwrap()
}

#[test]
fn test_add_and_count() {
    let pool = pool::TransactionPool::new();
    let kp = Keypair::random();
    let tx = make_signed_tx(&kp, 0, 1_000_000_000);
    let hash = pool.add_transaction(tx).unwrap();
    assert_eq!(pool.pending_count(), 1);
    assert!(pool.contains(&hash));
}

#[test]
fn test_duplicate_rejected() {
    let pool = pool::TransactionPool::new();
    let kp = Keypair::random();
    let tx = make_signed_tx(&kp, 0, 1_000_000_000);
    pool.add_transaction(tx.clone()).unwrap();
    assert!(pool.add_transaction(tx).is_err());
}

#[test]
fn test_nonce_collision_rejected() {
    let pool = pool::TransactionPool::new();
    let kp = Keypair::random();
    let tx1 = make_signed_tx(&kp, 0, 1_000_000_000);
    let tx2 = make_signed_tx(&kp, 0, 2_000_000_000); // same nonce, different gas price
    pool.add_transaction(tx1).unwrap();
    assert!(pool.add_transaction(tx2).is_err());
}

#[test]
fn test_pool_capacity() {
    let pool = pool::TransactionPool::with_max_size(2);
    let kp1 = Keypair::random();
    let kp2 = Keypair::random();
    let kp3 = Keypair::random();
    pool.add_transaction(make_signed_tx(&kp1, 0, 1_000_000_000))
        .unwrap();
    pool.add_transaction(make_signed_tx(&kp2, 0, 1_000_000_000))
        .unwrap();
    assert!(pool
        .add_transaction(make_signed_tx(&kp3, 0, 1_000_000_000))
        .is_err());
}

#[test]
fn test_remove_transaction() {
    let pool = pool::TransactionPool::new();
    let kp = Keypair::random();
    let tx = make_signed_tx(&kp, 0, 1_000_000_000);
    let hash = pool.add_transaction(tx).unwrap();
    assert_eq!(pool.pending_count(), 1);
    let removed = pool.remove_transaction(&hash);
    assert!(removed.is_some());
    assert_eq!(pool.pending_count(), 0);
    assert!(!pool.contains(&hash));
}

#[test]
fn test_remove_nonexistent() {
    let pool = pool::TransactionPool::new();
    let hash = alloy_primitives::B256::ZERO;
    assert!(pool.remove_transaction(&hash).is_none());
}

#[test]
fn test_get_pending_ordering() {
    let pool = pool::TransactionPool::new();
    let kp1 = Keypair::random();
    let kp2 = Keypair::random();

    // kp1 low gas price, kp2 high gas price
    pool.add_transaction(make_signed_tx(&kp1, 0, 1_000_000_000))
        .unwrap();
    pool.add_transaction(make_signed_tx(&kp2, 0, 5_000_000_000))
        .unwrap();

    let pending = pool.get_pending(10);
    assert_eq!(pending.len(), 2);
    // Higher gas price should come first
    assert!(pending[0].transaction.gas_price.unwrap() >= pending[1].transaction.gas_price.unwrap());
}

#[test]
fn test_get_pending_max_count() {
    let pool = pool::TransactionPool::new();
    let kp = Keypair::random();
    for i in 0..5 {
        pool.add_transaction(make_signed_tx(&kp, i, 1_000_000_000))
            .unwrap();
    }
    let pending = pool.get_pending(3);
    assert_eq!(pending.len(), 3);
}

#[test]
fn test_get_game_actions() {
    let pool = pool::TransactionPool::new();
    let kp = Keypair::random();
    pool.add_transaction(make_signed_tx(&kp, 0, 1_000_000_000))
        .unwrap();
    pool.add_transaction(make_game_action_tx(&kp, 1)).unwrap();

    let actions = pool.get_game_actions();
    assert_eq!(actions.len(), 1);
    assert!(actions[0].is_game_action());
}

#[test]
fn test_remove_included() {
    let pool = pool::TransactionPool::new();
    let kp = Keypair::random();
    let h1 = pool
        .add_transaction(make_signed_tx(&kp, 0, 1_000_000_000))
        .unwrap();
    let h2 = pool
        .add_transaction(make_signed_tx(&kp, 1, 1_000_000_000))
        .unwrap();
    assert_eq!(pool.pending_count(), 2);

    pool.remove_included(&[h1]);
    assert_eq!(pool.pending_count(), 1);
    assert!(!pool.contains(&h1));
    assert!(pool.contains(&h2));
}

#[test]
fn test_clear() {
    let pool = pool::TransactionPool::new();
    let kp = Keypair::random();
    pool.add_transaction(make_signed_tx(&kp, 0, 1_000_000_000))
        .unwrap();
    pool.add_transaction(make_signed_tx(&kp, 1, 1_000_000_000))
        .unwrap();
    assert_eq!(pool.pending_count(), 2);
    pool.clear();
    assert_eq!(pool.pending_count(), 0);
}

#[test]
fn test_pending_nonce() {
    let pool = pool::TransactionPool::new();
    let kp = Keypair::random();
    let addr = kp.address();

    assert_eq!(pool.pending_nonce(&addr), None);
    pool.add_transaction(make_signed_tx(&kp, 0, 1_000_000_000))
        .unwrap();
    assert_eq!(pool.pending_nonce(&addr), Some(1));
    pool.add_transaction(make_signed_tx(&kp, 1, 1_000_000_000))
        .unwrap();
    assert_eq!(pool.pending_nonce(&addr), Some(2));
}

#[test]
fn test_get_sender_txs() {
    let pool = pool::TransactionPool::new();
    let kp1 = Keypair::random();
    let kp2 = Keypair::random();

    pool.add_transaction(make_signed_tx(&kp1, 0, 1_000_000_000))
        .unwrap();
    pool.add_transaction(make_signed_tx(&kp1, 1, 1_000_000_000))
        .unwrap();
    pool.add_transaction(make_signed_tx(&kp2, 0, 1_000_000_000))
        .unwrap();

    let kp1_txs = pool.get_sender_txs(&kp1.address());
    assert_eq!(kp1_txs.len(), 2);

    let kp2_txs = pool.get_sender_txs(&kp2.address());
    assert_eq!(kp2_txs.len(), 1);
}

#[test]
fn test_multiple_senders_contiguous_nonces() {
    let pool = pool::TransactionPool::new();
    let kp1 = Keypair::random();
    let kp2 = Keypair::random();

    // kp1: nonces 0, 1, 2
    for i in 0..3 {
        pool.add_transaction(make_signed_tx(&kp1, i, 1_000_000_000))
            .unwrap();
    }
    // kp2: nonces 0, 1
    for i in 0..2 {
        pool.add_transaction(make_signed_tx(&kp2, i, 2_000_000_000))
            .unwrap();
    }

    let pending = pool.get_pending(100);
    assert_eq!(pending.len(), 5);
}

#[test]
fn test_default_impl() {
    let pool = pool::TransactionPool::default();
    assert_eq!(pool.pending_count(), 0);
    assert_eq!(pool.queued_count(), 0);
    assert_eq!(pool.total_count(), 0);
}
