use std::sync::{Arc, RwLock};

use libp2p::PeerId;
use slog::{debug, info, error, trace, warn};
use tokio::sync::{mpsc, oneshot};

use chain::blockchain::BlockChain;
use pool::tx_pool::TxPoolManager;
use map_core::block::Block;
use map_core::types::Hash;
use map_core::transaction::Transaction;

use crate::manager::NetworkMessage;
use crate::p2p::{methods::*, P2PEvent, P2PRequest, P2PResponse, RequestId};
use crate::sync::SyncMessage;
use priority_queue::PriorityQueue;
use crate::{
	{behaviour::{PubsubMessage}
	},
	GossipTopic,
};

/// If a block is more than `FUTURE_SLOT_TOLERANCE` slots ahead of our slot clock, we drop it.
/// Otherwise we queue it.
pub(crate) const FUTURE_SLOT_TOLERANCE: u64 = 1;
/// The number of slots ahead of us that is allowed before requesting a long-range (batch)  Sync
/// from a peer. If a peer is within this tolerance (forwards or backwards), it is treated as a
/// fully sync'd peer.
const SLOT_IMPORT_TOLERANCE: u64 = 20;

const SHOULD_FORWARD_GOSSIP_BLOCK: bool = true;
const SHOULD_NOT_FORWARD_GOSSIP_BLOCK: bool = false;
const QUEUE_GOSSIP_BLOCK: usize = 512;

/// Keeps track of syncing information for known connected peers.
#[derive(Clone, Copy, Debug)]
pub struct PeerSyncInfo {
    pub genesis_hash: Hash,

    /// Latest finalized root.
    pub finalized_root: Hash,

    /// Latest finalized number.
    pub finalized_number: u64,

    /// The latest block root.
    pub head_root: Hash,

    /// The fork version of the chain we are broadcasting.
    pub network_id: u16,
}

impl From<StatusMessage> for PeerSyncInfo {
    fn from(status: StatusMessage) -> PeerSyncInfo {
        PeerSyncInfo {
            network_id: status.network_id,
            finalized_root: status.finalized_root,
            finalized_number: status.finalized_number,
            head_root: status.head_root,
            genesis_hash: status.genesis_hash,
        }
    }
}

impl PeerSyncInfo {
    pub fn from_chain(chain: Arc<RwLock<BlockChain>>) -> Option<PeerSyncInfo> {
        Some(Self::from(status_message(chain)?))
    }
}

/// Processes validated messages from the network. It relays necessary data to the syncing thread
/// and processes blocks from the pubsub network.
pub struct MessageProcessor {
    /// A reference to the underlying beacon chain.
    chain: Arc<RwLock<BlockChain>>,
    /// Transaction pool
    tx_pool: Arc<RwLock<TxPoolManager>>,
    /// A channel to the syncing thread.
    sync_send: mpsc::UnboundedSender<SyncMessage>,
    /// A oneshot channel for destroying the sync thread.
    _sync_exit: oneshot::Sender<()>,
    /// A network context to return and handle RPC requests.
    network: HandlerNetworkContext,
    /// The `RPCHandler` logger.
    log: slog::Logger,
    pub queue :PriorityQueue<Block,i64>,
}

impl MessageProcessor {
    /// Instantiate a `MessageProcessor` instance
    pub fn new(
        executor: &tokio::runtime::TaskExecutor,
        block_chain: Arc<RwLock<BlockChain>>,
        tx_pool: Arc<RwLock<TxPoolManager>>,
        network_send: mpsc::UnboundedSender<NetworkMessage>,
        log: &slog::Logger,
    ) -> Self {

        // spawn the sync thread
        let (sync_send, _sync_exit) = crate::sync::manager::spawn(
            executor,
            block_chain.clone(),
            network_send.clone(),
            log.clone(),
        );

        MessageProcessor {
            chain: block_chain,
            tx_pool: tx_pool,
            sync_send,
            _sync_exit,
            network: HandlerNetworkContext::new(network_send, log.clone()),
            log: log.clone(),
            queue:PriorityQueue::with_capacity(QUEUE_GOSSIP_BLOCK),
        }
    }

    fn send_to_sync(&mut self, message: SyncMessage) {
        self.sync_send.try_send(message).unwrap_or_else(|_| {
            warn!(
                self.log,
                "Could not send message to the sync service";
            )
        });
    }

    /// Handle a peer disconnect.
    ///
    /// Removes the peer from the manager.
    pub fn on_disconnect(&mut self, peer_id: PeerId) {
        self.send_to_sync(SyncMessage::Disconnect(peer_id));
    }

    /// An error occurred during an RPC request. The state is maintained by the sync manager, so
    /// this function notifies the sync manager of the error.
    pub fn on_rpc_error(&mut self, peer_id: PeerId, request_id: RequestId) {
        self.send_to_sync(SyncMessage::RPCError(peer_id, request_id));
    }

    /// Handle the connection of a new peer.
    ///
    /// Sends a `Status` message to the peer.
    pub fn on_connect(&mut self, peer_id: PeerId) {
        if let Some(status_message) = status_message(self.chain.clone()) {
            debug!(
                self.log,
                "Sending Status Request";
                "peer" => format!("{:?}", peer_id),
                "status_message" => format!("{:?}", status_message),
            );
            self.network
                .send_rpc_request(peer_id, P2PRequest::Status(status_message));
        }
    }

    /// Handle a `Status` request.
    ///
    /// Processes the `Status` from the remote peer and sends back our `Status`.
    pub fn on_status_request(
        &mut self,
        peer_id: PeerId,
        request_id: RequestId,
        status: StatusMessage,
    ) {
        debug!(
            self.log,
            "Received Status Request";
            "peer" => format!("{:?}", peer_id),
            "status" => format!("{:?}", status),
        );

        // ignore status responses if we are shutting down
        if let Some(status_message) = status_message(self.chain.clone()) {
            // Say status back.
            self.network.send_rpc_response(
                peer_id.clone(),
                request_id,
                P2PResponse::Status(status_message),
            );
        }

        self.process_status(peer_id, status);
    }

    /// Process a `Status` response from a peer.
    pub fn on_status_response(&mut self, peer_id: PeerId, status: StatusMessage) {
        trace!(self.log, "StatusResponse"; "peer" => format!("{:?}", peer_id));

        // Process the status message, without sending back another status.
        self.process_status(peer_id, status);
    }

    /// Process a `Status` message, requesting new blocks if appropriate.
    ///
    /// Disconnects the peer if required.
    fn process_status(&mut self, peer_id: PeerId, status: StatusMessage) {
        let remote = PeerSyncInfo::from(status);
        let local = match PeerSyncInfo::from_chain(self.chain.clone()) {
            Some(local) => local,
            None => {
                return error!(
                    self.log,
                    "Failed to get peer sync info";
                    "msg" => "likely due to head lock contention"
                );
            }
        };

        if local.network_id != remote.network_id {
            // The node is on a different network/fork, disconnect them.
            debug!(
                self.log, "Handshake Failure";
                "peer" => format!("{:?}", peer_id),
                "reason" => "network_id"
            );

            self.network
                .disconnect(peer_id, GoodbyeReason::IrrelevantNetwork);
        } else if remote.finalized_number < local.finalized_number {
            // The node has a lower finalized epoch, their chain is not useful to us. There are two
            // cases where a node can have a lower finalized epoch:
            //
            // ## The node is on the same chain
            //
            // If a node is on the same chain but has a lower finalized epoch, their head must be
            // lower than ours. Therefore, we have nothing to request from them.
            //
            // ## The node is on a fork
            //
            // If a node is on a fork that has a lower finalized epoch, switching to that fork would
            // cause us to revert a finalized block. This is not permitted, therefore we have no
            // interest in their blocks.
            debug!(
                self.log,
                "NaivePeer";
                "peer" => format!("{:?}", peer_id),
                "reason" => "lower finalized epoch"
            );
        } else {
            // The remote node has an equal or great finalized epoch and we don't know it's head.
            //
            // Therefore, there are some blocks between the local finalized epoch and the remote
            // head that are worth downloading.
            debug!(
                self.log, "UsefulPeer";
                "peer" => format!("{:?}", peer_id),
                "local_finalized_epoch" => local.finalized_number,
                "remote_latest_finalized_epoch" => remote.finalized_number,
            );
            self.send_to_sync(SyncMessage::AddPeer(peer_id, remote));
        }
    }

    /// Handle a `BlocksByRange` request from the peer.
    pub fn on_blocks_by_range_request(
        &mut self,
        peer_id: PeerId,
        request_id: RequestId,
        req: BlocksByRangeRequest,
    ) {
        info!(
            self.log,
            "Received BlocksByRange Request";
            "peer" => format!("{:?}", peer_id),
            "count" => req.count,
            "start_slot" => req.start_slot,
            "step" => req.step,
        );

        if req.step == 0 {
            warn!(self.log,
                "Peer sent invalid range request";
                "error" => "Step sent was 0");
            self.network.disconnect(peer_id, GoodbyeReason::Fault);
            return;
        }

        let mut blocks = vec![];
        let block_chain = self.chain.write().unwrap();
        let current_block = block_chain.current_block();
        let mut start = req.start_slot;
        loop {
            if current_block.height() > start && blocks.len() == req.count as usize {
                break;
            }
            let block = block_chain.get_block_by_number(start);
            match block {
                Some(b) => {
                    blocks.push(b.clone());
                    self.network.send_rpc_response(
                        peer_id.clone(),
                        request_id,
                        P2PResponse::BlocksByRange(bincode::serialize(&b).unwrap()),
                    );
                }
                None => {
                    warn!(self.log, "can't get block over"; "start"=> start);
                    break;
                }
            }
            start = start + req.step
        }

        debug!(
                self.log,
                "Sending BlocksByRange Response";
                "peer" => format!("{:?}", peer_id),
                "start_slot" => req.start_slot,
                "requested" => req.count,
                "returned" => start-req.start_slot);

        // send the stream terminator
        self.network.send_rpc_error_response(
            peer_id,
            request_id,
            P2PErrorResponse::StreamTermination(ResponseTermination::BlocksByRange),
        );
    }

    /// Handle a `BlocksByRange` response from the peer.
    /// A `beacon_block` behaves as a stream which is terminated on a `None` response.
    pub fn on_blocks_by_range_response(
        &mut self,
        peer_id: PeerId,
        request_id: RequestId,
        beacon_block: Option<Block>,
    ) {
        let beacon_block = beacon_block.map(Box::new);
        self.send_to_sync(SyncMessage::BlocksByRangeResponse {
            peer_id,
            request_id,
            beacon_block,
        });
    }

    /// Process a block message response
    pub fn on_blocks_by_root_response(
        &mut self,
        peer_id: PeerId,
        request_id: RequestId,
        block: Option<Block>,
    ) {
        let block = block.map(Box::new);
        trace!(
            self.log,
            "Received BlocksByRoot Response";
            "peer" => format!("{:?}", peer_id),
        );

        self.send_to_sync(SyncMessage::BlocksByHashResponse {
            peer_id,
            request_id,
            block,
        });
    }

    /// Response to `BlocksByRoot` request from a peer.
    pub fn on_blocks_by_root_request(
        &mut self,
        peer_id: PeerId,
        request_id: RequestId,
        request: BlocksByRootRequest,
    ) {

        let block_chain = self.chain.read().unwrap();

        for root in request.block_roots.iter() {
            if let Some(b) = block_chain.get_block(*root) {
                self.network.send_rpc_response(
                    peer_id.clone(),
                    request_id,
                    P2PResponse::BlocksByRoot(bincode::serialize(&b).unwrap()),
                );
            } else {
                debug!(
                    self.log,
                    "Peer requested unknown hash";
                    "peer" => format!("{:?}", peer_id),
                    "request_root" => format!("{:}", root),
                );
            }
        }
        // Notify streamd terminator
        self.network.send_rpc_error_response(
            peer_id,
            request_id,
            P2PErrorResponse::StreamTermination(ResponseTermination::BlocksByRoot),
        );
    }

    /// Process a gossip message declaring a new block.
    ///
    /// Attempts to apply to block to the beacon chain. May queue the block for later processing.
    ///
    /// Returns a `bool` which, if `true`, indicates we should forward the block to our peers.
    pub fn on_block_gossip(
        &mut self,
        peer_id: PeerId,
        block: Block,
    ) -> bool {
        let current_block = self.chain.read().unwrap().current_block();
		// when start, receive first block height than local, start sync meanwhile cache latest block. Due to when sync over, if no cache block
		// will repeat again with syner need more time. so check receive block and cache block. if it match current + 1 will insert.
		// if lower will lost, if greater will cache, this reduce frequency.
		let height :u64= block.height();
		let current_height = current_block.height();
		debug!(self.log, "Gossip block received: {:?} {:?} current: {:?} {:?}", height, block.hash(), current_height, current_block.hash());

		let mut find = false;
		if height - current_height < QUEUE_GOSSIP_BLOCK as u64 {
			self.queue.push(block.clone(),-(height as i64));

			while !self.queue.is_empty() {
				let (block_low,height_low_neg) = self.queue.pop().unwrap();
				let height_low = (-height_low_neg) as u64;

				if  height_low > current_height + 1 {
					self.queue.push(block_low.clone(),height_low_neg);

					if  height > current_height + SLOT_IMPORT_TOLERANCE {
						warn!(self.log, "unknown gossip parent: {:?} {:?}", height, current_height);
						self.send_to_sync(SyncMessage::OrphanBlock(peer_id.clone(), Box::new(block.clone())));
					}
					break
				} else if height_low == current_height + 1 {
					if import_block(self.chain.clone(),&block_low) {
						if height_low == height {
							find = true;
						} else {
							debug!(self.log, "Broadcast block received: {:?} {:?} height_low: {:?} {:?} queue {:?}", block.height(), block.hash(), height_low, block_low.hash(),self.queue.len());
							self.network.broadcast_block(&block_low);
						}
					} else {
						break
					};
				}
			}
		}

		find
	}

    pub fn on_transaction_gossip(
        &mut self,
        peer_id: PeerId,
        tx: Transaction,
    ) -> bool {
        if self.tx_pool.write().expect("acquiring tx_pool write_lock").add_tx(tx.clone()) {
            return true;
        }

        false
    }

}

fn import_block(chain: Arc<RwLock<BlockChain>>, block: &Block) -> bool{
	let broadcast = match chain.write().expect("").import_block(block) {
		Ok(_) => {
			true
		}
		Err(e) => {
			println!("network insert_block,Error: {:?}", e);
			false
		}
	};
	return broadcast
}

/// Build a `StatusMessage` representing the state of the given `block_chain`.
pub(crate) fn status_message(
    block_chain: Arc<RwLock<BlockChain>>,
) -> Option<StatusMessage> {
    let block = block_chain.read().unwrap().current_block();
    Some(StatusMessage {
        genesis_hash: block_chain.read().unwrap().genesis_hash(),
        finalized_root: block.hash(),
        finalized_number: block.height(),
        head_root: block.hash(),
        network_id: 31133,
    })
}

/// Wraps a Network Channel to employ various RPC related network functionality for the message
/// handler. The handler doesn't manage it's own request Id's and can therefore only send
/// responses or requests with 0 request Ids.
pub struct HandlerNetworkContext {
    /// The network channel to relay messages to the Network service.
    network_send: mpsc::UnboundedSender<NetworkMessage>,
    /// Logger for the `NetworkContext`.
    log: slog::Logger,
}

impl HandlerNetworkContext {
    pub fn new(network_send: mpsc::UnboundedSender<NetworkMessage>, log: slog::Logger) -> Self {
        Self { network_send, log }
    }

    pub fn disconnect(&mut self, peer_id: PeerId, reason: GoodbyeReason) {
        warn!(
            &self.log,
            "Disconnecting peer (RPC)";
            "reason" => format!("{:?}", reason),
            "peer_id" => format!("{:?}", peer_id),
        );
        self.send_rpc_request(peer_id.clone(), P2PRequest::Goodbye(reason));
        self.network_send
            .try_send(NetworkMessage::Disconnect { peer_id })
            .unwrap_or_else(|_| {
                warn!(
                    self.log,
                    "Could not send a Disconnect to the network service"
                )
            });
    }

    pub fn send_rpc_request(&mut self, peer_id: PeerId, rpc_request: P2PRequest) {
        // the message handler cannot send requests with ids. Id's are managed by the sync
        // manager.
        let request_id = 0;
        self.send_rpc_event(peer_id, P2PEvent::Request(request_id, rpc_request));
    }

    /// Convenience function to wrap successful RPC Responses.
    pub fn send_rpc_response(
        &mut self,
        peer_id: PeerId,
        request_id: RequestId,
        rpc_response: P2PResponse,
    ) {
        self.send_rpc_event(
            peer_id,
            P2PEvent::Response(request_id, P2PErrorResponse::Success(rpc_response)),
        );
    }

    /// Send an P2PErrorResponse. This handles errors and stream terminations.
    pub fn send_rpc_error_response(
        &mut self,
        peer_id: PeerId,
        request_id: RequestId,
        rpc_error_response: P2PErrorResponse,
    ) {
        self.send_rpc_event(peer_id, P2PEvent::Response(request_id, rpc_error_response));
    }

    fn send_rpc_event(&mut self, peer_id: PeerId, rpc_event: P2PEvent) {
        self.network_send
            .try_send(NetworkMessage::P2P(peer_id, rpc_event))
            .unwrap_or_else(|_| {
                warn!(
                    self.log,
                    "Could not send P2P message to the network service"
                )
            });
    }

	pub fn broadcast_block(&mut self, data: &Block) {
		// Broadcast sealed block to the network
		let topic = GossipTopic::MapBlock;
		let message = PubsubMessage::Block(bincode::serialize(data).unwrap());
		self.network_send
			.try_send(NetworkMessage::Publish {
				topics: vec![topic.into()],
				message,
			})
			.unwrap_or_else(|_| warn!(self.log, "Could not send gossip sealed block."));
	}

}
