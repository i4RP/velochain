//! VeloChain node CLI entry point.

use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use velochain_consensus::poa::{PoaConfig, PoaConsensus};
use velochain_game_engine::GameWorld;
use velochain_network::service::{NetworkConfig, NetworkService};
use velochain_network::{Multiaddr, NetworkEvent};
use velochain_node::{BlockProducer, Chain, ConfigFile, NodeMetrics, ShutdownController};
use velochain_primitives::{BlockHeader, Genesis, Keypair};
use velochain_rpc::{new_event_channel, server::RpcConfig, RpcServer, SessionManager};
use velochain_state::WorldState;
use velochain_storage::Database;
use velochain_txpool::TransactionPool;

#[derive(Parser)]
#[command(name = "velochain")]
#[command(about = "VeloChain - On-chain game world node")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new chain with genesis block.
    Init {
        /// Data directory.
        #[arg(short, long, default_value = "./velochain-data")]
        datadir: PathBuf,
        /// Path to genesis configuration file.
        #[arg(short, long)]
        genesis: Option<PathBuf>,
    },
    /// Run the VeloChain node.
    Run {
        /// Data directory.
        #[arg(short, long, default_value = "./velochain-data")]
        datadir: PathBuf,
        /// RPC server address.
        #[arg(long, default_value = "127.0.0.1:8545")]
        rpc_addr: String,
        /// P2P listen address.
        #[arg(long, default_value = "/ip4/0.0.0.0/tcp/30303")]
        p2p_addr: String,
        /// Run as validator (block producer).
        #[arg(long)]
        validator: bool,
        /// Validator private key (hex). Required if --validator is set.
        #[arg(long, env = "VELOCHAIN_VALIDATOR_KEY")]
        validator_key: Option<String>,
        /// Boot node multiaddresses (comma separated).
        #[arg(long)]
        bootnodes: Option<String>,
    },
    /// Show chain status information.
    Status {
        /// Data directory.
        #[arg(short, long, default_value = "./velochain-data")]
        datadir: PathBuf,
    },
    /// Generate a new validator keypair.
    GenerateKey {
        /// Output keystore file path.
        #[arg(short, long, default_value = "./validator.key.json")]
        output: PathBuf,
        /// Password to encrypt the keystore.
        #[arg(short, long, default_value = "")]
        password: String,
    },
    /// Import a private key into a keystore file.
    ImportKey {
        /// Hex-encoded private key.
        #[arg(long)]
        private_key: String,
        /// Output keystore file path.
        #[arg(short, long, default_value = "./validator.key.json")]
        output: PathBuf,
        /// Password to encrypt the keystore.
        #[arg(short, long, default_value = "")]
        password: String,
    },
    /// Export a private key from a keystore file.
    ExportKey {
        /// Path to keystore file.
        #[arg(short, long)]
        keystore: PathBuf,
        /// Password to decrypt the keystore.
        #[arg(short, long, default_value = "")]
        password: String,
    },
    /// List accounts in the chain state.
    Accounts {
        /// Data directory.
        #[arg(short, long, default_value = "./velochain-data")]
        datadir: PathBuf,
    },
    /// Show peer info.
    Peers {
        /// RPC endpoint to query.
        #[arg(long, default_value = "http://127.0.0.1:8545")]
        rpc: String,
    },
    /// Export chain state to a snapshot file.
    Snapshot {
        /// Data directory.
        #[arg(short, long, default_value = "./velochain-data")]
        datadir: PathBuf,
        /// Output snapshot file path.
        #[arg(short, long, default_value = "./velochain-snapshot.bin")]
        output: PathBuf,
    },
    /// Restore chain state from a snapshot file.
    Restore {
        /// Data directory.
        #[arg(short, long, default_value = "./velochain-data")]
        datadir: PathBuf,
        /// Snapshot file to import.
        #[arg(short, long)]
        snapshot: PathBuf,
    },
    /// Show snapshot file metadata without importing.
    SnapshotInfo {
        /// Path to snapshot file.
        #[arg(short, long)]
        snapshot: PathBuf,
    },
    /// Generate a default configuration file.
    ConfigInit {
        /// Output config file path.
        #[arg(short, long, default_value = "./velochain.toml")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { datadir, genesis } => {
            cmd_init(datadir, genesis).await?;
        }
        Commands::Run {
            datadir,
            rpc_addr,
            p2p_addr,
            validator,
            validator_key,
            bootnodes,
        } => {
            cmd_run(
                datadir,
                rpc_addr,
                p2p_addr,
                validator,
                validator_key,
                bootnodes,
            )
            .await?;
        }
        Commands::Status { datadir } => {
            cmd_status(datadir).await?;
        }
        Commands::GenerateKey { output, password } => {
            cmd_generate_key(output, password)?;
        }
        Commands::ImportKey {
            private_key,
            output,
            password,
        } => {
            cmd_import_key(private_key, output, password)?;
        }
        Commands::ExportKey { keystore, password } => {
            cmd_export_key(keystore, password)?;
        }
        Commands::Accounts { datadir } => {
            cmd_accounts(datadir).await?;
        }
        Commands::Peers { rpc } => {
            cmd_peers(rpc).await?;
        }
        Commands::Snapshot { datadir, output } => {
            cmd_snapshot(datadir, output).await?;
        }
        Commands::Restore { datadir, snapshot } => {
            cmd_restore(datadir, snapshot).await?;
        }
        Commands::SnapshotInfo { snapshot } => {
            cmd_snapshot_info(snapshot)?;
        }
        Commands::ConfigInit { output } => {
            cmd_config_init(output)?;
        }
    }

    Ok(())
}

async fn cmd_init(datadir: PathBuf, genesis_path: Option<PathBuf>) -> anyhow::Result<()> {
    info!("Initializing VeloChain at {:?}", datadir);

    // Load or create genesis
    let genesis = match genesis_path {
        Some(path) => {
            let data = std::fs::read_to_string(&path)?;
            serde_json::from_str::<Genesis>(&data)?
        }
        None => Genesis::default(),
    };

    // Create data directory
    std::fs::create_dir_all(&datadir)?;

    // Open database
    let db = Database::open(&datadir.join("db"))?;
    let db = Arc::new(db);

    // Create game world
    let game_world = GameWorld::new(genesis.config.world.seed);
    let game_state_root = game_world.state_root();

    // Create genesis block header
    let genesis_header = BlockHeader::genesis(game_state_root);

    // Initialize world state
    let state = WorldState::new(db.clone());

    // Set up initial account allocations
    for (address, alloc) in &genesis.alloc {
        let account = velochain_primitives::Account::with_balance(alloc.balance);
        state.put_account(address, &account)?;
    }
    state.commit()?;

    // Create chain and init genesis
    let txpool = Arc::new(TransactionPool::new());
    let poa_config = PoaConfig {
        period: genesis.config.consensus.period,
        epoch: genesis.config.consensus.epoch,
        chain_id: genesis.config.chain_id,
    };
    let consensus = Arc::new(PoaConsensus::new_readonly(
        genesis.config.consensus.validators.clone(),
        poa_config,
    ));

    let chain = Chain::new(
        db,
        Arc::new(state),
        Arc::new(game_world),
        txpool,
        consensus,
        genesis.config.chain_id,
    );

    chain.init_genesis(genesis_header)?;

    // Save genesis config
    let genesis_json = serde_json::to_string_pretty(&genesis)?;
    std::fs::write(datadir.join("genesis.json"), genesis_json)?;

    info!("VeloChain initialized successfully!");
    info!("  Chain ID: {}", genesis.config.chain_id);
    info!("  World seed: {}", genesis.config.world.seed);
    info!(
        "  Validators: {}",
        genesis.config.consensus.validators.len()
    );
    info!("  Data directory: {:?}", datadir);

    Ok(())
}

async fn cmd_run(
    datadir: PathBuf,
    rpc_addr: String,
    p2p_addr: String,
    validator: bool,
    validator_key: Option<String>,
    bootnodes: Option<String>,
) -> anyhow::Result<()> {
    info!("Starting VeloChain node...");

    // Load genesis
    let genesis_path = datadir.join("genesis.json");
    if !genesis_path.exists() {
        anyhow::bail!(
            "Chain not initialized. Run 'velochain init' first. Missing: {:?}",
            genesis_path
        );
    }
    let genesis: Genesis = serde_json::from_str(&std::fs::read_to_string(&genesis_path)?)?;

    // Open database
    let db = Arc::new(Database::open(&datadir.join("db"))?);

    // Create subsystems
    let state = Arc::new(WorldState::new(db.clone()));

    // Restore game world from persisted state, or create fresh
    let game_world = match db.get_game_state(b"world") {
        Ok(Some(data)) => match GameWorld::from_state(&data, genesis.config.world.seed) {
            Ok(world) => {
                info!("Restored game world from persistent storage");
                Arc::new(world)
            }
            Err(e) => {
                tracing::warn!("Failed to restore game world: {}, creating fresh", e);
                Arc::new(GameWorld::new(genesis.config.world.seed))
            }
        },
        _ => Arc::new(GameWorld::new(genesis.config.world.seed)),
    };
    let txpool = Arc::new(TransactionPool::new());

    let poa_config = PoaConfig {
        period: genesis.config.consensus.period,
        epoch: genesis.config.consensus.epoch,
        chain_id: genesis.config.chain_id,
    };

    let consensus = if validator {
        let key_hex = validator_key.ok_or_else(|| {
            anyhow::anyhow!("--validator-key is required when --validator is set")
        })?;
        let keypair = Keypair::from_hex(&key_hex)?;
        info!("Validator address: {:?}", keypair.address());
        Arc::new(PoaConsensus::new_with_keypair(
            keypair,
            genesis.config.consensus.validators.clone(),
            poa_config,
        ))
    } else {
        Arc::new(PoaConsensus::new_readonly(
            genesis.config.consensus.validators.clone(),
            poa_config,
        ))
    };

    let chain = Arc::new(Chain::new(
        db,
        state,
        game_world,
        txpool,
        consensus,
        genesis.config.chain_id,
    ));

    // Restore chain head from DB
    chain.restore_head()?;
    info!("Chain head at block {}", chain.block_number());

    // Start RPC server
    let rpc_config = RpcConfig {
        addr: rpc_addr.parse()?,
        chain_id: genesis.config.chain_id,
        ..Default::default()
    };
    let (event_tx, _event_rx) = new_event_channel();
    let session_manager = Arc::new(SessionManager::default());
    let rpc_addr = RpcServer::start(
        rpc_config,
        chain.db().clone(),
        chain.state().clone(),
        chain.game_world().clone(),
        chain.txpool().clone(),
        Some(event_tx),
        Some(session_manager),
    )
    .await?;
    info!("RPC server listening on {}", rpc_addr);

    // Start P2P network service
    let boot_nodes: Vec<Multiaddr> = bootnodes
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let net_config = NetworkConfig {
        listen_addr: p2p_addr.parse()?,
        boot_nodes,
        max_peers: 50,
    };

    let (network, mut net_events) = NetworkService::new(net_config).await?;
    let network = Arc::new(network);
    info!("P2P network started, peer_id={}", network.local_peer_id());

    // Start block producer if validator
    if validator {
        let block_producer = BlockProducer::new(chain.clone(), genesis.config.tick_interval_ms)
            .with_network(network.clone());

        info!("Starting block producer (validator mode)");
        tokio::spawn(async move {
            if let Err(e) = block_producer.start().await {
                tracing::error!("Block producer error: {}", e);
            }
        });
    }

    // Spawn network event handler with block buffering for chain sync
    let chain_for_net = chain.clone();
    let network_for_handler = network.clone();
    tokio::spawn(async move {
        // Buffer for out-of-order blocks (block_number -> Block)
        let mut block_buffer: BTreeMap<u64, velochain_primitives::Block> = BTreeMap::new();
        // Track which blocks we've already requested to avoid duplicate requests
        let mut requested_blocks: std::collections::HashSet<u64> = std::collections::HashSet::new();

        while let Some(event) = net_events.recv().await {
            match event {
                NetworkEvent::BlockReceived(block) | NetworkEvent::BlockResponseReceived(block) => {
                    let block_num = block.number();
                    let expected = chain_for_net.block_number() + 1;

                    if block_num < expected {
                        // Already have this block, skip
                        tracing::debug!("Skipping already-applied block {}", block_num);
                        continue;
                    }

                    if block_num == expected {
                        // This is the next expected block - apply it
                        info!(
                            "Received block from network: number={}, hash={}",
                            block.number(),
                            block.hash()
                        );
                        match chain_for_net.apply_block(&block) {
                            Ok(()) => {
                                requested_blocks.remove(&block_num);
                                // Try to apply buffered blocks sequentially
                                loop {
                                    let next = chain_for_net.block_number() + 1;
                                    if let Some(buffered) = block_buffer.remove(&next) {
                                        info!("Applying buffered block {}", next);
                                        match chain_for_net.apply_block(&buffered) {
                                            Ok(()) => {
                                                requested_blocks.remove(&next);
                                            }
                                            Err(e) => {
                                                tracing::warn!("Failed to apply buffered block {}: {}", next, e);
                                                break;
                                            }
                                        }
                                    } else {
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to apply received block {}: {}", block_num, e);
                            }
                        }
                    } else {
                        // Out of order - buffer it and request missing blocks
                        info!(
                            "Buffering out-of-order block {} (expected {})",
                            block_num, expected
                        );
                        block_buffer.insert(block_num, *block);

                        // Request missing blocks from peers
                        for missing in expected..block_num {
                            if !requested_blocks.contains(&missing) && !block_buffer.contains_key(&missing) {
                                tracing::debug!("Requesting missing block {} from peers", missing);
                                if let Err(e) = network_for_handler.request_block(missing) {
                                    tracing::warn!("Failed to request block {}: {}", missing, e);
                                }
                                requested_blocks.insert(missing);
                            }
                        }
                    }

                    // Limit buffer size to prevent memory issues
                    while block_buffer.len() > 1000 {
                        block_buffer.pop_first();
                    }
                }
                NetworkEvent::BlockRequested(number) => {
                    // A peer is requesting a block - respond if we have it
                    match chain_for_net.get_block_by_number(number) {
                        Ok(Some(block)) => {
                            tracing::debug!("Responding to block request for block {}", number);
                            if let Err(e) = network_for_handler.send_block_response(block) {
                                tracing::warn!("Failed to send block response: {}", e);
                            }
                        }
                        Ok(None) => {
                            tracing::debug!("Block {} not found for peer request", number);
                        }
                        Err(e) => {
                            tracing::warn!("Error looking up block {}: {}", number, e);
                        }
                    }
                }
                NetworkEvent::TransactionReceived(tx) => {
                    tracing::debug!("Received tx from network: hash={}", tx.hash);
                    if let Err(e) = chain_for_net.txpool().add_transaction(*tx) {
                        tracing::debug!("Failed to add received tx to pool: {}", e);
                    }
                }
                NetworkEvent::PeerConnected(peer_id) => {
                    info!("Peer connected: {}", peer_id);
                }
                NetworkEvent::PeerDisconnected(peer_id) => {
                    info!("Peer disconnected: {}", peer_id);
                }
                NetworkEvent::HeadersReceived(headers) => {
                    info!("Received {} block headers from peer", headers.len());
                }
                NetworkEvent::BodiesReceived(bodies) => {
                    info!("Received {} block bodies from peer", bodies.len());
                }
                NetworkEvent::PeerStatus {
                    peer_id,
                    best_block,
                    ..
                } => {
                    info!("Peer {} status: best_block={}", peer_id, best_block);
                }
            }
        }
    });

    // Initialize metrics
    let metrics_registry = prometheus::Registry::new();
    let metrics = NodeMetrics::new(&metrics_registry)
        .map_err(|e| anyhow::anyhow!("Failed to create metrics: {e}"))?;
    metrics.chain_height.set(chain.block_number() as i64);
    info!("Prometheus metrics initialized");

    // Install graceful shutdown handler
    let shutdown = ShutdownController::new();
    shutdown.install_signal_handlers();

    info!("VeloChain node running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    shutdown.wait_for_shutdown().await;
    info!("Graceful shutdown initiated...");

    // Persist game world state before exit
    let game_state = chain
        .game_world()
        .serialize_state()
        .map_err(|e| anyhow::anyhow!("Failed to serialize game world: {e}"))?;
    if let Err(e) = chain.db().put_game_state(b"world", &game_state) {
        tracing::error!("Failed to persist game world on shutdown: {}", e);
    } else {
        info!("Game world state persisted to database");
    }

    // Shut down network
    let _ = network.shutdown();
    info!("VeloChain node stopped.");

    Ok(())
}

async fn cmd_status(datadir: PathBuf) -> anyhow::Result<()> {
    let db = Database::open(&datadir.join("db"))?;

    match db.get_latest_block_number()? {
        Some(number) => {
            println!("VeloChain Status");
            println!("================");
            println!("Latest block: {}", number);
            println!("Data directory: {:?}", datadir);
        }
        None => {
            println!("Chain not initialized. Run 'velochain init' first.");
        }
    }

    Ok(())
}

fn cmd_generate_key(output: PathBuf, password: String) -> anyhow::Result<()> {
    let keypair = Keypair::random();
    keypair.save_keystore(&output, &password)?;
    println!("New validator key generated:");
    println!("  Address: {:?}", keypair.address());
    println!("  Keystore: {:?}", output);
    Ok(())
}

fn cmd_import_key(private_key: String, output: PathBuf, password: String) -> anyhow::Result<()> {
    let keypair = Keypair::from_hex(&private_key)?;
    keypair.save_keystore(&output, &password)?;
    println!("Key imported:");
    println!("  Address: {:?}", keypair.address());
    println!("  Keystore: {:?}", output);
    Ok(())
}

fn cmd_export_key(keystore: PathBuf, password: String) -> anyhow::Result<()> {
    let keypair = Keypair::load_keystore(&keystore, &password)?;
    println!("Address: {:?}", keypair.address());
    println!("Private key: 0x{}", keypair.secret_key_hex());
    Ok(())
}

async fn cmd_accounts(datadir: PathBuf) -> anyhow::Result<()> {
    let genesis_path = datadir.join("genesis.json");
    if !genesis_path.exists() {
        anyhow::bail!("Chain not initialized. Run 'velochain init' first.");
    }
    let genesis: Genesis = serde_json::from_str(&std::fs::read_to_string(&genesis_path)?)?;

    println!("VeloChain Accounts (from genesis)");
    println!("=================================");
    for (address, alloc) in &genesis.alloc {
        println!("  {:?} — balance: {}", address, alloc.balance);
    }
    println!("\nValidators:");
    for v in &genesis.config.consensus.validators {
        println!("  {:?}", v);
    }
    Ok(())
}

async fn cmd_peers(rpc: String) -> anyhow::Result<()> {
    // Parse host:port from URL like http://host:port
    let stripped = rpc
        .strip_prefix("http://")
        .or_else(|| rpc.strip_prefix("https://"))
        .unwrap_or(&rpc);
    let addr = if stripped.contains(':') {
        stripped
            .split('/')
            .next()
            .unwrap_or("127.0.0.1:8545")
            .to_string()
    } else {
        format!("{}:8545", stripped.split('/').next().unwrap_or("127.0.0.1"))
    };

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "net_version",
        "params": [],
        "id": 1,
    });
    let body_str = serde_json::to_string(&body)?;

    match tokio::net::TcpStream::connect(&addr).await {
        Ok(stream) => {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let request = format!(
                "POST / HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                addr,
                body_str.len(),
                body_str,
            );
            let (mut reader, mut writer) = stream.into_split();
            writer.write_all(request.as_bytes()).await?;
            writer.shutdown().await?;
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;
            let response = String::from_utf8_lossy(&buf);
            // Extract the JSON body after the HTTP headers
            if let Some(idx) = response.find("\r\n\r\n") {
                let json_body = &response[idx + 4..];
                if let Ok(result) = serde_json::from_str::<serde_json::Value>(json_body) {
                    println!("Connected to node at {}", rpc);
                    println!("  Network version: {}", result["result"]);
                } else {
                    println!("Connected to node at {} but got unexpected response", rpc);
                }
            } else {
                println!("Connected to {} but received malformed response", rpc);
            }
        }
        Err(e) => {
            println!("Failed to connect to {}: {}", rpc, e);
            println!("Make sure a VeloChain node is running with RPC enabled.");
        }
    }
    Ok(())
}

async fn cmd_snapshot(datadir: PathBuf, output: PathBuf) -> anyhow::Result<()> {
    let genesis_path = datadir.join("genesis.json");
    if !genesis_path.exists() {
        anyhow::bail!("Chain not initialized. Run 'velochain init' first.");
    }
    let genesis: Genesis = serde_json::from_str(&std::fs::read_to_string(&genesis_path)?)?;

    let db = Arc::new(Database::open(&datadir.join("db"))?);
    let game_world = match db.get_game_state(b"world") {
        Ok(Some(data)) => match GameWorld::from_state(&data, genesis.config.world.seed) {
            Ok(world) => Arc::new(world),
            Err(_) => Arc::new(GameWorld::new(genesis.config.world.seed)),
        },
        _ => Arc::new(GameWorld::new(genesis.config.world.seed)),
    };

    let meta = velochain_node::export_snapshot(&db, &game_world, genesis.config.chain_id, &output)?;

    println!("Snapshot exported successfully!");
    println!("  Block number: {}", meta.block_number);
    println!("  Block hash: {}", meta.block_hash);
    println!("  Game tick: {}", meta.game_tick);
    println!("  Blocks: {}", meta.block_count);
    println!("  File: {:?}", output);

    Ok(())
}

async fn cmd_restore(datadir: PathBuf, snapshot_path: PathBuf) -> anyhow::Result<()> {
    let genesis_path = datadir.join("genesis.json");
    if !genesis_path.exists() {
        anyhow::bail!("Chain not initialized. Run 'velochain init' first.");
    }
    let genesis: Genesis = serde_json::from_str(&std::fs::read_to_string(&genesis_path)?)?;

    let db = Arc::new(Database::open(&datadir.join("db"))?);
    let state = Arc::new(WorldState::new(db.clone()));

    let (meta, _game_world) =
        velochain_node::import_snapshot(&db, &state, &snapshot_path, genesis.config.world.seed)?;

    println!("Snapshot restored successfully!");
    println!("  Block number: {}", meta.block_number);
    println!("  Block hash: {}", meta.block_hash);
    println!("  Game tick: {}", meta.game_tick);
    println!("  Blocks imported: {}", meta.block_count);

    Ok(())
}

fn cmd_snapshot_info(snapshot_path: PathBuf) -> anyhow::Result<()> {
    let meta = velochain_node::read_snapshot_meta(&snapshot_path)?;

    println!("VeloChain Snapshot Info");
    println!("=======================");
    println!("  Version: {}", meta.version);
    println!("  Chain ID: {}", meta.chain_id);
    println!("  Block number: {}", meta.block_number);
    println!("  Block hash: {}", meta.block_hash);
    println!("  Game tick: {}", meta.game_tick);
    println!("  Block count: {}", meta.block_count);
    println!("  Created at: {} (unix)", meta.created_at);

    Ok(())
}

fn cmd_config_init(output: PathBuf) -> anyhow::Result<()> {
    ConfigFile::write_default(&output)?;
    println!("Default configuration written to {:?}", output);
    println!(
        "Edit this file and use it with: velochain run --config {:?}",
        output
    );
    Ok(())
}
