//! Provides network functionality for the Syncing thread. This fundamentally wraps a network
//! channel and stores a global P2P ID to perform requests.

use std::sync::{Arc, RwLock};

use libp2p::PeerId;
use slog::{debug, trace, warn};
use tokio::sync::mpsc;

use chain::blockchain::BlockChain;

use crate::handler_processor::status_message;
use crate::manager::NetworkMessage;
use crate::p2p::{methods::*, P2PEvent, P2PRequest, RequestId};

/// Wraps a Network channel to employ various P2P related network functionality for the Sync manager. This includes management of a global P2P request Id.

pub struct SyncNetworkContext {
    /// The network channel to relay messages to the Network service.
    network_send: mpsc::UnboundedSender<NetworkMessage>,

    request_id: RequestId,
    /// Logger for the `SyncNetworkContext`.
    log: slog::Logger,
}

impl SyncNetworkContext {
    pub fn new(network_send: mpsc::UnboundedSender<NetworkMessage>, log: slog::Logger) -> Self {
        Self {
            network_send,
            request_id: 0,
            log,
        }
    }

    pub fn status_peer(
        &mut self,
        chain: Arc<RwLock<BlockChain>>,
        peer_id: PeerId,
    ) {
        if let Some(status_message) = status_message(chain) {
            debug!(
                    self.log,
                    "Sending Status Request";
                    "peer" => format!("{:?}", peer_id),
                    "status_message" => format!("{:?}", status_message),
                );

            let _ = self.send_rpc_request(peer_id, P2PRequest::Status(status_message));
        }
    }

    pub fn blocks_by_range_request(
        &mut self,
        peer_id: PeerId,
        request: BlocksByRangeRequest,
    ) -> Result<RequestId, &'static str> {
        trace!(
            self.log,
            "Sending BlocksByRange Request";
            "method" => "BlocksByRange",
            "count" => request.count,
            "peer" => format!("{:?}", peer_id)
        );
        self.send_rpc_request(peer_id, P2PRequest::BlocksByRange(request))
    }

    pub fn blocks_by_hash_request(
        &mut self,
        peer_id: PeerId,
        request: BlocksByRootRequest,
    ) -> Result<RequestId, &'static str> {
        trace!(
            self.log,
            "Sending BlocksByRoot Request";
            "method" => "BlocksByRoot",
            "count" => request.block_roots.len(),
            "peer" => format!("{:?}", peer_id)
        );
        self.send_rpc_request(peer_id.clone(), P2PRequest::BlocksByRoot(request))
    }

    pub fn downvote_peer(&mut self, peer_id: PeerId) {
        debug!(
            self.log,
            "Peer downvoted";
            "peer" => format!("{:?}", peer_id)
        );
        // TODO: Implement reputation
        self.disconnect(peer_id, GoodbyeReason::Fault);
    }

    fn disconnect(&mut self, peer_id: PeerId, reason: GoodbyeReason) {
        warn!(
            &self.log,
            "Disconnecting peer (P2P)";
            "reason" => format!("{:?}", reason),
            "peer_id" => format!("{:?}", peer_id),
        );

        // ignore the error if the channel send fails
        let _ = self.send_rpc_request(peer_id.clone(), P2PRequest::Goodbye(reason));
        self.network_send
            .try_send(NetworkMessage::Disconnect { peer_id })
            .unwrap_or_else(|_| {
                warn!(
                    self.log,
                    "Could not send a Disconnect to the network service"
                )
            });
    }

    pub fn send_rpc_request(
        &mut self,
        peer_id: PeerId,
        rpc_request: P2PRequest,
    ) -> Result<RequestId, &'static str> {
        let request_id = self.request_id;
        self.request_id += 1;
        self.send_rpc_event(peer_id, P2PEvent::Request(request_id, rpc_request))?;
        Ok(request_id)
    }

    fn send_rpc_event(&mut self, peer_id: PeerId, rpc_event: P2PEvent) -> Result<(), &'static str> {
        self.network_send
            .try_send(NetworkMessage::P2P(peer_id, rpc_event))
            .map_err(|_| {
                debug!(
                    self.log,
                    "Could not send P2P message to the network service"
                );
                "Network channel send Failed"
            })
    }
}
