//! P2P networking layer built on libp2p.

pub mod constants;
pub mod messages;
pub mod peer_score;
pub mod topics;

use libp2p::gossipsub::{self, MessageAuthenticity, MessageId};
use libp2p::identify;
use libp2p::kad::{self, store::MemoryStore};
use libp2p::mdns;
use libp2p::ping;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{identity, Multiaddr, PeerId, Swarm};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use parity_scale_codec::{Decode, Encode};

use crate::core::block::{Block, CommitVote};
use crate::core::transaction::Transaction;
use crate::network::messages::{EquivocationEvidence, GossipMessage};
use crate::network::peer_score::{PeerScoreRepo, ScoreEvent};
use crate::network::topics::TopicConfig;

// ---------------------------------------------------------------------------
// Combined behaviour
// ---------------------------------------------------------------------------

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "CombinedEvent")]
pub struct MononiumBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
}

#[derive(Debug)]
pub enum CombinedEvent {
    Gossipsub(gossipsub::Event),
    Kademlia(kad::Event),
    Mdns(mdns::Event),
    Identify(identify::Event),
    Ping(ping::Event),
}

impl From<gossipsub::Event> for CombinedEvent {
    fn from(e: gossipsub::Event) -> Self { Self::Gossipsub(e) }
}
impl From<kad::Event> for CombinedEvent {
    fn from(e: kad::Event) -> Self { Self::Kademlia(e) }
}
impl From<mdns::Event> for CombinedEvent {
    fn from(e: mdns::Event) -> Self { Self::Mdns(e) }
}
impl From<identify::Event> for CombinedEvent {
    fn from(e: identify::Event) -> Self { Self::Identify(e) }
}
impl From<ping::Event> for CombinedEvent {
    fn from(e: ping::Event) -> Self { Self::Ping(e) }
}

// ---------------------------------------------------------------------------
// P2pConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct P2pConfig {
    pub p2p_port: u16,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub enable_mdns: bool,
    pub max_peers: usize,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            p2p_port: constants::DEFAULT_P2P_PORT,
            bootstrap_peers: vec![],
            enable_mdns: true,
            max_peers: constants::MAX_PEERS,
        }
    }
}

// ---------------------------------------------------------------------------
// Command channel
// ---------------------------------------------------------------------------

enum P2pCommand {
    Dial(Multiaddr),
    PublishTx(Vec<Transaction>),
    PublishBlock(Box<Block>),
    PublishVote(CommitVote),
    PublishEvidence(Box<EquivocationEvidence>),
    Shutdown,
}

// ---------------------------------------------------------------------------
// P2pHandle
// ---------------------------------------------------------------------------

/// A handle to a running P2P service.
pub struct P2pHandle {
    cmd_tx: mpsc::Sender<P2pCommand>,
    local_peer_id: PeerId,
}

impl P2pHandle {
    #[must_use]
    pub fn local_peer_id(&self) -> &PeerId { &self.local_peer_id }

    /// Dial a remote peer at the given multiaddress.
    pub fn dial(&self, addr: Multiaddr) -> Result<(), Box<dyn std::error::Error>> {
        self.cmd_tx.try_send(P2pCommand::Dial(addr))?;
        Ok(())
    }

    /// Publish transactions to the gossip network.
    pub async fn publish_tx(&self, txs: Vec<Transaction>) -> Result<(), Box<dyn std::error::Error>> {
        self.cmd_tx.send(P2pCommand::PublishTx(txs)).await?;
        Ok(())
    }

    /// Signal the event loop to shut down and wait for it to finish.
    pub async fn shutdown(self) {
        let _ = self.cmd_tx.send(P2pCommand::Shutdown).await;
    }
}

// ---------------------------------------------------------------------------
// P2pService
// ---------------------------------------------------------------------------

pub struct P2pService {
    swarm: Swarm<MononiumBehaviour>,
    local_peer_id: PeerId,
    chain_id: u64,
    p2p_port: u16,
    peer_scores: PeerScoreRepo,
}

impl P2pService {
    pub fn new(config: P2pConfig, chain_id: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = local_key.public().to_peer_id();

        // Gossipsub
        let message_id_fn = |msg: &gossipsub::Message| {
            let hash = blake3::hash(&msg.data);
            MessageId::from(&hash.as_bytes()[..20])
        };
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .message_id_fn(message_id_fn)
            .max_transmit_size(constants::GOSSIP_MAX_TRANSMIT_SIZE)
            .history_length(10)
            .gossip_factor(0.25)
            .build()?;
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )?;

        // Kademlia
        let kademlia = kad::Behaviour::new(
            local_peer_id,
            MemoryStore::new(local_peer_id),
        );

        // mDNS
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

        // Identify
        let identify = identify::Behaviour::new(
            identify::Config::new(
                constants::PROTOCOL_VERSION.to_string(),
                local_key.public(),
            ),
        );

        // Ping
        let ping = ping::Behaviour::default();

        let behaviour = MononiumBehaviour {
            gossipsub,
            kademlia,
            mdns,
            identify,
            ping,
        };

        let swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            .with_behaviour(|_| behaviour)?
            .with_swarm_config(|c| c.with_max_negotiating_inbound_streams(config.max_peers))
            .build();

        Ok(Self {
            swarm,
            local_peer_id,
            chain_id,
            p2p_port: config.p2p_port,
            peer_scores: PeerScoreRepo::new(),
        })
    }

    /// Start the P2P event loop. Consumes `self` and returns a [`P2pHandle`].
    pub fn start(mut self) -> Result<P2pHandle, Box<dyn std::error::Error>> {
        let listen_addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}", self.p2p_port)
            .parse()?;
        self.swarm.listen_on(listen_addr)?;

        for topic in TopicConfig::standard_topics(self.chain_id) {
            let gt = libp2p::gossipsub::IdentTopic::new(topic.name);
            self.swarm.behaviour_mut().gossipsub.subscribe(&gt)?;
            info!("subscribed to: {gt}");
        }

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<P2pCommand>(64);
        let local_peer_id = self.local_peer_id;
        let _handle = tokio::spawn(async move {
            use libp2p::futures::StreamExt;
            loop {
                tokio::select! {
                    event = self.swarm.select_next_some() => self.handle_event(event),
                    cmd = cmd_rx.recv() => match cmd {
                        Some(P2pCommand::Dial(addr)) => {
                            if let Err(e) = self.swarm.dial(addr) {
                                warn!("dial failed: {e}");
                            }
                        }
                        Some(P2pCommand::PublishTx(txs)) => {
                            let data = GossipMessage::Txs(txs).encode();
                            let topic = libp2p::gossipsub::IdentTopic::new(
                                format!("mononium/txs/{}", self.chain_id)
                            );
                            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                warn!("publish txs failed: {e}");
                            }
                        }
                        Some(P2pCommand::PublishBlock(block)) => {
                            let data = GossipMessage::Block(block).encode();
                            let topic = libp2p::gossipsub::IdentTopic::new(
                                format!("mononium/blocks/{}", self.chain_id)
                            );
                            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                warn!("publish block failed: {e}");
                            }
                        }
                        Some(P2pCommand::PublishVote(vote)) => {
                            let data = GossipMessage::Vote(vote).encode();
                            let topic = libp2p::gossipsub::IdentTopic::new(
                                format!("mononium/votes/{}", self.chain_id)
                            );
                            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                warn!("publish vote failed: {e}");
                            }
                        }
                        Some(P2pCommand::PublishEvidence(evidence)) => {
                            let data = GossipMessage::Evidence(evidence).encode();
                            let topic = libp2p::gossipsub::IdentTopic::new(
                                format!("mononium/evidence/{}", self.chain_id)
                            );
                            if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                warn!("publish evidence failed: {e}");
                            }
                        }
                        Some(P2pCommand::Shutdown) | None => {
                            info!("P2P shutting down");
                            break;
                        }
                    }
                }
            }
        });

        Ok(P2pHandle { cmd_tx, local_peer_id })
    }

    fn handle_event(&mut self, event: SwarmEvent<CombinedEvent>) {
        match event {
            SwarmEvent::Behaviour(CombinedEvent::Gossipsub(e)) => {
                if let gossipsub::Event::Message { propagation_source, message, .. } = e {
                    match GossipMessage::decode(&mut &message.data[..]) {
                        Ok(_) => {
                            self.peer_scores.apply_event(&propagation_source, ScoreEvent::ValidBlockPropagated);
                        }
                        Err(_) => {
                            self.peer_scores.apply_event(&propagation_source, ScoreEvent::InvalidBlockGossiped);
                        }
                    }
                }
            }
            SwarmEvent::NewListenAddr { address, .. } => info!("listening on {address}"),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[tokio::test]
    async fn test_p2p_service_start_returns_join_handle() {
        let config = P2pConfig::default();
        let service = P2pService::new(config, 0).unwrap();
        let handle = service.start().unwrap();
        handle.shutdown().await;
    }

    #[tokio::test]
    async fn test_p2p_service_dial_connects_peers() {
        let port1 = pick_unused_port();
        let port2 = pick_unused_port();

        let cfg1 = P2pConfig { p2p_port: port1, ..Default::default() };
        let cfg2 = P2pConfig { p2p_port: port2, ..Default::default() };

        let node1 = P2pService::new(cfg1, 0).unwrap();
        let node2 = P2pService::new(cfg2, 0).unwrap();

        let handle1 = node1.start().unwrap();
        let handle2 = node2.start().unwrap();

        // Give nodes time to start listening
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // node2 dials node1
        let addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{port1}").parse().unwrap();
        handle2.dial(addr).unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        handle1.shutdown().await;
        handle2.shutdown().await;
    }

    #[tokio::test]
    async fn test_p2p_event_channel_delivers_gossip() {
        let port1 = pick_unused_port();
        let port2 = pick_unused_port();

        let cfg1 = P2pConfig { p2p_port: port1, ..Default::default() };
        let cfg2 = P2pConfig { p2p_port: port2, ..Default::default() };

        let node1 = P2pService::new(cfg1, 0).unwrap();
        let node2 = P2pService::new(cfg2, 0).unwrap();

        let handle1 = node1.start().unwrap();
        let handle2 = node2.start().unwrap();

        let mut events = handle2.subscribe();

        // Connect node2 → node1
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{port1}").parse().unwrap();
        handle2.dial(addr).unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Publish a transaction from node1
        use crate::core::account::Address;
        use crate::core::transaction::Transaction;
        use crate::crypto::falcon::Falcon512Signature;
        use crate::crypto::constants::FALCON_SIGNATURE_SIZE;
        use primitive_types::U256;

        let tx = Transaction {
            chain_id: 0,
            nonce: 0,
            sender: Address::from([0x11u8; 32]),
            fee: U256::zero(),
            body: crate::core::transaction::TxBody::Transfer {
                recipient: Address::from([0x22u8; 32]),
                amount: U256::from(100),
            },
            signature: Falcon512Signature::from_bytes(&[0u8; FALCON_SIGNATURE_SIZE]).unwrap(),
        };
        handle1.publish_tx(vec![tx]).await.unwrap();

        // Wait for gossip propagation
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // Verify event received on node2
        match events.try_recv() {
            Ok(P2pEvent::TxReceived { .. }) => {} // success
            Ok(other) => panic!("expected TxReceived, got {other:?}"),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                panic!("no event received within timeout");
            }
            Err(e) => panic!("channel error: {e}"),
        }

        handle1.shutdown().await;
        handle2.shutdown().await;
    }

    /// Find an available TCP port on localhost.
    fn pick_unused_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }
}
