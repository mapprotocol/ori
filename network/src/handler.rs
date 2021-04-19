#![allow(clippy::unit_arg)]

use std::sync::{Arc, RwLock};

use futures::future::Future;
use futures::stream::Stream;
use libp2p::{gossipsub::MessageId, PeerId};
use slog::{debug, trace, warn};
use tokio::sync::mpsc;

use pool::tx_pool::TxPoolManager;
use chain::blockchain::BlockChain;
use map_core::transaction::Transaction;
use crate::{behaviour::PubsubMessage, manager::NetworkMessage};
use crate::error;
use crate::MessageProcessor;
use crate::p2p::{P2PError, P2PErrorResponse, P2PEvent, P2PRequest, P2PResponse, RequestId, ResponseTermination};

/// Handles messages received from the network and client and organises syncing. This
/// functionality of this struct is to validate an decode messages from the network before
/// passing them to the internal message processor. The message processor spawns a syncing thread
/// which manages which blocks need to be requested and processed.
pub struct MessageHandler {
    /// A channel to the network service to allow for gossip propagation.
    network_send: mpsc::UnboundedSender<NetworkMessage>,
    /// Processes validated and decoded messages from the network. Has direct access to the
    /// sync manager.
    message_processor: MessageProcessor,
    /// The `MessageHandler` logger.
    pub log: slog::Logger,
}

/// Types of messages the handler can receive.
#[derive(Debug)]
pub enum HandlerMessage {
    /// We have initiated a connection to a new peer.
    PeerDialed(PeerId),
    /// Peer has disconnected,
    PeerDisconnected(PeerId),
    /// An RPC response/request has been received.
    RPC(PeerId, P2PEvent),
    /// A gossip message has been received. The fields are: message id, the peer that sent us this
    /// message and the message itself.
    PubsubMessage(MessageId, PeerId, PubsubMessage),
}

impl MessageHandler {
    /// Initializes and runs the MessageHandler.
    pub fn spawn(
        block_chain: Arc<RwLock<BlockChain>>,
        network_send: mpsc::UnboundedSender<NetworkMessage>,
        tx_pool: Arc<RwLock<TxPoolManager>>,
        executor: &tokio::runtime::TaskExecutor,
        log: slog::Logger,
    ) -> error::Result<mpsc::UnboundedSender<HandlerMessage>> {
        trace!(log, "MessageHandler service starting");

        let (handler_send, handler_recv) = mpsc::unbounded_channel();

        // Initialise a message instance, which itself spawns the syncing thread.
        let message_processor =
            MessageProcessor::new(executor, block_chain, tx_pool, network_send.clone(), &log);

        // generate the Message handler
        let mut handler = MessageHandler {
            network_send,
            message_processor,
            log:log.clone(),
        };

        // spawn handler task and move the message handler instance into the spawned thread
        executor.spawn(
            handler_recv
                .for_each(move |msg| Ok(handler.handle_message(msg)))
                .map_err(move |_| {
                    debug!(log, "Network message handler terminated.");
                }),
        );

        Ok(handler_send)
    }

    /// Handle all messages incoming from the network service.
    fn handle_message(&mut self, message: HandlerMessage) {
        match message {
            // we have initiated a connection to a peer
            HandlerMessage::PeerDialed(peer_id) => {
                self.message_processor.on_connect(peer_id);
            }
            // A peer has disconnected
            HandlerMessage::PeerDisconnected(peer_id) => {
                self.message_processor.on_disconnect(peer_id);
            }
            // An RPC message request/response has been received
            HandlerMessage::RPC(peer_id, rpc_event) => {
                self.handle_rpc_message(peer_id, rpc_event);
            }
            // An RPC message request/response has been received
            HandlerMessage::PubsubMessage(id, peer_id, gossip) => {
                self.handle_gossip(id, peer_id, gossip);
            }
        }
    }

    /* RPC - Related functionality */

    /// Handle RPC messages
    fn handle_rpc_message(&mut self, peer_id: PeerId, rpc_message: P2PEvent) {
        match rpc_message {
            P2PEvent::Request(id, req) => self.handle_rpc_request(peer_id, id, req),
            P2PEvent::Response(id, resp) => self.handle_rpc_response(peer_id, id, resp),
            P2PEvent::Error(id, error) => self.handle_rpc_error(peer_id, id, error),
        }
    }

    /// A new RPC request has been received from the network.
    fn handle_rpc_request(&mut self, peer_id: PeerId, request_id: RequestId, request: P2PRequest) {
        match request {
            P2PRequest::Status(status_message) => {
                self.message_processor
                    .on_status_request(peer_id, request_id, status_message)
            }
            P2PRequest::Goodbye(goodbye_reason) => {
                debug!(
                    self.log, "PeerGoodbye";
                    "peer" => format!("{:?}", peer_id),
                    "reason" => format!("{:?}", goodbye_reason),
                );
                self.message_processor.on_disconnect(peer_id);
            }
            P2PRequest::BlocksByRange(request) => self
                .message_processor
                .on_blocks_by_range_request(peer_id, request_id, request),
            P2PRequest::BlocksByRoot(request) => {
                self.message_processor.on_blocks_by_root_request(peer_id, request_id, request);
            }
        }
    }

    /// An RPC response has been received from the network.
    // we match on id and ignore responses past the timeout.
    fn handle_rpc_response(
        &mut self,
        peer_id: PeerId,
        request_id: RequestId,
        error_response: P2PErrorResponse,
    ) {
        // an error could have occurred.
        match error_response {
            P2PErrorResponse::InvalidRequest(error) => {
                warn!(self.log, "Peer indicated invalid request";"peer_id" => format!("{:?}", peer_id), "error" => error.as_string());
                self.handle_rpc_error(peer_id, request_id, P2PError::P2PErrorResponse);
            }
            P2PErrorResponse::ServerError(error) => {
                warn!(self.log, "Peer internal server error";"peer_id" => format!("{:?}", peer_id), "error" => error.as_string());
                self.handle_rpc_error(peer_id, request_id, P2PError::P2PErrorResponse);
            }
            P2PErrorResponse::Unknown(error) => {
                warn!(self.log, "Unknown peer error";"peer" => format!("{:?}", peer_id), "error" => error.as_string());
                self.handle_rpc_error(peer_id, request_id, P2PError::P2PErrorResponse);
            }
            P2PErrorResponse::Success(response) => {
                match response {
                    P2PResponse::Status(status_message) => {
                        self.message_processor
                            .on_status_response(peer_id, status_message);
                    }
                    P2PResponse::BlocksByRange(response) => {
                        match bincode::deserialize(&response[..]) {
                            Ok(block) => {
                                self.message_processor.on_blocks_by_range_response(
                                    peer_id,
                                    request_id,
                                    Some(block),
                                );
                            }
                            Err(e) => {
                                // TODO: Down-vote Peer
                                warn!(self.log, "Peer sent invalid BEACON_BLOCKS response";"peer" => format!("{:?}", peer_id), "error" => format!("{:?}", e));
                            }
                        }
                    }
                    P2PResponse::BlocksByRoot(response) => {
                        match bincode::deserialize(&response[..]) {
                            Ok(block) => {
                                self.message_processor.on_blocks_by_root_response(
                                    peer_id,
                                    request_id,
                                    Some(block),
                                );
                            }
                            Err(e) => {
                                // TODO: Down-vote Peer
                                warn!(self.log, "Peer sent invalid BEACON_BLOCKS response";
                                    "peer" => format!("{:?}", peer_id), "error" => format!("{:?}", e));
                            }
                        }
                    }
                }
            }
            P2PErrorResponse::StreamTermination(response_type) => {
                // have received a stream termination, notify the processing functions
                match response_type {
                    ResponseTermination::BlocksByRange => {
                        self.message_processor
                            .on_blocks_by_range_response(peer_id, request_id, None);
                    }
                    ResponseTermination::BlocksByRoot => {
                        self.message_processor
                            .on_blocks_by_root_response(peer_id, request_id, None);
                    }
                }
            }
        }
    }
    /// Handle various RPC errors
    fn handle_rpc_error(&mut self, peer_id: PeerId, request_id: RequestId, error: P2PError) {
        warn!(self.log, "RPC Error"; "Peer" => format!("{:?}", peer_id), "request_id" => format!("{}", request_id), "Error" => format!("{:?}", error));
        self.message_processor.on_rpc_error(peer_id, request_id);
    }

    /// Handle RPC messages
    fn handle_gossip(&mut self, id: MessageId, peer_id: PeerId, gossip_message: PubsubMessage) {
        match gossip_message {
            PubsubMessage::Block(message) => match bincode::deserialize(&message[..]) {
                Ok(block) => {
                    let should_forward_on = self
                        .message_processor
                        .on_block_gossip(peer_id.clone(), block);
                    if should_forward_on {
                        self.propagate_message(id, peer_id);
                    }
                }
                Err(e) => {
                    debug!(self.log, "Invalid gossiped block"; "peer_id" => format!("{}", peer_id), "Error" => format!("{:?}", e));
                }
            },
            PubsubMessage::Transaction(message) => match bincode::deserialize::<Transaction>(&message) {
                Ok(tx) => {
                    // Received new transaction
                    debug!(self.log, "Gossip transaction received"; "peer_id" => format!("{}", peer_id),
                        "hash" => format!("{}", tx.hash()));
                    self.message_processor.on_transaction_gossip(peer_id.clone(), tx);
                },
                Err(e) => {
                    // Received new transaction
                    warn!(self.log, "Gossip transaction decoded error"; "peer_id" => format!("{}", peer_id),);
                },
            },
            PubsubMessage::Unknown(message) => {
                // Received a message from an unknown topic. Ignore for now
                debug!(self.log, "Unknown Gossip Message"; "peer_id" => format!("{}", peer_id), "Message" => format!("{:?}", message));
            },
        }
    }

    /// Informs the network service that the message should be forwarded to other peers.
    fn propagate_message(&mut self, message_id: MessageId, propagation_source: PeerId) {
        self.network_send
            .try_send(NetworkMessage::Propagate {
                propagation_source,
                message_id,
            })
            .unwrap_or_else(|_| {
                warn!(
                    self.log,
                    "Could not send propagation request to the network service"
                )
            });
    }
}
