use std::num::NonZeroU32;
use std::time::Duration;

use futures::prelude::*;
use libp2p::{
    core::{ConnectedPoint, identity::Keypair},
    gossipsub::{Gossipsub, GossipsubConfigBuilder, GossipsubEvent, GossipsubMessage, MessageId},
    identify::{Identify, IdentifyEvent},
    kad::{Addresses, GetClosestPeersError, Kademlia, KademliaConfig, KademliaEvent},
    kad::record::store::MemoryStore,
    mdns::{Mdns, MdnsEvent},
    NetworkBehaviour,
    PeerId, ping::{Ping, PingConfig, PingEvent, PingFailure, PingSuccess},
    swarm::{NetworkBehaviourAction, NetworkBehaviourEventProcess},
    tokio_io::{AsyncRead, AsyncWrite},
};
use lru::LruCache;
use sha2::{Digest, Sha256};
use slog::{debug, o};

use crate::{error};
use crate::{GossipTopic, Topic, TopicHash};
use crate::p2p::{P2P, P2PEvent, P2PMessage};

const MAX_IDENTIFY_ADDRESSES: usize = 20;

/// Builds the network behaviour that manages the core protocols of map.
/// This core behaviour is managed by `Behaviour` which adds peer management to all core
/// behaviours.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourEvent", poll_method = "poll")]
pub struct Behaviour<TSubstream: AsyncRead + AsyncWrite> {
    /// The routing pub-sub mechanism for map.
    gossipsub: Gossipsub<TSubstream>,
    /// The map P2P specified in the wire-0 protocol.
    p2p: P2P<TSubstream>,
    /// Keep regular connection to peers and disconnect if absent.
    ping: Ping<TSubstream>,
    mdns: Mdns<TSubstream>,
    kademlia: Kademlia<TSubstream, MemoryStore>,
    /// Provides IP addresses and peer information.
    identify: Identify<TSubstream>,
    #[behaviour(ignore)]
    /// The events generated by this behaviour to be consumed in the swarm poll.
    events: Vec<BehaviourEvent>,
    /// Logger for behaviour actions.
    #[behaviour(ignore)]
    log: slog::Logger,
    /// A cache of recently seen gossip messages. This is used to filter out any possible
    /// duplicates that may still be seen over gossipsub.
    #[behaviour(ignore)]
    seen_gossip_messages: LruCache<MessageId, ()>,
}

impl<TSubstream: AsyncRead + AsyncWrite> Behaviour<TSubstream> {
    pub fn new(
        local_key: &Keypair,
        log: &slog::Logger,
    ) -> error::Result<Self> {
        let local_peer_id = local_key.public().into_peer_id();
        let behaviour_log = log.new(o!());
        let ping_config = PingConfig::new()
            .with_timeout(Duration::from_secs(30))
            .with_interval(Duration::from_secs(20))
            .with_max_failures(NonZeroU32::new(2).expect("2 != 0"))
            .with_keep_alive(false);

        let identify = Identify::new(
            "map/p2p".into(),
            "0.1".to_string(),
            local_key.public(),
        );


        // Create a Kademlia behaviour.
        let mut cfg = KademliaConfig::default();
        cfg.set_query_timeout(Duration::from_secs(5 * 60));
        let store = MemoryStore::new(local_peer_id.clone());
        let kademlia = Kademlia::with_config(local_peer_id.clone(), store, cfg);
        // behaviour.add_address(&"QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ".parse().unwrap(), "/ip4/104.131.131.82/tcp/4001".parse().unwrap());

        // The function used to generate a gossipsub message id
        // We use base64(SHA256(data)) for content addressing
        let gossip_message_id = |message: &GossipsubMessage| {
            MessageId(base64::encode_config(
                &Sha256::digest(&message.data),
                base64::URL_SAFE,
            ))
        };

        Ok(Behaviour {
            gossipsub: Gossipsub::new(local_peer_id, GossipsubConfigBuilder::new()
                .max_transmit_size(1_048_576)
                .manual_propagation() // require validation before propagation
                .no_source_id()
                .message_id_fn(gossip_message_id)
                .heartbeat_interval(Duration::from_secs(20))
                .build()),
            p2p: P2P::new(log.clone()),
            ping: Ping::new(ping_config),
            mdns: Mdns::new().expect("Failed to create mDNS service"),
            kademlia,
            identify,
            events: Vec::new(),
            log: behaviour_log,
            seen_gossip_messages: LruCache::new(100_000),
        })
    }
}

// Implement the NetworkBehaviourEventProcess trait so that we can derive NetworkBehaviour for Behaviour
impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<GossipsubEvent>
for Behaviour<TSubstream>
{
    fn inject_event(&mut self, event: GossipsubEvent) {
        // println!("inject_event gossipsub:  {:?}", event);
        match event {
            GossipsubEvent::Message(propagation_source, id, gs_msg) => {
                debug!(self.log, "Message received"; "id" => format!("{:?}", id));

                let msg = PubsubMessage::from_topics(&gs_msg.topics, gs_msg.data);

                // Note: We are keeping track here of the peer that sent us the message, not the
                // peer that originally published the message.
                if self.seen_gossip_messages.put(id.clone(), ()).is_none() {
                    // if this message isn't a duplicate, notify the network
                    self.events.push(BehaviourEvent::GossipMessage {
                        id,
                        source: propagation_source,
                        topics: gs_msg.topics,
                        message: msg,
                    });
                } else {
                    debug!(self.log, "A duplicate message was received"; "message" => format!("{:?}", msg));
                }
            }

            GossipsubEvent::Subscribed { peer_id, topic } => {
                println!(
                    "gossipsub: peer_id {} topic {:?}",
                    peer_id.to_base58(),
                    topic
                );
            }
            GossipsubEvent::Unsubscribed { .. } => {}
        }
    }
}

impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<P2PMessage>
for Behaviour<TSubstream>
{
    fn inject_event(&mut self, event: P2PMessage) {
        // println!("inject_event P2PMessage:  {:?}", event);
        match event {
            P2PMessage::InjectConnect(peer_id,connected_point) => {
                self.events.push(BehaviourEvent::InjectConnect(peer_id,connected_point))
            }
            P2PMessage::PeerDisconnected(peer_id) => {
                self.events.push(BehaviourEvent::PeerDisconnected(peer_id))
            }
            P2PMessage::P2P(peer_id, rpc_event) => {
                self.events.push(BehaviourEvent::RPC(peer_id, rpc_event))
            }
        }
    }
}

impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<PingEvent>
for Behaviour<TSubstream>
{
    fn inject_event(&mut self, event: PingEvent) {
        match event {
            PingEvent {
                peer,
                result: Result::Ok(PingSuccess::Ping { rtt }),
            } => {}
            PingEvent {
                peer,
                result: Result::Ok(PingSuccess::Pong),
            } => {}
            PingEvent {
                peer,
                result: Result::Err(PingFailure::Timeout),
            } => {
                println!("ping: timeout to {}", peer.to_base58());
            }
            PingEvent {
                peer,
                result: Result::Err(PingFailure::Other { error }),
            } => {
                println!("ping: failure with {}: {}", peer.to_base58(), error);
            }
        }
    }
}

impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<MdnsEvent>
for Behaviour<TSubstream>
{
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(list) => {
                for (peer_id, multiaddr) in list {
                    self.kademlia.add_address(&peer_id, multiaddr);
                }
            }
            MdnsEvent::Expired(list) => {
                for (peer, _) in list {
                    println!("inject_event Expired {:?}", peer);
                    if !self.mdns.has_node(&peer) {}
                }
            }
        }
    }
}

impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<KademliaEvent>
for Behaviour<TSubstream>
{
    // Called when `kademlia` produces an event.
    fn inject_event(&mut self, message: KademliaEvent) {
        println!("KademliaEvent inject_event( {:?} ", message);
        match message {
            KademliaEvent::GetClosestPeersResult(res) => {
                match res {
                    Ok(ok) => {
                        if !ok.peers.is_empty() {
                            println!("Query finished with closest peers: {:#?}", ok.peers);
                        } else {
                            // The example is considered failed as there
                            // should always be at least 1 reachable peer.
                            println!("Query finished with no closest peers.");
                        }
                    }
                    Err(GetClosestPeersError::Timeout { peers, .. }) => {
                        if !peers.is_empty() {
                            println!("Query timed out with closest peers: {:#?}", peers);
                        } else {
                            // The example is considered failed as there
                            // should always be at least 1 reachable peer.
                            println!("Query timed out with no closest peers.");
                        }
                    }
                }
            }
            KademliaEvent::RoutingUpdated { peer, addresses, .. } => {
                self.events.push(BehaviourEvent::FindPeers {
                    peer_id: peer,
                    addrs: addresses,
                });
            }
            _ => {
                println!("KademliaEvent inject_event else ");
            }
        }
    }
}

impl<TSubstream: AsyncRead + AsyncWrite> Behaviour<TSubstream> {
    /// Consumes the events list when polled.
    fn poll<TBehaviourIn>(
        &mut self,
    ) -> Async<NetworkBehaviourAction<TBehaviourIn, BehaviourEvent>> {
        if !self.events.is_empty() {
            return Async::Ready(NetworkBehaviourAction::GenerateEvent(self.events.remove(0)));
        }

        Async::NotReady
    }
}

impl<TSubstream: AsyncRead + AsyncWrite> NetworkBehaviourEventProcess<IdentifyEvent>
for Behaviour<TSubstream>
{
    fn inject_event(&mut self, event: IdentifyEvent) {
        // println!("inject_event IdentifyEvent:  {:?}", event);
        match event {
            IdentifyEvent::Received {
                peer_id,
                mut info,
                observed_addr,
            } => {
                if info.listen_addrs.len() > MAX_IDENTIFY_ADDRESSES {
                    debug!(
                        self.log,
                        "More than 20 addresses have been identified, truncating"
                    );
                    info.listen_addrs.truncate(MAX_IDENTIFY_ADDRESSES);
                }
                debug!(self.log, "Identified Peer"; "peer" => format!("{}", peer_id),
                "protocol_version" => info.protocol_version,
                "agent_version" => info.agent_version,
                "listening_ addresses" => format!("{:?}", info.listen_addrs),
                "observed_address" => format!("{:?}", observed_addr),
                "protocols" => format!("{:?}", info.protocols)
                );
            }
            IdentifyEvent::Sent { .. } => {}
            IdentifyEvent::Error { .. } => {}
        }
    }
}

/// Implements the combined behaviour for the libp2p service.
impl<TSubstream: AsyncRead + AsyncWrite> Behaviour<TSubstream> {
    /* Pubsub behaviour functions */

    /// Subscribes to a gossipsub topic.
    pub fn subscribe(&mut self, topic: Topic) -> bool {
        self.gossipsub.subscribe(topic)
    }

    /// Publishes a message on the pubsub (gossipsub) behaviour.
    pub fn publish(&mut self, topics: &[Topic], message: PubsubMessage) {
        let message_data = message.into_data();
        for topic in topics {
            self.gossipsub.publish(topic, message_data.clone());
        }
    }

    /// Publishes a message on the pubsub (gossipsub) behaviour.
    pub fn query_kad(&mut self, to_search: PeerId) {
        println!("Searching for the closest peers to {:?}", to_search);
        self.kademlia.get_closest_peers(to_search);
    }

    /// Forwards a message that is waiting in gossipsub's mcache. Messages are only propagated
/// once validated by the beacon chain.
    pub fn propagate_message(&mut self, propagation_source: &PeerId, message_id: MessageId) {
        self.gossipsub
            .propagate_message(&message_id, propagation_source);
    }

    /// Sends an p2p Request/Response via the p2p protocol.
    pub fn send_rpc(&mut self, peer_id: PeerId, p2p_event: P2PEvent) {
        self.p2p.send_rpc(peer_id, p2p_event);
    }

    /// Notify discovery that the peer has been banned.
    pub fn inject_disconnected(&mut self, id: &PeerId, old_endpoint: ConnectedPoint) {
        // self.kademlia.inject_disconnected(id,old_endpoint);
        println!("kademlia.inject_disconnected");
    }
}

/// The types of events than can be obtained from polling the behaviour.
pub enum BehaviourEvent {
    /// A received RPC event and the peer that it was received from.
    RPC(PeerId, P2PEvent),
    /// We have completed an initial connection to a new peer.
    InjectConnect(PeerId, ConnectedPoint),
    /// A peer has disconnected.
    PeerDisconnected(PeerId),
    /// A gossipsub message has been received.
    GossipMessage {
        /// The gossipsub message id. Used when propagating blocks after validation.
        id: MessageId,
        /// The peer from which we received this message, not the peer that published it.
        source: PeerId,
        /// The topics that this message was sent on.
        topics: Vec<TopicHash>,
        /// The message itself.
        message: PubsubMessage,
    },
    FindPeers {
        peer_id: PeerId,
        addrs: Addresses,
    },
}

/// Messages that are passed to and from the pubsub (Gossipsub) behaviour. These are encoded and
/// decoded upstream.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PubsubMessage {
    /// Gossipsub message providing notification of a new block.
    Block(Vec<u8>),
    /// Transaction message providing notification of a new external transaction.
    Transaction(Vec<u8>),
    /// Gossipsub message from an unknown topic.
    Unknown(Vec<u8>),
}

impl PubsubMessage {
    /* Note: This is assuming we are not hashing topics. If we choose to hash topics, these will
     * need to be modified.
     *
     * Also note that a message can be associated with many topics. As soon as one of the topics is
     * known we match. If none of the topics are known we return an unknown state.
     */
    fn from_topics(topics: &[TopicHash], data: Vec<u8>) -> Self {
        for topic in topics {
            match GossipTopic::from(topic.as_str()) {
                GossipTopic::MapBlock => return PubsubMessage::Block(data),
                GossipTopic::Transaction => return PubsubMessage::Transaction(data),
                GossipTopic::Shard => return PubsubMessage::Unknown(data),
                GossipTopic::Unknown(_) => continue,
            }
        }
        PubsubMessage::Unknown(data)
    }

    fn into_data(self) -> Vec<u8> {
        match self {
            PubsubMessage::Block(data)
            | PubsubMessage::Transaction(data)
            | PubsubMessage::Unknown(data) => data,
        }
    }
}
