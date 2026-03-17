//! Integration tests for VeloChain node.
//!
//! Tests the full lifecycle: chain initialization, transaction submission,
//! block production, receipt generation, and game state updates.

use alloy_primitives::{Address, B256, U256};
use std::sync::Arc;
use velochain_consensus::poa::{PoaConfig, PoaConsensus};
use velochain_game_engine::GameWorld;
use velochain_node::Chain;
use velochain_primitives::{
    genesis::{EvmConfig, ForkConfig},
    transaction::{GameAction, Transaction},
    BlockHeader, Genesis, Keypair,
};
use velochain_state::WorldState;
use velochain_storage::Database;
use velochain_txpool::TransactionPool;

/// Helper to create a temporary database for testing.
fn temp_db() -> Arc<Database> {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().to_path_buf();
    std::mem::forget(dir); // Prevent early cleanup while DB is open
    Arc::new(Database::open(&path).expect("open db"))
}

/// Helper to create a test chain with a single validator.
fn setup_test_chain() -> (Arc<Chain>, Keypair) {
    let db = temp_db();
    let state = Arc::new(WorldState::new(db.clone()));
    let game_world = Arc::new(GameWorld::new(42));
    let txpool = Arc::new(TransactionPool::new());

    let validator = Keypair::random();
    let poa_config = PoaConfig {
        period: 1,
        epoch: 30000,
        chain_id: 27181,
    };
    let consensus = Arc::new(PoaConsensus::new_with_keypair(
        validator.clone(),
        vec![validator.address()],
        poa_config,
    ));

    let chain_id = 27181u64;
    let chain = Arc::new(Chain::new(
        db, state, game_world, txpool, consensus, chain_id,
    ));

    // Initialize genesis
    let genesis_header = BlockHeader::genesis(B256::ZERO);
    chain.init_genesis(genesis_header).expect("init genesis");

    (chain, validator)
}

#[test]
fn test_chain_initialization() {
    let (chain, _validator) = setup_test_chain();

    // Chain should be initialized at block 0
    assert_eq!(chain.block_number(), 0);
    assert!(chain.head().is_some());

    // Block 0 should exist
    let block = chain.get_block_by_number(0).expect("get block 0");
    assert!(block.is_some());
    let block = block.unwrap();
    assert_eq!(block.number(), 0);
}

#[test]
fn test_chain_head_restore() {
    let db = temp_db();
    let state = Arc::new(WorldState::new(db.clone()));
    let game_world = Arc::new(GameWorld::new(42));
    let txpool = Arc::new(TransactionPool::new());

    let validator = Keypair::random();
    let validator_addr = validator.address();
    let poa_config = PoaConfig {
        period: 1,
        epoch: 30000,
        chain_id: 27181,
    };
    let consensus = Arc::new(PoaConsensus::new_with_keypair(
        validator,
        vec![validator_addr],
        poa_config,
    ));

    let chain = Chain::new(
        db.clone(),
        state.clone(),
        game_world.clone(),
        txpool.clone(),
        consensus.clone(),
        27181,
    );

    // Initialize genesis
    let genesis_header = BlockHeader::genesis(B256::ZERO);
    chain.init_genesis(genesis_header).expect("init genesis");
    assert_eq!(chain.block_number(), 0);

    // Create a new chain instance pointing to same DB (simulates restart)
    let chain2 = Chain::new(db, state, game_world, txpool, consensus, 27181);
    chain2.restore_head().expect("restore head");
    assert_eq!(chain2.block_number(), 0);
}

#[test]
fn test_txpool_add_and_retrieve() {
    let txpool = TransactionPool::new();
    let keypair = Keypair::random();

    let tx = Transaction::new_game_action(
        27181,
        0,
        GameAction::Move {
            x: 100,
            y: 200,
            z: 0,
        },
    );
    let signed = tx.sign(&keypair).expect("sign tx");
    let hash = signed.hash;

    txpool.add_transaction(signed).expect("add tx");
    assert_eq!(txpool.pending_count(), 1);

    // Retrieve pending transactions
    let pending = txpool.get_pending(10);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].hash, hash);
}

#[test]
fn test_txpool_remove_included() {
    let txpool = TransactionPool::new();
    let keypair = Keypair::random();

    let tx = Transaction::new_game_action(27181, 0, GameAction::Respawn);
    let signed = tx.sign(&keypair).expect("sign tx");
    let hash = signed.hash;

    txpool.add_transaction(signed).expect("add tx");
    assert_eq!(txpool.pending_count(), 1);

    txpool.remove_included(&[hash]);
    assert_eq!(txpool.pending_count(), 0);
}

#[test]
fn test_game_world_tick() {
    let game_world = GameWorld::new(42);

    // Initial state
    assert_eq!(game_world.current_tick(), 0);
    assert!(game_world.entity_count() > 0); // NPCs from initialization

    // Run a tick with no actions
    let root = game_world.tick(&[]).expect("tick");
    assert_eq!(game_world.current_tick(), 1);
    assert!(!root.is_zero());

    // Run a tick with a player move action
    let root2 = game_world
        .tick(&[(
            "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            GameAction::Move {
                x: 1000,
                y: 2000,
                z: 0,
            },
        )])
        .expect("tick with action");
    assert_eq!(game_world.current_tick(), 2);
    // State should change after player action
    assert_ne!(root, root2);
}

#[test]
fn test_game_world_player_spawn() {
    let game_world = GameWorld::new(42);
    let player_addr = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

    // No player initially
    assert!(game_world.get_player_info(player_addr).is_none());

    // Player auto-spawns on first action
    game_world
        .tick(&[(
            player_addr.to_string(),
            GameAction::Move {
                x: 5000,
                y: 10000,
                z: 0,
            },
        )])
        .expect("tick");

    let info = game_world.get_player_info(player_addr);
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.address, player_addr);
    assert!(info.is_alive);
}

#[test]
fn test_game_world_serialization() {
    let game_world = GameWorld::new(42);
    game_world.tick(&[]).expect("tick");

    let data = game_world.serialize_state().expect("serialize");
    assert!(!data.is_empty());

    let restored = GameWorld::from_state(&data, 42).expect("deserialize");
    assert_eq!(restored.entity_count(), game_world.entity_count());
}

#[test]
fn test_evm_executor_balance() {
    use velochain_evm::EvmExecutor;

    let mut evm = EvmExecutor::new(27181);
    let addr = Address::from([0x42; 20]);

    // Set balance
    evm.set_balance(addr, U256::from(1_000_000));
    assert_eq!(evm.get_balance(addr), U256::from(1_000_000));
}

#[test]
fn test_evm_simulate_call() {
    use velochain_evm::EvmExecutor;

    let evm = EvmExecutor::new(27181);
    let from = Address::ZERO;

    // Simple call to non-existent contract should succeed (empty return)
    let result = evm.simulate_call(
        from,
        Some(Address::from([0x01; 20])),
        U256::ZERO,
        vec![],
        100_000,
    );
    assert!(result.is_ok());
}

#[test]
fn test_evm_estimate_gas() {
    use velochain_evm::EvmExecutor;

    let evm = EvmExecutor::new(27181);
    let from = Address::ZERO;

    // Estimate gas for a simple transfer
    let gas = evm.estimate_gas(from, Some(Address::from([0x01; 20])), U256::ZERO, vec![]);
    assert!(gas.is_ok());
    let gas = gas.unwrap();
    assert!(gas >= 21_000); // Minimum gas for a transfer
}

#[test]
fn test_genesis_config_defaults() {
    let genesis = Genesis::default();

    assert_eq!(genesis.config.chain_id, 27181);
    assert_eq!(genesis.gas_limit, 30_000_000);
    assert_eq!(genesis.config.evm.block_gas_limit, 30_000_000);
    assert_eq!(genesis.config.evm.max_code_size, 24576);
    assert!(!genesis.config.evm.eip1559);
    assert_eq!(genesis.config.evm.precompile_range, [1, 9]);
}

#[test]
fn test_genesis_config_serialization() {
    let genesis = Genesis::default();
    let json = serde_json::to_string_pretty(&genesis).expect("serialize");
    let deserialized: Genesis = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.config.chain_id, genesis.config.chain_id);
    assert_eq!(deserialized.gas_limit, genesis.gas_limit);
    assert_eq!(
        deserialized.config.evm.block_gas_limit,
        genesis.config.evm.block_gas_limit
    );
}

#[test]
fn test_fork_config() {
    let fork = ForkConfig {
        eip1559_block: Some(100),
        code_size_increase_block: Some(200),
        game_precompiles_block: None,
    };

    assert!(!fork.is_eip1559_active(99));
    assert!(fork.is_eip1559_active(100));
    assert!(fork.is_eip1559_active(101));

    assert!(!fork.is_code_size_increase_active(199));
    assert!(fork.is_code_size_increase_active(200));

    assert!(!fork.is_game_precompiles_active(1000));
}

#[test]
fn test_evm_config_custom() {
    let evm_config = EvmConfig {
        block_gas_limit: 50_000_000,
        min_gas_price: 1,
        max_code_size: 49152, // 48KB
        eip1559: true,
        precompile_range: [1, 20],
    };

    assert_eq!(evm_config.block_gas_limit, 50_000_000);
    assert_eq!(evm_config.min_gas_price, 1);
    assert!(evm_config.eip1559);
}

#[test]
fn test_keypair_operations() {
    let keypair = Keypair::random();
    let address = keypair.address();

    // Address should be 20 bytes
    assert!(!address.is_zero());

    // Secret key hex should be 64 characters (32 bytes)
    let hex = keypair.secret_key_hex();
    assert_eq!(hex.len(), 64);

    // Should be able to recreate from hex
    let keypair2 = Keypair::from_hex(&hex).expect("from hex");
    assert_eq!(keypair2.address(), address);
}

#[test]
fn test_transaction_signing() {
    let keypair = Keypair::random();
    let chain_id = 27181u64;

    let tx = Transaction::new_game_action(chain_id, 0, GameAction::Respawn);
    let signed = tx.sign(&keypair).expect("sign");

    // Recover sender should return the same address
    let sender = signed.sender().expect("recover sender");
    assert_eq!(sender, keypair.address());
}

#[test]
fn test_world_state_balance_ops() {
    let db = temp_db();
    let state = WorldState::new(db);
    let addr = Address::from([0xAB; 20]);

    // Initial balance should be zero
    let balance = state.get_balance(&addr).expect("get balance");
    assert_eq!(balance, U256::ZERO);

    // Add balance
    state
        .add_balance(&addr, U256::from(1000))
        .expect("add balance");
    let balance = state.get_balance(&addr).expect("get balance");
    assert_eq!(balance, U256::from(1000));

    // Add more balance
    state
        .add_balance(&addr, U256::from(500))
        .expect("add balance");
    let balance = state.get_balance(&addr).expect("get balance");
    assert_eq!(balance, U256::from(1500));

    // Sub balance
    state
        .sub_balance(&addr, U256::from(200))
        .expect("sub balance");
    let balance = state.get_balance(&addr).expect("get balance");
    assert_eq!(balance, U256::from(1300));
}

#[test]
fn test_world_state_nonce_ops() {
    let db = temp_db();
    let state = WorldState::new(db);
    let addr = Address::from([0xCD; 20]);

    // Initial nonce should be zero
    let nonce = state.get_nonce(&addr).expect("get nonce");
    assert_eq!(nonce, 0);

    // Increment nonce
    state.increment_nonce(&addr).expect("increment nonce");
    let nonce = state.get_nonce(&addr).expect("get nonce");
    assert_eq!(nonce, 1);

    state.increment_nonce(&addr).expect("increment nonce");
    let nonce = state.get_nonce(&addr).expect("get nonce");
    assert_eq!(nonce, 2);
}

#[test]
fn test_websocket_event_channel() {
    use velochain_rpc::{new_event_channel, GameEvent};

    let (tx, mut rx) = new_event_channel();

    // Send an event
    tx.send(GameEvent::NewBlock {
        number: 1,
        hash: "0xabcd".to_string(),
        tx_count: 5,
        timestamp: 1000,
    })
    .expect("send event");

    // Receive the event
    let event = rx.try_recv().expect("recv event");
    match event {
        GameEvent::NewBlock {
            number, tx_count, ..
        } => {
            assert_eq!(number, 1);
            assert_eq!(tx_count, 5);
        }
        _ => panic!("unexpected event type"),
    }
}

#[test]
fn test_websocket_game_tick_event() {
    use velochain_rpc::{new_event_channel, GameEvent};

    let (tx, mut rx) = new_event_channel();

    tx.send(GameEvent::GameTick {
        tick: 42,
        entity_count: 100,
        player_count: 5,
        state_root: "0x1234".to_string(),
    })
    .expect("send event");

    let event = rx.try_recv().expect("recv event");
    match event {
        GameEvent::GameTick {
            tick,
            entity_count,
            player_count,
            ..
        } => {
            assert_eq!(tick, 42);
            assert_eq!(entity_count, 100);
            assert_eq!(player_count, 5);
        }
        _ => panic!("unexpected event type"),
    }
}
