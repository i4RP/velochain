//! VeloChain node CLI entry point.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use velochain_consensus::poa::{PoaConfig, PoaConsensus};
use velochain_game_engine::GameWorld;
use velochain_network::service::{NetworkConfig, NetworkService};
use velochain_network::{Multiaddr, NetworkEvent};
use velochain_node::{BlockProducer, Chain};
use velochain_primitives::{BlockHeader, Genesis, Keypair};
use velochain_rpc::{server::RpcConfig, RpcServer};
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
            cmd_run(datadir, rpc_addr, p2p_addr, validator, validator_key, bootnodes).await?;
        }
        Commands::Status { datadir } => {
            cmd_status(datadir).await?;
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
        Ok(Some(data)) => {
            match GameWorld::from_state(&data, genesis.config.world.seed) {
                Ok(world) => {
                    info!("Restored game world from persistent storage");
                    Arc::new(world)
                }
                Err(e) => {
                    tracing::warn!("Failed to restore game world: {}, creating fresh", e);
                    Arc::new(GameWorld::new(genesis.config.world.seed))
                }
            }
        }
        _ => Arc::new(GameWorld::new(genesis.config.world.seed)),
    };
    let txpool = Arc::new(TransactionPool::new());

    let poa_config = PoaConfig {
        period: genesis.config.consensus.period,
        epoch: genesis.config.consensus.epoch,
        chain_id: genesis.config.chain_id,
    };

    let consensus = if validator {
        let key_hex = validator_key
            .ok_or_else(|| anyhow::anyhow!("--validator-key is required when --validator is set"))?;
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
    };
    let rpc_addr = RpcServer::start(
        rpc_config,
        chain.db().clone(),
        chain.state().clone(),
        chain.game_world().clone(),
        chain.txpool().clone(),
    ).await?;
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
        let block_producer = BlockProducer::new(
            chain.clone(),
            genesis.config.tick_interval_ms,
        )
        .with_network(network.clone());

        info!("Starting block producer (validator mode)");
        tokio::spawn(async move {
            if let Err(e) = block_producer.start().await {
                tracing::error!("Block producer error: {}", e);
            }
        });
    }

    // Spawn network event handler
    let chain_for_net = chain.clone();
    tokio::spawn(async move {
        while let Some(event) = net_events.recv().await {
            match event {
                NetworkEvent::BlockReceived(block) => {
                    info!(
                        "Received block from network: number={}, hash={}",
                        block.number(),
                        block.hash()
                    );
                    if let Err(e) = chain_for_net.apply_block(&block) {
                        tracing::warn!("Failed to apply received block: {}", e);
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
            }
        }
    });

    info!("VeloChain node running. Press Ctrl+C to stop.");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");
    let _ = network.shutdown();

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
