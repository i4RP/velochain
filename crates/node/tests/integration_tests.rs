//! Cross-crate integration tests for VeloChain.
//!
//! Tests the full chain pipeline: storage → state → consensus → game engine → EVM → chain.
//! These tests verify that all crates work together correctly end-to-end.

use std::sync::Arc;
use tempfile::tempdir;

use alloy_primitives::{Address, B256, U256};
use velochain_consensus::poa::{PoaConfig, PoaConsensus};
use velochain_consensus::ConsensusEngine;
use velochain_evm::EvmExecutor;
use velochain_game_engine::GameWorld;
use velochain_node::{Chain, NodeMetrics};
use velochain_primitives::transaction::{GameAction, Transaction};
use velochain_primitives::{Account, Block, BlockHeader, Genesis, Keypair};
use velochain_state::WorldState;
use velochain_storage::Database;
use velochain_txpool::TransactionPool;

/// Helper: create a test chain with a single validator, genesis initialized.
fn setup_test_chain() -> (Arc<Chain>, Keypair, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let db = Arc::new(Database::open(&dir.path().join("db")).unwrap());

    let validator_kp = Keypair::random();
    let validator_addr = validator_kp.address();

    let genesis = Genesis::devnet(validator_addr);
    let game_world = Arc::new(GameWorld::new(genesis.config.world.seed));
    let game_state_root = game_world.state_root();

    let state = Arc::new(WorldState::new(db.clone()));

    // Allocate genesis balance
    for (address, alloc) in &genesis.alloc {
        let account = Account::with_balance(alloc.balance);
        state.put_account(address, &account).unwrap();
    }
    state.commit().unwrap();

    let txpool = Arc::new(TransactionPool::new());
    let poa_config = PoaConfig {
        period: genesis.config.consensus.period,
        epoch: genesis.config.consensus.epoch,
        chain_id: genesis.config.chain_id,
    };
    let consensus = Arc::new(PoaConsensus::new_with_keypair(
        validator_kp.clone(),
        genesis.config.consensus.validators.clone(),
        poa_config,
    ));

    let chain = Arc::new(Chain::new(
        db,
        state,
        game_world,
        txpool,
        consensus,
        genesis.config.chain_id,
    ));

    // Initialize genesis
    let genesis_header = BlockHeader::genesis(game_state_root);
    chain.init_genesis(genesis_header).unwrap();

    (chain, validator_kp, dir)
}

/// Helper: produce and apply a single block on the chain.
fn produce_block(chain: &Arc<Chain>) -> Block {
    let parent = chain.head().unwrap();
    let mut header = chain.consensus().prepare_header(&parent).unwrap();

    let pending_txs = chain.txpool().get_pending(1000);
    let mut selected = Vec::new();
    let mut gas_used: u64 = 0;
    for tx in pending_txs {
        let tx_gas = if tx.is_game_action() {
            21_000
        } else {
            tx.transaction.gas_limit
        };
        if gas_used + tx_gas > header.gas_limit {
            break;
        }
        gas_used += tx_gas;
        selected.push(tx);
    }

    let mut block = Block::new(header.clone(), selected);
    header.transactions_root = block.compute_transactions_root();
    header.gas_used = gas_used;
    block.header = header;

    chain.consensus().seal_block(&mut block).unwrap();
    chain.apply_block(&block).unwrap();
    block
}

// ============================================================
// 15.1: Cross-crate integration tests
// ============================================================

#[test]
fn test_genesis_initialization_full_pipeline() {
    let (chain, _kp, _dir) = setup_test_chain();

    // Genesis block should be at number 0
    assert_eq!(chain.block_number(), 0);

    // Chain head should exist
    let head = chain.head().unwrap();
    assert_eq!(head.number, 0);
    assert_eq!(head.game_tick, 0);

    // Genesis block should be retrievable
    let block = chain.get_block_by_number(0).unwrap().unwrap();
    assert_eq!(block.number(), 0);
    assert!(block.body.transactions.is_empty());
}

#[test]
fn test_block_production_with_game_actions() {
    let (chain, kp, _dir) = setup_test_chain();

    // Create and submit a game action transaction
    let move_tx = Transaction::new_game_action(
        chain.chain_id(),
        0, // nonce
        GameAction::Move {
            x: 5000,
            y: 3000,
            z: 64000,
        },
    );
    let signed = move_tx.sign(&kp).unwrap();
    chain.txpool().add_transaction(signed).unwrap();
    assert_eq!(chain.txpool().pending_count(), 1);

    // Produce block
    let block = produce_block(&chain);
    assert_eq!(block.number(), 1);
    assert_eq!(block.tx_count(), 1);
    assert_eq!(chain.block_number(), 1);

    // Transaction should be removed from pool
    assert_eq!(chain.txpool().pending_count(), 0);

    // Game world should have advanced one tick
    assert!(chain.game_world().current_tick() > 0);

    // Player should have spawned
    assert!(chain.game_world().player_count() > 0);
}

#[test]
fn test_multiple_blocks_sequential_production() {
    let (chain, _kp, _dir) = setup_test_chain();

    // Produce 5 empty blocks
    for i in 1..=5 {
        let block = produce_block(&chain);
        assert_eq!(block.number(), i);
    }

    assert_eq!(chain.block_number(), 5);

    // All blocks should be retrievable
    for i in 0..=5 {
        let block = chain.get_block_by_number(i).unwrap();
        assert!(block.is_some(), "Block {} should exist", i);
    }
}

#[test]
fn test_chain_state_persistence_across_blocks() {
    let (chain, kp, _dir) = setup_test_chain();
    let validator_addr = kp.address();

    // Validator should have genesis balance
    let balance = chain.state().get_balance(&validator_addr).unwrap();
    assert!(
        balance > U256::ZERO,
        "Validator should have genesis balance"
    );

    // Submit a game action and produce block
    let tx = Transaction::new_game_action(
        chain.chain_id(),
        0,
        GameAction::Move {
            x: 1000,
            y: 2000,
            z: 64000,
        },
    );
    let signed = tx.sign(&kp).unwrap();
    chain.txpool().add_transaction(signed).unwrap();
    produce_block(&chain);

    // Balance should have decreased (gas cost deducted)
    let new_balance = chain.state().get_balance(&validator_addr).unwrap();
    assert!(
        new_balance <= balance,
        "Balance should decrease after gas deduction"
    );

    // Nonce should have incremented
    let nonce = chain.state().get_nonce(&validator_addr).unwrap();
    assert_eq!(nonce, 1, "Nonce should be 1 after one transaction");
}

#[test]
fn test_transaction_receipt_storage_and_retrieval() {
    let (chain, kp, _dir) = setup_test_chain();

    let tx = Transaction::new_game_action(
        chain.chain_id(),
        0,
        GameAction::Chat {
            message: "Hello VeloChain!".to_string(),
        },
    );
    let signed = tx.sign(&kp).unwrap();
    let tx_hash = signed.hash;
    chain.txpool().add_transaction(signed).unwrap();

    produce_block(&chain);

    // Receipt should exist
    let receipt = chain.get_receipt(&tx_hash).unwrap();
    assert!(receipt.is_some(), "Receipt should exist for included tx");
    let receipt = receipt.unwrap();
    assert_eq!(receipt.block_number, 1);
    assert_eq!(receipt.tx_hash, tx_hash);
}

#[test]
fn test_game_world_state_root_changes_per_block() {
    let (chain, _kp, _dir) = setup_test_chain();

    let root_before = chain.game_world().state_root();

    // Produce a block (game tick advances, NPC AI runs)
    produce_block(&chain);

    let root_after = chain.game_world().state_root();

    // State root should change after a game tick
    assert_ne!(
        root_before, root_after,
        "Game state root should change after a tick"
    );
}

#[test]
fn test_consensus_block_seal_verification() {
    let (chain, _kp, _dir) = setup_test_chain();

    let block = produce_block(&chain);

    // Block should be sealed (extra_data = 65 bytes signature)
    assert_eq!(
        block.header.extra_data.len(),
        65,
        "Sealed block should have 65-byte signature"
    );

    // Verify the seal
    let signer = PoaConsensus::recover_block_signer(&block).unwrap();
    assert_eq!(
        signer, block.header.beneficiary,
        "Signer should match beneficiary"
    );

    // Signer should be in the validator set
    let validators = chain.consensus().validators();
    assert!(validators.contains(&signer), "Signer should be a validator");
}

#[test]
fn test_evm_executor_integration_with_world_state() {
    let dir = tempdir().unwrap();
    let db = Arc::new(Database::open(&dir.path().join("db")).unwrap());
    let state = Arc::new(WorldState::new(db));

    let address = Address::repeat_byte(0x42);
    let initial_balance = U256::from(1_000_000_000_000_000_000u128); // 1 ETH

    // Set up account in world state
    let account = Account::with_balance(initial_balance);
    state.put_account(&address, &account).unwrap();
    state.commit().unwrap();

    // Load into EVM
    let mut evm = EvmExecutor::new(27181);
    evm.load_account(address, &state);

    // Verify balance in EVM
    assert_eq!(evm.get_balance(address), initial_balance);
}

#[test]
fn test_txpool_to_block_inclusion_flow() {
    let (chain, kp, _dir) = setup_test_chain();

    // Submit multiple game actions
    for i in 0..5u64 {
        let tx = Transaction::new_game_action(
            chain.chain_id(),
            i,
            GameAction::Move {
                x: (i as i64 * 1000),
                y: 0,
                z: 64000,
            },
        );
        let signed = tx.sign(&kp).unwrap();
        chain.txpool().add_transaction(signed).unwrap();
    }
    assert_eq!(chain.txpool().pending_count(), 5);

    // Produce one block — should include all 5 transactions
    let block = produce_block(&chain);
    assert_eq!(block.tx_count(), 5);
    assert_eq!(chain.txpool().pending_count(), 0);
}

#[test]
fn test_snapshot_export_import_roundtrip() {
    let (chain, kp, dir) = setup_test_chain();

    // Produce some blocks with transactions
    let tx = Transaction::new_game_action(
        chain.chain_id(),
        0,
        GameAction::Move {
            x: 1000,
            y: 2000,
            z: 64000,
        },
    );
    let signed = tx.sign(&kp).unwrap();
    chain.txpool().add_transaction(signed).unwrap();
    produce_block(&chain);
    produce_block(&chain);

    assert_eq!(chain.block_number(), 2);

    // Export snapshot
    let snap_path = dir.path().join("test_snapshot.bin");
    let meta = velochain_node::export_snapshot(
        chain.db(),
        chain.game_world(),
        chain.chain_id(),
        &snap_path,
    )
    .unwrap();
    assert_eq!(meta.block_number, 2);
    assert_eq!(meta.block_count, 3); // genesis + 2 blocks

    // Read snapshot metadata
    let read_meta = velochain_node::read_snapshot_meta(&snap_path).unwrap();
    assert_eq!(read_meta.block_number, meta.block_number);
    assert_eq!(read_meta.chain_id, meta.chain_id);
}

#[test]
fn test_metrics_recording() {
    let registry = prometheus::Registry::new();
    let metrics = NodeMetrics::new(&registry).unwrap();

    metrics.record_block(1, 5);
    metrics.record_block(2, 3);
    metrics.record_game_tick(0.015);
    metrics.set_txpool_size(42);
    metrics.set_peer_count(7);

    // Encode to Prometheus text format and verify
    let text = metrics.encode(&registry);
    assert!(
        text.contains("velochain_blocks_total 2"),
        "blocks_total should be 2"
    );
    assert!(
        text.contains("velochain_chain_height 2"),
        "chain_height should be 2"
    );
    assert!(
        text.contains("velochain_transactions_total 8"),
        "transactions_total should be 8"
    );
    assert!(
        text.contains("velochain_txpool_size 42"),
        "txpool_size should be 42"
    );
    assert!(
        text.contains("velochain_peer_count 7"),
        "peer_count should be 7"
    );
}

#[test]
fn test_player_spawn_and_movement_via_game_actions() {
    let (chain, kp, _dir) = setup_test_chain();

    // First move → spawns the player
    let tx1 = Transaction::new_game_action(
        chain.chain_id(),
        0,
        GameAction::Move {
            x: 10000,
            y: 20000,
            z: 64000,
        },
    );
    chain
        .txpool()
        .add_transaction(tx1.sign(&kp).unwrap())
        .unwrap();
    produce_block(&chain);

    assert_eq!(chain.game_world().player_count(), 1);

    // Second move → updates position
    let tx2 = Transaction::new_game_action(
        chain.chain_id(),
        1,
        GameAction::Move {
            x: 15000,
            y: 25000,
            z: 64000,
        },
    );
    chain
        .txpool()
        .add_transaction(tx2.sign(&kp).unwrap())
        .unwrap();
    produce_block(&chain);

    // Player should still exist
    assert_eq!(chain.game_world().player_count(), 1);

    // Verify player info via API
    let players = chain.game_world().get_all_players();
    assert_eq!(players.len(), 1);
    // Position should be the last move (15.0, 25.0, 64.0)
    assert!((players[0].position[0] - 15.0).abs() < 0.01);
    assert!((players[0].position[1] - 25.0).abs() < 0.01);
}

#[test]
fn test_multiple_players_in_same_world() {
    let (chain, kp1, _dir) = setup_test_chain();
    let kp2 = Keypair::random();

    // Give player 2 some balance for gas
    let account2 = Account::with_balance(U256::from(1_000_000_000_000_000_000u128));
    chain
        .state()
        .put_account(&kp2.address(), &account2)
        .unwrap();
    chain.state().commit().unwrap();

    // Player 1 moves
    let tx1 = Transaction::new_game_action(
        chain.chain_id(),
        0,
        GameAction::Move {
            x: 1000,
            y: 0,
            z: 64000,
        },
    );
    chain
        .txpool()
        .add_transaction(tx1.sign(&kp1).unwrap())
        .unwrap();

    // Player 2 moves
    let tx2 = Transaction::new_game_action(
        chain.chain_id(),
        0,
        GameAction::Move {
            x: 5000,
            y: 5000,
            z: 64000,
        },
    );
    chain
        .txpool()
        .add_transaction(tx2.sign(&kp2).unwrap())
        .unwrap();

    produce_block(&chain);

    assert_eq!(chain.game_world().player_count(), 2);
}

#[test]
fn test_chain_head_restore_from_db() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("db");
    let chain_id: u64;
    let block_number: u64;

    // Phase 1: Create chain, produce blocks, drop chain
    {
        let db = Arc::new(Database::open(&db_path).unwrap());
        let kp = Keypair::random();
        let genesis = Genesis::devnet(kp.address());
        chain_id = genesis.config.chain_id;
        let game_world = Arc::new(GameWorld::new(genesis.config.world.seed));
        let game_state_root = game_world.state_root();
        let state = Arc::new(WorldState::new(db.clone()));
        for (addr, alloc) in &genesis.alloc {
            state
                .put_account(addr, &Account::with_balance(alloc.balance))
                .unwrap();
        }
        state.commit().unwrap();
        let txpool = Arc::new(TransactionPool::new());
        let poa_config = PoaConfig {
            period: genesis.config.consensus.period,
            epoch: genesis.config.consensus.epoch,
            chain_id: genesis.config.chain_id,
        };
        let consensus = Arc::new(PoaConsensus::new_with_keypair(
            kp,
            genesis.config.consensus.validators.clone(),
            poa_config,
        ));
        let chain = Arc::new(Chain::new(
            db, state, game_world, txpool, consensus, chain_id,
        ));
        chain
            .init_genesis(BlockHeader::genesis(game_state_root))
            .unwrap();

        produce_block(&chain);
        produce_block(&chain);
        block_number = chain.block_number();
        assert_eq!(block_number, 2);
    }

    // Phase 2: Re-open DB, restore chain head
    {
        let db = Arc::new(Database::open(&db_path).unwrap());
        let kp = Keypair::random();
        let game_world = Arc::new(GameWorld::new(42));
        let state = Arc::new(WorldState::new(db.clone()));
        let txpool = Arc::new(TransactionPool::new());
        let poa_config = PoaConfig::default();
        let consensus = Arc::new(PoaConsensus::new_with_keypair(kp, vec![], poa_config));
        let chain = Arc::new(Chain::new(
            db, state, game_world, txpool, consensus, chain_id,
        ));
        chain.restore_head().unwrap();

        assert_eq!(chain.block_number(), block_number);
    }
}

#[test]
fn test_nonce_enforcement_rejects_duplicate() {
    let (chain, kp, _dir) = setup_test_chain();

    // Submit tx with nonce 0
    let tx0 = Transaction::new_game_action(chain.chain_id(), 0, GameAction::Respawn);
    chain
        .txpool()
        .add_transaction(tx0.sign(&kp).unwrap())
        .unwrap();

    // Try to submit another tx with the same nonce 0 → should fail
    let tx0_dup = Transaction::new_game_action(
        chain.chain_id(),
        0,
        GameAction::Chat {
            message: "dup".to_string(),
        },
    );
    let result = chain.txpool().add_transaction(tx0_dup.sign(&kp).unwrap());
    assert!(
        result.is_err(),
        "Duplicate nonce should be rejected by txpool"
    );
}

#[test]
fn test_genesis_validation_roundtrip() {
    let kp = Keypair::random();

    // Devnet genesis should validate
    let devnet = Genesis::devnet(kp.address());
    assert!(devnet.validate().is_ok());

    // Testnet genesis should validate
    let testnet = Genesis::testnet(vec![kp.address()]);
    assert!(testnet.validate().is_ok());

    // Serialize/deserialize roundtrip
    let json = serde_json::to_string(&devnet).unwrap();
    let restored: Genesis = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.config.chain_id, devnet.config.chain_id);
    assert_eq!(restored.gas_limit, devnet.gas_limit);
}

#[test]
fn test_game_world_tick_advances_state() {
    // Verify that ticking the game world changes state deterministically.
    let world = GameWorld::new(42);

    let root_before = world.state_root();
    assert_ne!(
        root_before,
        B256::ZERO,
        "Initial state root should be non-zero"
    );

    // Tick without actions — NPC AI runs, events fire
    world.tick(&[]).unwrap();
    assert_eq!(world.current_tick(), 1);

    let root_after = world.state_root();
    // State root should change because NPC AI and events run each tick
    assert_ne!(
        root_before, root_after,
        "State root should change after a tick"
    );

    // Entity count should be > 0 (NPCs exist from initialization)
    assert!(world.entity_count() > 0, "Should have NPC entities");
}

// ============================================================
// 15.2: End-to-end node startup & block production test
// ============================================================

#[test]
fn test_end_to_end_node_init_and_produce_10_blocks() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("db");

    let kp = Keypair::random();
    let validator_addr = kp.address();
    let genesis = Genesis::devnet(validator_addr);
    genesis.validate().unwrap();

    // Step 1: Open DB
    let db = Arc::new(Database::open(&db_path).unwrap());

    // Step 2: Init world state with genesis allocations
    let state = Arc::new(WorldState::new(db.clone()));
    for (address, alloc) in &genesis.alloc {
        state
            .put_account(address, &Account::with_balance(alloc.balance))
            .unwrap();
    }
    state.commit().unwrap();

    // Step 3: Create game world
    let game_world = Arc::new(GameWorld::new(genesis.config.world.seed));
    let game_state_root = game_world.state_root();

    // Step 4: Create consensus engine
    let poa_config = PoaConfig {
        period: genesis.config.consensus.period,
        epoch: genesis.config.consensus.epoch,
        chain_id: genesis.config.chain_id,
    };
    let consensus = Arc::new(PoaConsensus::new_with_keypair(
        kp.clone(),
        genesis.config.consensus.validators.clone(),
        poa_config,
    ));
    assert!(consensus.is_validator());

    // Step 5: Create chain, init genesis
    let txpool = Arc::new(TransactionPool::new());
    let chain = Arc::new(Chain::new(
        db.clone(),
        state.clone(),
        game_world.clone(),
        txpool.clone(),
        consensus.clone(),
        genesis.config.chain_id,
    ));
    chain
        .init_genesis(BlockHeader::genesis(game_state_root))
        .unwrap();
    assert_eq!(chain.block_number(), 0);

    // Step 6: Produce 10 blocks, some with game actions
    for i in 1..=10u64 {
        // Every other block, include a game action
        if i % 2 == 0 {
            let tx = Transaction::new_game_action(
                genesis.config.chain_id,
                (i / 2) - 1, // nonce
                GameAction::Move {
                    x: (i as i64 * 1000),
                    y: 0,
                    z: 64000,
                },
            );
            txpool.add_transaction(tx.sign(&kp).unwrap()).unwrap();
        }
        produce_block(&chain);
    }

    assert_eq!(chain.block_number(), 10);

    // Verify chain integrity: each block's parent_hash should match previous block's hash
    for i in 1..=10u64 {
        let block = chain.get_block_by_number(i).unwrap().unwrap();
        let parent = chain.get_block_by_number(i - 1).unwrap().unwrap();
        assert_eq!(
            block.header.parent_hash,
            parent.hash(),
            "Block {} parent_hash should match block {} hash",
            i,
            i - 1
        );
        assert_eq!(block.header.game_tick, i);
    }

    // Verify game state
    assert!(game_world.player_count() > 0);
    assert!(game_world.current_tick() >= 10);
}

#[test]
fn test_end_to_end_with_respawn() {
    let (chain, kp, _dir) = setup_test_chain();

    // Spawn player
    let tx1 = Transaction::new_game_action(
        chain.chain_id(),
        0,
        GameAction::Move {
            x: 1000,
            y: 1000,
            z: 64000,
        },
    );
    chain
        .txpool()
        .add_transaction(tx1.sign(&kp).unwrap())
        .unwrap();
    produce_block(&chain);

    // Respawn player
    let tx2 = Transaction::new_game_action(chain.chain_id(), 1, GameAction::Respawn);
    chain
        .txpool()
        .add_transaction(tx2.sign(&kp).unwrap())
        .unwrap();
    produce_block(&chain);

    // Player should still exist
    assert_eq!(chain.game_world().player_count(), 1);

    // Player should be at spawn point (0, 0, 64)
    let players = chain.game_world().get_all_players();
    assert_eq!(players.len(), 1);
    assert!((players[0].position[0]).abs() < 0.01);
    assert!((players[0].position[1]).abs() < 0.01);
}

#[test]
fn test_end_to_end_game_state_persistence() {
    let (chain, _kp, _dir) = setup_test_chain();

    // Produce several blocks to advance game state (NPC AI runs, events fire)
    for _ in 0..5 {
        produce_block(&chain);
    }

    // Serialize game world
    let serialized = chain.game_world().serialize_state().unwrap();
    assert!(!serialized.is_empty());

    // Store in DB (simulating shutdown persistence)
    chain.db().put_game_state(b"world", &serialized).unwrap();

    // Restore from DB
    let restored_data = chain.db().get_game_state(b"world").unwrap().unwrap();
    assert_eq!(serialized, restored_data);

    // Deserialize into a new GameWorld
    let restored_world = GameWorld::from_state(&restored_data, chain.game_world().seed()).unwrap();
    assert!(restored_world.entity_count() > 0);
}

// ============================================================
// 15.3: Configuration validation & defaults
// ============================================================

#[test]
fn test_config_file_defaults_are_valid() {
    let config = velochain_node::ConfigFile::default();

    // All default sections should parse correctly
    assert_eq!(config.rpc.addr, "127.0.0.1:8545");
    assert_eq!(config.network.listen_addr, "/ip4/0.0.0.0/tcp/30303");
    assert_eq!(config.network.max_peers, 50);
    assert!(!config.validator.enabled);
    assert_eq!(config.logging.level, "info");
    assert!(config.logging.timestamps);
    assert!(!config.logging.json);

    // RPC socket addr should parse
    let rpc_addr = config.rpc_socket_addr();
    assert!(rpc_addr.is_ok());

    // Log filter string should be well-formed
    let filter = config.log_filter_string();
    assert_eq!(filter, "info");
}

#[test]
fn test_config_file_toml_roundtrip() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("velochain.toml");

    // Write default config
    velochain_node::ConfigFile::write_default(&config_path).unwrap();

    // Read it back
    let loaded = velochain_node::ConfigFile::load(&config_path).unwrap();
    assert_eq!(loaded.rpc.addr, "127.0.0.1:8545");
    assert_eq!(loaded.network.max_peers, 50);
}

#[test]
fn test_config_env_overrides() {
    let mut config = velochain_node::ConfigFile::default();

    // Set environment variables
    std::env::set_var("VELOCHAIN_RPC_ADDR", "0.0.0.0:9999");
    std::env::set_var("VELOCHAIN_NETWORK_MAX_PEERS", "100");
    std::env::set_var("VELOCHAIN_VALIDATOR_ENABLED", "true");
    std::env::set_var("VELOCHAIN_LOG_LEVEL", "debug");

    config.apply_env_overrides();

    assert_eq!(config.rpc.addr, "0.0.0.0:9999");
    assert_eq!(config.network.max_peers, 100);
    assert!(config.validator.enabled);
    assert_eq!(config.logging.level, "debug");

    // Clean up
    std::env::remove_var("VELOCHAIN_RPC_ADDR");
    std::env::remove_var("VELOCHAIN_NETWORK_MAX_PEERS");
    std::env::remove_var("VELOCHAIN_VALIDATOR_ENABLED");
    std::env::remove_var("VELOCHAIN_LOG_LEVEL");
}

#[test]
fn test_node_config_defaults() {
    let config = velochain_node::NodeConfig::default();

    assert_eq!(config.rpc_addr.to_string(), "127.0.0.1:8545");
    assert_eq!(config.p2p_addr, "/ip4/0.0.0.0/tcp/30303");
    assert_eq!(config.max_peers, 50);
    assert!(!config.is_validator);
    assert!(config.validator_key.is_none());
    assert!(config.boot_nodes.is_empty());
    assert_eq!(config.log_level, "info");
}

#[test]
fn test_genesis_invalid_configs_are_rejected() {
    // Zero chain ID
    let mut g = Genesis::default();
    g.config.chain_id = 0;
    assert!(g.validate().is_err());

    // Zero block time
    let mut g = Genesis::default();
    g.config.block_time = 0;
    assert!(g.validate().is_err());

    // Zero tick interval
    let mut g = Genesis::default();
    g.config.tick_interval_ms = 0;
    assert!(g.validate().is_err());

    // Invalid consensus engine
    let mut g = Genesis::default();
    g.config.consensus.engine = "invalid".to_string();
    assert!(g.validate().is_err());

    // Zero gas limit
    let mut g = Genesis::default();
    g.gas_limit = 0;
    assert!(g.validate().is_err());

    // Zero world size
    let mut g = Genesis::default();
    g.config.world.size_chunks = [0, 0];
    assert!(g.validate().is_err());

    // Invalid precompile range
    let mut g = Genesis::default();
    g.config.evm.precompile_range = [10, 5];
    assert!(g.validate().is_err());
}

#[test]
fn test_poa_config_defaults() {
    let config = PoaConfig::default();
    assert_eq!(config.period, 1);
    assert_eq!(config.epoch, 30000);
    assert_eq!(config.chain_id, velochain_primitives::DEFAULT_CHAIN_ID);
}

#[test]
fn test_readonly_node_cannot_produce_blocks() {
    let kp = Keypair::random();
    let poa_config = PoaConfig::default();

    // Create a readonly consensus (no validator key)
    let consensus = PoaConsensus::new_readonly(vec![kp.address()], poa_config);

    assert!(!consensus.is_validator());

    // prepare_header should fail for non-validator
    let parent = BlockHeader::genesis(B256::ZERO);
    let result = consensus.prepare_header(&parent);
    assert!(result.is_err());
}

#[test]
fn test_shutdown_controller() {
    let shutdown = velochain_node::ShutdownController::new();

    assert!(!shutdown.is_shutdown());

    shutdown.shutdown();

    assert!(shutdown.is_shutdown());

    // Double shutdown should be idempotent
    shutdown.shutdown();
    assert!(shutdown.is_shutdown());
}

#[test]
fn test_logging_config_filter_string() {
    let mut config = velochain_node::ConfigFile::default();
    config.logging.level = "warn".to_string();
    config.logging.modules = vec![
        "velochain_rpc=debug".to_string(),
        "velochain_consensus=trace".to_string(),
    ];

    let filter = config.log_filter_string();
    assert_eq!(filter, "warn,velochain_rpc=debug,velochain_consensus=trace");
}

// ============================================================
// 15.4: Storage & cache integration
// ============================================================

#[test]
fn test_storage_block_roundtrip() {
    let dir = tempdir().unwrap();
    let db = Database::open(&dir.path().join("db")).unwrap();

    let mut header = BlockHeader::genesis(B256::ZERO);
    header.number = 42;
    header.timestamp = 1000;
    header.game_tick = 42;
    let block = Block::new(header, vec![]);
    let hash = block.hash();

    db.put_block(&block).unwrap();
    db.put_latest_block_number(42).unwrap();

    // Retrieve by hash
    let h = db.get_header(hash.as_ref()).unwrap().unwrap();
    assert_eq!(h.number, 42);

    // Retrieve by number
    let hash_by_num = db.get_block_hash_by_number(42).unwrap().unwrap();
    assert_eq!(&hash_by_num, hash.as_ref() as &[u8; 32]);

    // Latest block number
    assert_eq!(db.get_latest_block_number().unwrap(), Some(42));
}

#[test]
fn test_world_state_balance_operations() {
    let dir = tempdir().unwrap();
    let db = Arc::new(Database::open(&dir.path().join("db")).unwrap());
    let state = WorldState::new(db);

    let addr = Address::repeat_byte(0x11);
    let amount = U256::from(1_000_000u64);

    // Add balance
    state.add_balance(&addr, amount).unwrap();
    assert_eq!(state.get_balance(&addr).unwrap(), amount);

    // Sub balance
    state.sub_balance(&addr, U256::from(500_000u64)).unwrap();
    assert_eq!(state.get_balance(&addr).unwrap(), U256::from(500_000u64));

    // Sub more than balance → error
    let result = state.sub_balance(&addr, U256::from(999_999u64));
    assert!(result.is_err());

    // Nonce operations
    assert_eq!(state.get_nonce(&addr).unwrap(), 0);
    state.increment_nonce(&addr).unwrap();
    assert_eq!(state.get_nonce(&addr).unwrap(), 1);

    // Commit should produce a state root
    let root = state.commit().unwrap();
    assert_ne!(root, B256::ZERO);
}

#[test]
fn test_keypair_operations() {
    let dir = tempdir().unwrap();

    // Generate random keypair
    let kp = Keypair::random();
    assert_ne!(kp.address(), Address::ZERO);

    // Save and load keystore
    let keystore_path = dir.path().join("test.key.json");
    kp.save_keystore(&keystore_path, "testpass").unwrap();
    let loaded = Keypair::load_keystore(&keystore_path, "testpass").unwrap();
    assert_eq!(loaded.address(), kp.address());

    // Sign and recover
    let msg = B256::from_slice(&[0xABu8; 32]);
    let (sig, recid) = kp.sign_hash(&msg).unwrap();
    let sig_bytes = sig.to_bytes();
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&sig_bytes[..32]);
    s.copy_from_slice(&sig_bytes[32..]);
    let recovered =
        velochain_primitives::recover_signer(&msg, recid.to_byte() as u64, &r, &s).unwrap();
    assert_eq!(recovered, kp.address());

    // From hex roundtrip
    let hex_key = kp.secret_key_hex();
    let from_hex = Keypair::from_hex(&hex_key).unwrap();
    assert_eq!(from_hex.address(), kp.address());
}

#[test]
fn test_fork_config_activation_checks() {
    use velochain_primitives::genesis::ForkConfig;

    let forks = ForkConfig {
        eip1559_block: Some(100),
        code_size_increase_block: Some(500),
        game_precompiles_block: None,
    };

    assert!(!forks.is_eip1559_active(99));
    assert!(forks.is_eip1559_active(100));
    assert!(forks.is_eip1559_active(200));

    assert!(!forks.is_code_size_increase_active(499));
    assert!(forks.is_code_size_increase_active(500));

    assert!(!forks.is_game_precompiles_active(u64::MAX));
}
