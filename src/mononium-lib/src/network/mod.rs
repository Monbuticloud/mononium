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
// P2pService
// ---------------------------------------------------------------------------

pub struct P2pService {
    swarm: Swarm<MononiumBehaviour>,
    local_peer_id: PeerId,
    chain_id: u64,
    peer_scores: PeerScoreRepo,
    shutdown_rx: Option<mpsc::Receiver<()>>,
    shutdown_tx: mpsc::Sender<()>,
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

        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Ok(Self {
            swarm,
            local_peer_id,
            chain_id,
            peer_scores: PeerScoreRepo::new(),
            shutdown_rx: Some(shutdown_rx),
            shutdown_tx,
        })
    }

    #[must_use]
    pub fn local_peer_id(&self) -> &PeerId { &self.local_peer_id }

    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", constants::DEFAULT_P2P_PORT)
            .parse()?;
        self.swarm.listen_on(listen_addr)?;

        for topic in TopicConfig::standard_topics(self.chain_id) {
            let gt = libp2p::gossipsub::IdentTopic::new(topic.name);
            self.swarm.behaviour_mut().gossipsub.subscribe(&gt)?;
            info!("subscribed to: {gt}");
        }
        Ok(())
    }

    pub async fn run(&mut self) {
        use libp2p::futures::StreamExt;
        let mut shutdown_rx = self.shutdown_rx.take().unwrap();
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => self.handle_event(event),
                _ = shutdown_rx.recv() => {
                    info!("P2P shutting down");
                    break;
                }
            }
        }
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

    pub async fn publish_tx(&mut self, txs: &[Transaction]) -> Result<MessageId, Box<dyn std::error::Error>> {
        let data = GossipMessage::Txs(txs.to_vec()).encode();
        let topic = libp2p::gossipsub::IdentTopic::new(format!("mononium/txs/{}", self.chain_id));
        Ok(self.swarm.behaviour_mut().gossipsub.publish(topic, data)?)
    }

    pub async fn publish_block(&mut self, block: &Block) -> Result<MessageId, Box<dyn std::error::Error>> {
        let data = GossipMessage::Block(Box::new(block.clone())).encode();
        let topic = libp2p::gossipsub::IdentTopic::new(format!("mononium/blocks/{}", self.chain_id));
        Ok(self.swarm.behaviour_mut().gossipsub.publish(topic, data)?)
    }

    pub async fn publish_vote(&mut self, vote: &CommitVote) -> Result<MessageId, Box<dyn std::error::Error>> {
        let data = GossipMessage::Vote(vote.clone()).encode();
        let topic = libp2p::gossipsub::IdentTopic::new(format!("mononium/votes/{}", self.chain_id));
        Ok(self.swarm.behaviour_mut().gossipsub.publish(topic, data)?)
    }

    pub async fn publish_evidence(&mut self, evidence: &EquivocationEvidence) -> Result<MessageId, Box<dyn std::error::Error>> {
        let data = GossipMessage::Evidence(Box::new(evidence.clone())).encode();
        let topic = libp2p::gossipsub::IdentTopic::new(format!("mononium/evidence/{}", self.chain_id));
        Ok(self.swarm.behaviour_mut().gossipsub.publish(topic, data)?)
    }

    pub fn stop(&self) {
        let _ = self.shutdown_tx.try_send(());
    }
}
