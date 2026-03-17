//! Network service managing P2P connections and message handling.

use crate::error::NetworkError;
use crate::protocol::{self, NetworkMessage};
use futures::StreamExt;
use libp2p::{
    gossipsub, identify, mdns, noise,
    swarm::SwarmEvent,
    tcp, yamux, Multiaddr, PeerId,
};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Combined libp2p behaviour for VeloChain.
#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct VelochainBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
}

/// Network service configuration.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Address to listen on.
    pub listen_addr: Multiaddr,
    /// Bootstrap peer addresses.
    pub boot_nodes: Vec<Multiaddr>,
    /// Maximum number of peers.
    pub max_peers: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/30303".parse().unwrap(),
            boot_nodes: Vec::new(),
            max_peers: 50,
        }
    }
}

/// The main network service.
pub struct NetworkService {
    /// Connected peers.
    peers: Arc<RwLock<HashSet<PeerId>>>,
    /// Channel for sending messages to the network event loop.
    command_tx: mpsc::UnboundedSender<NetworkCommand>,
    /// Our local peer ID.
    local_peer_id: PeerId,
}

/// Commands that can be sent to the network event loop.
#[derive(Debug)]
pub enum NetworkCommand {
    /// Broadcast a block to all peers.
    BroadcastBlock(velochain_primitives::Block),
    /// Broadcast a transaction to all peers.
    BroadcastTransaction(velochain_primitives::SignedTransaction),
    /// Connect to a peer.
    Dial(Multiaddr),
    /// Shutdown the network.
    Shutdown,
}

/// Events emitted by the network to the node.
#[derive(Debug)]
pub enum NetworkEvent {
    /// A new block was received from a peer.
    BlockReceived(velochain_primitives::Block),
    /// A new transaction was received from a peer.
    TransactionReceived(velochain_primitives::SignedTransaction),
    /// A peer connected.
    PeerConnected(PeerId),
    /// A peer disconnected.
    PeerDisconnected(PeerId),
}

impl NetworkService {
    /// Create a new network service and start the event loop.
    pub async fn new(
        config: NetworkConfig,
    ) -> Result<(Self, mpsc::UnboundedReceiver<NetworkEvent>), NetworkError> {
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
                        .map_err(|e| NetworkError::Transport(e.to_string()))?
                        .with_behaviour(|key| {
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(std::time::Duration::from_secs(1))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .build()
                    .map_err(|e| NetworkError::Protocol(e.to_string()))
                    .expect("valid gossipsub config");

                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .expect("valid gossipsub behaviour");

                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                )
                .expect("valid mdns");

                let identify = identify::Behaviour::new(identify::Config::new(
                    "/velochain/1.0.0".to_string(),
                    key.public(),
                ));

                Ok(VelochainBehaviour {
                    gossipsub,
                    mdns,
                    identify,
                })
            })
                        .map_err(|e| NetworkError::Protocol(e.to_string()))?
                        .build();

        let local_peer_id = *swarm.local_peer_id();

        // Subscribe to topics
        let block_topic = gossipsub::IdentTopic::new(protocol::topics::BLOCKS);
        let tx_topic = gossipsub::IdentTopic::new(protocol::topics::TRANSACTIONS);

        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&block_topic)
            .map_err(|e: gossipsub::SubscriptionError| NetworkError::Protocol(e.to_string()))?;
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&tx_topic)
            .map_err(|e: gossipsub::SubscriptionError| NetworkError::Protocol(e.to_string()))?;

        // Listen on configured address
        swarm
            .listen_on(config.listen_addr.clone())
            .map_err(|e: libp2p::TransportError<std::io::Error>| NetworkError::Transport(e.to_string()))?;

        info!("Network listening on {}, peer_id={}", config.listen_addr, local_peer_id);

        // Dial boot nodes
        for addr in &config.boot_nodes {
            match swarm.dial(addr.clone()) {
                Ok(_) => info!("Dialing boot node: {}", addr),
                Err(e) => warn!("Failed to dial {}: {}", addr, e),
            }
        }

        let peers = Arc::new(RwLock::new(HashSet::new()));
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let peers_clone = peers.clone();

        // Spawn the network event loop
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = swarm.select_next_some() => {
                        match event {
                            SwarmEvent::Behaviour(VelochainBehaviourEvent::Gossipsub(
                                gossipsub::Event::Message {
                                    propagation_source: _,
                                    message_id: _,
                                    message,
                                },
                            )) => {
                                if let Ok(msg) = serde_json::from_slice::<NetworkMessage>(&message.data) {
                                    match msg {
                                        NetworkMessage::NewBlock(block) => {
                                            let _ = event_tx.send(NetworkEvent::BlockReceived(block));
                                        }
                                        NetworkMessage::NewTransaction(tx) => {
                                            let _ = event_tx.send(NetworkEvent::TransactionReceived(tx));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            SwarmEvent::Behaviour(VelochainBehaviourEvent::Mdns(
                                mdns::Event::Discovered(list),
                            )) => {
                                for (peer_id, addr) in list {
                                    debug!("mDNS discovered peer: {} at {}", peer_id, addr);
                                    swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                                    peers_clone.write().insert(peer_id);
                                    let _ = event_tx.send(NetworkEvent::PeerConnected(peer_id));
                                }
                            }
                            SwarmEvent::Behaviour(VelochainBehaviourEvent::Mdns(
                                mdns::Event::Expired(list),
                            )) => {
                                for (peer_id, _addr) in list {
                                    swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                                    peers_clone.write().remove(&peer_id);
                                    let _ = event_tx.send(NetworkEvent::PeerDisconnected(peer_id));
                                }
                            }
                            _ => {}
                        }
                    }
                    cmd = command_rx.recv() => {
                        match cmd {
                            Some(NetworkCommand::BroadcastBlock(block)) => {
                                let msg = NetworkMessage::NewBlock(block);
                                if let Ok(data) = serde_json::to_vec(&msg) {
                                    let topic = gossipsub::IdentTopic::new(protocol::topics::BLOCKS);
                                    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                        error!("Failed to publish block: {}", e);
                                    }
                                }
                            }
                            Some(NetworkCommand::BroadcastTransaction(tx)) => {
                                let msg = NetworkMessage::NewTransaction(tx);
                                if let Ok(data) = serde_json::to_vec(&msg) {
                                    let topic = gossipsub::IdentTopic::new(protocol::topics::TRANSACTIONS);
                                    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                        error!("Failed to publish transaction: {}", e);
                                    }
                                }
                            }
                            Some(NetworkCommand::Dial(addr)) => {
                                let _ = swarm.dial(addr);
                            }
                            Some(NetworkCommand::Shutdown) | None => {
                                info!("Network service shutting down");
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok((
            Self {
                peers,
                command_tx,
                local_peer_id,
            },
            event_rx,
        ))
    }

    /// Get the local peer ID.
    pub fn local_peer_id(&self) -> &PeerId {
        &self.local_peer_id
    }

    /// Get the number of connected peers.
    pub fn peer_count(&self) -> usize {
        self.peers.read().len()
    }

    /// Broadcast a block to the network.
    pub fn broadcast_block(&self, block: velochain_primitives::Block) -> Result<(), NetworkError> {
        self.command_tx
            .send(NetworkCommand::BroadcastBlock(block))
            .map_err(|_| NetworkError::ChannelClosed)
    }

    /// Broadcast a transaction to the network.
    pub fn broadcast_transaction(
        &self,
        tx: velochain_primitives::SignedTransaction,
    ) -> Result<(), NetworkError> {
        self.command_tx
            .send(NetworkCommand::BroadcastTransaction(tx))
            .map_err(|_| NetworkError::ChannelClosed)
    }

    /// Shutdown the network service.
    pub fn shutdown(&self) -> Result<(), NetworkError> {
        self.command_tx
            .send(NetworkCommand::Shutdown)
            .map_err(|_| NetworkError::ChannelClosed)
    }
}
