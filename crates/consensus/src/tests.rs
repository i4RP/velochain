//! Unit tests for PoA consensus engine.

use super::*;
use alloy_primitives::{Address, B256, B64, Bloom, U256};
use velochain_primitives::{Block, BlockHeader, Keypair, DEFAULT_BLOCK_GAS_LIMIT};

fn make_genesis_header() -> BlockHeader {
    BlockHeader::genesis(B256::ZERO)
}

fn make_child_header(parent: &BlockHeader, beneficiary: Address, tick: u64) -> BlockHeader {
    BlockHeader {
        parent_hash: parent.hash(),
        ommers_hash: B256::ZERO,
        beneficiary,
        state_root: B256::ZERO,
        transactions_root: B256::ZERO,
        receipts_root: B256::ZERO,
        game_state_root: B256::ZERO,
        logs_bloom: Bloom::ZERO,
        difficulty: U256::from(1),
        number: parent.number + 1,
        gas_limit: DEFAULT_BLOCK_GAS_LIMIT,
        gas_used: 0,
        timestamp: parent.timestamp + 1,
        game_tick: tick,
        extra_data: Vec::new(),
        mix_hash: B256::ZERO,
        nonce: B64::ZERO,
        base_fee_per_gas: None,
    }
}

fn test_validators() -> (Vec<Keypair>, Vec<Address>) {
    let kp1 = Keypair::random();
    let kp2 = Keypair::random();
    let addrs = vec![kp1.address(), kp2.address()];
    (vec![kp1, kp2], addrs)
}

#[test]
fn test_poa_config_default() {
    let config = poa::PoaConfig::default();
    assert_eq!(config.period, 1);
    assert_eq!(config.epoch, 30000);
    assert_eq!(config.chain_id, velochain_primitives::DEFAULT_CHAIN_ID);
}

#[test]
fn test_readonly_is_not_validator() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs, poa::PoaConfig::default());
    assert!(!engine.is_validator());
}

#[test]
fn test_with_keypair_is_validator() {
    let (keypairs, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_with_keypair(
        keypairs[0].clone(),
        addrs,
        poa::PoaConfig::default(),
    );
    assert!(engine.is_validator());
}

#[test]
fn test_proposer_round_robin() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs.clone(), poa::PoaConfig::default());

    // Round-robin: height 0 -> validator 0, height 1 -> validator 1, height 2 -> validator 0
    assert_eq!(engine.proposer_for_height(0), Some(addrs[0]));
    assert_eq!(engine.proposer_for_height(1), Some(addrs[1]));
    assert_eq!(engine.proposer_for_height(2), Some(addrs[0]));
    assert_eq!(engine.proposer_for_height(3), Some(addrs[1]));
}

#[test]
fn test_proposer_empty_validators() {
    let engine = poa::PoaConsensus::new_readonly(vec![], poa::PoaConfig::default());
    assert_eq!(engine.proposer_for_height(0), None);
}

#[test]
fn test_is_our_turn() {
    let (keypairs, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_with_keypair(
        keypairs[0].clone(),
        addrs,
        poa::PoaConfig::default(),
    );
    // Validator 0 should produce blocks at height 0, 2, 4, ...
    assert!(engine.is_our_turn(0));
    assert!(!engine.is_our_turn(1));
    assert!(engine.is_our_turn(2));
}

#[test]
fn test_verify_header_valid() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs.clone(), poa::PoaConfig::default());

    let genesis = make_genesis_header();
    let child = make_child_header(&genesis, addrs[0], 1);
    assert!(engine.verify_header(&child, &genesis).is_ok());
}

#[test]
fn test_verify_header_wrong_number() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs.clone(), poa::PoaConfig::default());

    let genesis = make_genesis_header();
    let mut child = make_child_header(&genesis, addrs[0], 1);
    child.number = 5; // Wrong number
    assert!(engine.verify_header(&child, &genesis).is_err());
}

#[test]
fn test_verify_header_wrong_parent_hash() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs.clone(), poa::PoaConfig::default());

    let genesis = make_genesis_header();
    let mut child = make_child_header(&genesis, addrs[0], 1);
    child.parent_hash = B256::ZERO; // Wrong parent hash
    assert!(engine.verify_header(&child, &genesis).is_err());
}

#[test]
fn test_verify_header_wrong_timestamp() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs.clone(), poa::PoaConfig::default());

    let genesis = make_genesis_header();
    let mut child = make_child_header(&genesis, addrs[0], 1);
    child.timestamp = 0; // Timestamp before parent
    assert!(engine.verify_header(&child, &genesis).is_err());
}

#[test]
fn test_verify_header_wrong_difficulty() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs.clone(), poa::PoaConfig::default());

    let genesis = make_genesis_header();
    let mut child = make_child_header(&genesis, addrs[0], 1);
    child.difficulty = U256::from(2); // Should be 1 for PoA
    assert!(engine.verify_header(&child, &genesis).is_err());
}

#[test]
fn test_verify_header_wrong_game_tick() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs.clone(), poa::PoaConfig::default());

    let genesis = make_genesis_header();
    let mut child = make_child_header(&genesis, addrs[0], 1);
    child.game_tick = 5; // Wrong tick
    assert!(engine.verify_header(&child, &genesis).is_err());
}

#[test]
fn test_advance_tick() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs, poa::PoaConfig::default());
    assert_eq!(engine.current_tick(), 0);
    assert_eq!(engine.advance_tick(), 1);
    assert_eq!(engine.advance_tick(), 2);
    assert_eq!(engine.current_tick(), 2);
}

#[test]
fn test_prepare_and_seal_block() {
    let (keypairs, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_with_keypair(
        keypairs[0].clone(),
        addrs,
        poa::PoaConfig::default(),
    );

    let genesis = make_genesis_header();
    let header = engine.prepare_header(&genesis).unwrap();
    assert_eq!(header.number, 1);
    assert_eq!(header.difficulty, U256::from(1));

    let mut block = Block::new(header, vec![]);
    assert!(engine.seal_block(&mut block).is_ok());
    assert_eq!(block.header.extra_data.len(), 65); // R(32) + S(32) + V(1)
}

#[test]
fn test_seal_and_recover_signer() {
    let (keypairs, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_with_keypair(
        keypairs[0].clone(),
        addrs.clone(),
        poa::PoaConfig::default(),
    );

    let genesis = make_genesis_header();
    let header = engine.prepare_header(&genesis).unwrap();
    let mut block = Block::new(header, vec![]);
    engine.seal_block(&mut block).unwrap();

    let recovered = poa::PoaConsensus::recover_block_signer(&block).unwrap();
    assert_eq!(recovered, addrs[0]);
}

#[test]
fn test_prepare_header_readonly_fails() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs, poa::PoaConfig::default());
    let genesis = make_genesis_header();
    assert!(engine.prepare_header(&genesis).is_err());
}

#[test]
fn test_validators_list() {
    let (_, addrs) = test_validators();
    let engine = poa::PoaConsensus::new_readonly(addrs.clone(), poa::PoaConfig::default());
    assert_eq!(engine.validators(), addrs);
}
