//! The MAP Wire Protocol
//!
//! This protocol is a purpose built MAP libp2p protocol. It's role is to facilitate
//! direct peer-to-peer communication primarily for sending/receiving chain information for
//! syncing.

use std::marker::PhantomData;
use std::time::Duration;

use futures::prelude::*;
use libp2p::{multiaddr::Multiaddr, PeerId};
use libp2p::core::ConnectedPoint;
use libp2p::swarm::{
    NetworkBehaviour, NetworkBehaviourAction, PollParameters, protocols_handler::ProtocolsHandler,
    SubstreamProtocol,
};
use slog::o;
use tokio::io::{AsyncRead, AsyncWrite};

use handler::P2PHandler;
pub use methods::{
    ErrorMessage, RequestId, ResponseTermination, P2PErrorResponse, P2PResponse, StatusMessage,
};
pub use protocol::{P2PError, P2PProtocol, P2PRequest};

pub(crate) mod codec;
mod handler;
pub mod methods;
mod protocol;

/// The return type used in the behaviour and the resultant event from the protocols handler.
#[derive(Debug)]
pub enum P2PEvent {
    /// An inbound/outbound request for P2P protocol. The first parameter is a sequential
    /// id which tracks an awaiting substream for the response.
    Request(RequestId, P2PRequest),
    /// A response that is being sent or has been received from the P2P protocol. The first parameter returns
    /// that which was sent with the corresponding request, the second is a single chunk of a
    /// response.
    Response(RequestId, P2PErrorResponse),
    /// An Error occurred.
    Error(RequestId, P2PError),
}

impl P2PEvent {
    pub fn id(&self) -> usize {
        match *self {
            P2PEvent::Request(id, _) => id,
            P2PEvent::Response(id, _) => id,
            P2PEvent::Error(id, _) => id,
        }
    }
}

impl std::fmt::Display for P2PEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            P2PEvent::Request(id, req) => write!(f, "P2P Request(id: {}, {})", id, req),
            P2PEvent::Response(id, res) => write!(f, "P2P Response(id: {}, {})", id, res),
            P2PEvent::Error(id, err) => write!(f, "P2P Request(id: {}, error: {:?})", id, err),
        }
    }
}

/// Implements the libp2p `NetworkBehaviour` trait and therefore manages network-level
/// logic.
pub struct P2P<TSubstream> {
    /// Queue of events to processed.
    events: Vec<NetworkBehaviourAction<P2PEvent, P2PMessage>>,
    /// Pins the generic substream.
    marker: PhantomData<TSubstream>,
    /// Slog logger for P2P behaviour.
    log: slog::Logger,
}

impl<TSubstream> P2P<TSubstream> {
    pub fn new(log: slog::Logger) -> Self {
        let log = log.new(o!("service" => "libp2p_p2p"));
        P2P {
            events: Vec::new(),
            marker: PhantomData,
            log,
        }
    }

    /// Submits an P2P request.
    ///
    /// The peer must be connected for this to succeed.
    pub fn send_rpc(&mut self, peer_id: PeerId, p2p_event: P2PEvent) {
        self.events.push(NetworkBehaviourAction::SendEvent {
            peer_id,
            event: p2p_event,
        });
    }
}

impl<TSubstream> NetworkBehaviour for P2P<TSubstream>
    where
        TSubstream: AsyncRead + AsyncWrite,
{
    type ProtocolsHandler = P2PHandler<TSubstream>;
    type OutEvent = P2PMessage;

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        P2PHandler::new(
            SubstreamProtocol::new(P2PProtocol),
            Duration::from_secs(30),
            &self.log,
        )
    }

    // handled by discovery
    fn addresses_of_peer(&mut self, _peer_id: &PeerId) -> Vec<Multiaddr> {
        Vec::new()
    }

    fn inject_connected(&mut self, peer_id: PeerId, connected_point: ConnectedPoint) {
        // if initialised the connection, report this upwards to send the HELLO request
        self.events.push(NetworkBehaviourAction::GenerateEvent(
            P2PMessage::InjectConnect(peer_id,connected_point),
        ));
    }

    fn inject_disconnected(&mut self, peer_id: &PeerId, _: ConnectedPoint) {
        // inform the p2p handler that the peer has disconnected
        self.events.push(NetworkBehaviourAction::GenerateEvent(
            P2PMessage::PeerDisconnected(peer_id.clone()),
        ));
    }

    fn inject_node_event(
        &mut self,
        source: PeerId,
        event: <Self::ProtocolsHandler as ProtocolsHandler>::OutEvent,
    ) {
        // send the event to the user
        self.events
            .push(NetworkBehaviourAction::GenerateEvent(P2PMessage::P2P(
                source, event,
            )));
    }

    fn poll(
        &mut self,
        _: &mut impl PollParameters,
    ) -> Async<
        NetworkBehaviourAction<
            <Self::ProtocolsHandler as ProtocolsHandler>::InEvent,
            Self::OutEvent,
        >,
    > {
        if !self.events.is_empty() {
            return Async::Ready(self.events.remove(0));
        }
        Async::NotReady
    }
}

/// Messages sent to the user from the P2P protocol.
#[derive(Debug)]
pub enum P2PMessage {
    P2P(PeerId, P2PEvent),
    InjectConnect(PeerId,ConnectedPoint),
    PeerDisconnected(PeerId),
}
