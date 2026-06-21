//! P2P networking layer built on libp2p.

pub mod constants;
pub mod messages;
pub mod peer_score;
pub mod sync;
pub mod topics;

use libp2p::gossipsub::{self, MessageAuthenticity, MessageId};
use libp2p::identify;
use libp2p::kad::{self, store::MemoryStore};
use libp2p::mdns;
use libp2p::ping;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{identity, Multiaddr, PeerId, Swarm};
use tokio::sync::{broadcast, mpsc};
use tracing::{info, warn};

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
// P2pEvent — events emitted by the P2P layer to higher-level consumers
// ---------------------------------------------------------------------------

/// Events that higher layers (consensus, mempool, etc.) can subscribe to.
#[derive(Debug, Clone)]
pub enum P2pEvent {
    /// A gossip message containing transactions was received.
    TxReceived {
        source: PeerId,
        txs: Vec<Transaction>,
    },
    /// A gossip message containing a block was received.
    BlockReceived {
        source: PeerId,
        block: Box<Block>,
    },
    /// A gossip message containing a commit vote was received.
    VoteReceived {
        source: PeerId,
        vote: CommitVote,
    },
    /// A gossip message containing equivocation evidence was received.
    EvidenceReceived {
        source: PeerId,
        evidence: Box<EquivocationEvidence>,
    },
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
    event_tx: broadcast::Sender<P2pEvent>,
}

impl P2pHandle {
    #[must_use]
    pub fn local_peer_id(&self) -> &PeerId { &self.local_peer_id }

    /// Subscribe to events emitted by this P2P node.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<P2pEvent> {
        self.event_tx.subscribe()
    }

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

    /// Publish a block to the gossip network.
    pub async fn publish_block(&self, block: Block) -> Result<(), Box<dyn std::error::Error>> {
        self.cmd_tx.send(P2pCommand::PublishBlock(Box::new(block))).await?;
        Ok(())
    }

    /// Publish a commit vote to the gossip network.
    pub async fn publish_vote(&self, vote: CommitVote) -> Result<(), Box<dyn std::error::Error>> {
        self.cmd_tx.send(P2pCommand::PublishVote(vote)).await?;
        Ok(())
    }

    /// Publish equivocation evidence to the gossip network.
    pub async fn publish_evidence(&self, evidence: EquivocationEvidence) -> Result<(), Box<dyn std::error::Error>> {
        self.cmd_tx.send(P2pCommand::PublishEvidence(Box::new(evidence))).await?;
        Ok(())
    }

    /// Signal the event loop to shut down and wait for it to finish.
    pub async fn shutdown(self) {
        let _ = self.cmd_tx.send(P2pCommand::Shutdown).await;
    }
}

// ---------------------------------------------------------------------------
// Utility: validate and encode outgoing gossip messages
// ---------------------------------------------------------------------------

/// Encode a [`GossipMessage`] and validate it fits within the topic's size
/// limit. Returns the encoded bytes on success, or an error message.
fn prepare_gossip_message(msg: &GossipMessage, topic: &TopicConfig) -> Result<Vec<u8>, String> {
    let data = msg.encode();
    if !topic.validate_size(data.len()) {
        return Err(format!(
            "gossip message too large: {} bytes exceeds {} byte limit",
            data.len(),
            topic.max_message_size,
        ));
    }
    Ok(data)
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
    event_tx: broadcast::Sender<P2pEvent>,
    bootstrap_peers: Vec<Multiaddr>,
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

        // mDNS (always enabled; `enable_mdns` config flag is accepted but
        // unconditional in libp2p 0.56 — harmless when no responders exist)
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

        let (event_tx, _) = broadcast::channel(256);

        Ok(Self {
            swarm,
            local_peer_id,
            chain_id,
            p2p_port: config.p2p_port,
            peer_scores: PeerScoreRepo::new(),
            event_tx,
            bootstrap_peers: config.bootstrap_peers,
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

        // Dial bootstrap peers concurrently at startup
        for peer_addr in std::mem::take(&mut self.bootstrap_peers) {
            info!("dialing bootstrap peer: {peer_addr}");
            if let Err(e) = self.swarm.dial(peer_addr) {
                warn!("bootstrap dial failed: {e}");
            }
        }

        // Start Kademlia random walk for peer discovery
        if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
            warn!("kademlia bootstrap failed: {e}");
        }

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<P2pCommand>(64);
        let local_peer_id = self.local_peer_id;
        let event_tx = self.event_tx.clone();
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
                            let topics = TopicConfig::standard_topics(self.chain_id);
                            let gossip_msg = GossipMessage::Txs(txs);
                            match prepare_gossip_message(&gossip_msg, &topics[0]) {
                                Ok(data) => {
                                    let topic = libp2p::gossipsub::IdentTopic::new(topics[0].name.clone());
                                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                        warn!("publish txs failed: {e}");
                                    }
                                }
                                Err(e) => warn!("{e}"),
                            }
                        }
                        Some(P2pCommand::PublishBlock(block)) => {
                            let topics = TopicConfig::standard_topics(self.chain_id);
                            let gossip_msg = GossipMessage::Block(block);
                            match prepare_gossip_message(&gossip_msg, &topics[1]) {
                                Ok(data) => {
                                    let topic = libp2p::gossipsub::IdentTopic::new(topics[1].name.clone());
                                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                        warn!("publish block failed: {e}");
                                    }
                                }
                                Err(e) => warn!("{e}"),
                            }
                        }
                        Some(P2pCommand::PublishVote(vote)) => {
                            let topics = TopicConfig::standard_topics(self.chain_id);
                            let gossip_msg = GossipMessage::Vote(vote);
                            match prepare_gossip_message(&gossip_msg, &topics[2]) {
                                Ok(data) => {
                                    let topic = libp2p::gossipsub::IdentTopic::new(topics[2].name.clone());
                                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                        warn!("publish vote failed: {e}");
                                    }
                                }
                                Err(e) => warn!("{e}"),
                            }
                        }
                        Some(P2pCommand::PublishEvidence(evidence)) => {
                            let topics = TopicConfig::standard_topics(self.chain_id);
                            let gossip_msg = GossipMessage::Evidence(evidence);
                            match prepare_gossip_message(&gossip_msg, &topics[3]) {
                                Ok(data) => {
                                    let topic = libp2p::gossipsub::IdentTopic::new(topics[3].name.clone());
                                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                        warn!("publish evidence failed: {e}");
                                    }
                                }
                                Err(e) => warn!("{e}"),
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

        Ok(P2pHandle { cmd_tx, local_peer_id, event_tx })
    }

    fn handle_event(&mut self, event: SwarmEvent<CombinedEvent>) {
        match event {
            SwarmEvent::Behaviour(CombinedEvent::Gossipsub(e)) => {
                if let gossipsub::Event::Message { propagation_source, message, .. } = e {
                    let event_tx = self.event_tx.clone();
                    match GossipMessage::decode(&mut &message.data[..]) {
                        Ok(GossipMessage::Txs(txs)) => {
                            self.peer_scores.apply_event(&propagation_source, ScoreEvent::ValidBlockPropagated);
                            let _ = event_tx.send(P2pEvent::TxReceived { source: propagation_source, txs });
                        }
                        Ok(GossipMessage::Block(block)) => {
                            self.peer_scores.apply_event(&propagation_source, ScoreEvent::ValidBlockPropagated);
                            let _ = event_tx.send(P2pEvent::BlockReceived { source: propagation_source, block });
                        }
                        Ok(GossipMessage::Vote(vote)) => {
                            self.peer_scores.apply_event(&propagation_source, ScoreEvent::ValidVotePropagated);
                            let _ = event_tx.send(P2pEvent::VoteReceived { source: propagation_source, vote });
                        }
                        Ok(GossipMessage::Evidence(evidence)) => {
                            let _ = event_tx.send(P2pEvent::EvidenceReceived { source: propagation_source, evidence });
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

    #[test]
    fn test_prepare_gossip_message_accepts_valid_size() {
        let topics = TopicConfig::standard_topics(0);
        let msg = GossipMessage::Txs(vec![]);
        let result = prepare_gossip_message(&msg, &topics[0]);
        assert!(result.is_ok(), "empty txs should fit in 1MB topic");
    }

    #[test]
    fn test_prepare_gossip_message_rejects_oversized() {
        let tiny_topic = TopicConfig::new("tiny", 0, 1);
        let msg = GossipMessage::Txs(vec![]);
        let result = prepare_gossip_message(&msg, &tiny_topic);
        assert!(result.is_err(), "any message exceeds 0-byte limit");
    }

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

    #[tokio::test]
    async fn test_p2p_bootstrap_dial_connects_automatically() {
        let port1 = pick_unused_port();
        let port2 = pick_unused_port();
        let addr1: Multiaddr = format!("/ip4/127.0.0.1/tcp/{port1}").parse().unwrap();

        let cfg1 = P2pConfig { p2p_port: port1, bootstrap_peers: vec![], ..Default::default() };
        let cfg2 = P2pConfig { p2p_port: port2, bootstrap_peers: vec![addr1], ..Default::default() };

        let node1 = P2pService::new(cfg1, 0).unwrap();
        let node2 = P2pService::new(cfg2, 0).unwrap();

        let handle1 = node1.start().unwrap();
        let handle2 = node2.start().unwrap();

        let mut events1 = handle1.subscribe();

        // Node2 should automatically dial node1 via bootstrap_peers.
        // Wait for connection + gossipsub mesh formation.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // Verify: node2 publishes a tx → node1 receives it via event channel
        use crate::core::account::Address;
        use crate::core::transaction::Transaction;
        use crate::crypto::falcon::Falcon512Signature;
        use crate::crypto::constants::FALCON_SIGNATURE_SIZE;
        use primitive_types::U256;

        let tx = Transaction {
            chain_id: 0,
            nonce: 0,
            sender: Address::from([0x33u8; 32]),
            fee: U256::zero(),
            body: crate::core::transaction::TxBody::Transfer {
                recipient: Address::from([0x44u8; 32]),
                amount: U256::from(200),
            },
            signature: Falcon512Signature::from_bytes(&[0u8; FALCON_SIGNATURE_SIZE]).unwrap(),
        };
        handle2.publish_tx(vec![tx]).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        match events1.try_recv() {
            Ok(P2pEvent::TxReceived { .. }) => {} // success
            Ok(other) => panic!("expected TxReceived from bootnode dial, got {other:?}"),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                panic!("no event received — bootstrap dial likely failed");
            }
            Err(e) => panic!("channel error: {e}"),
        }

        handle1.shutdown().await;
        handle2.shutdown().await;
    }

    #[tokio::test]
    async fn test_p2p_publish_block_delivers_block_received_event() {
        use crate::core::block::{BlockHeader, BlockBody};

        let port1 = pick_unused_port();
        let port2 = pick_unused_port();

        let cfg1 = P2pConfig { p2p_port: port1, ..Default::default() };
        let cfg2 = P2pConfig { p2p_port: port2, ..Default::default() };

        let node1 = P2pService::new(cfg1, 0).unwrap();
        let node2 = P2pService::new(cfg2, 0).unwrap();

        let handle1 = node1.start().unwrap();
        let handle2 = node2.start().unwrap();
        let mut events1 = handle1.subscribe();

        // Connect and wait for mesh
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{port1}").parse().unwrap();
        handle2.dial(addr).unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Node2 publishes a block
        let block = Block {
            header: BlockHeader {
                height: 1,
                parent_hash: [0xAAu8; 32],
                global_state_root: [0xBBu8; 32],
                tx_root: [0xCCu8; 32],
                timestamp: 1_700_000_000,
                proposer: crate::core::account::Address::from([0xDDu8; 32]),
                chain_id: 0,
            },
            body: BlockBody { transactions: vec![] },
        };
        handle2.publish_block(block).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // Node1 should receive BlockReceived event
        match events1.try_recv() {
            Ok(P2pEvent::BlockReceived { .. }) => {} // success
            Ok(other) => panic!("expected BlockReceived, got {other:?}"),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                panic!("no BlockReceived event within timeout");
            }
            Err(e) => panic!("channel error: {e}"),
        }

        handle1.shutdown().await;
        handle2.shutdown().await;
    }

    #[tokio::test]
    async fn test_p2p_publish_vote_delivers_vote_event() {
        let port1 = pick_unused_port();
        let port2 = pick_unused_port();

        let cfg1 = P2pConfig { p2p_port: port1, ..Default::default() };
        let cfg2 = P2pConfig { p2p_port: port2, ..Default::default() };

        let node1 = P2pService::new(cfg1, 0).unwrap();
        let node2 = P2pService::new(cfg2, 0).unwrap();

        let handle1 = node1.start().unwrap();
        let handle2 = node2.start().unwrap();
        let mut events1 = handle1.subscribe();

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{port1}").parse().unwrap();
        handle2.dial(addr).unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Node2 publishes a vote
        use crate::crypto::falcon::Falcon512Signature;
        use crate::crypto::constants::FALCON_SIGNATURE_SIZE;
        let vote = CommitVote {
            height: 1,
            block_hash: [0xEEu8; 32],
            validator: crate::core::account::Address::from([0xFFu8; 32]),
            signature: Falcon512Signature::from_bytes(&[0xAAu8; FALCON_SIGNATURE_SIZE]).unwrap(),
        };
        handle2.publish_vote(vote).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        match events1.try_recv() {
            Ok(P2pEvent::VoteReceived { .. }) => {} // success
            Ok(other) => panic!("expected VoteReceived, got {other:?}"),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                panic!("no VoteReceived event within timeout");
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
