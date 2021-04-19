//! Available P2P methods types and ids.

use serde::{Serialize, Deserialize};
use map_core::types::Hash;

/* Request/Response data structures for P2P methods */

/* Requests */

pub type RequestId = usize;

/// The STATUS request/response handshake message.
#[derive(Serialize, Deserialize,Clone, Debug, PartialEq)]
pub struct StatusMessage {
    /// The fork version of the chain we are broadcasting.
    pub genesis_hash: Hash,

    /// Latest finalized root.
    pub finalized_root: Hash,

    /// Latest finalized number.
    pub finalized_number: u64,

    /// The latest block root.
    pub head_root: Hash,

    /// The slot associated with the latest block root.
    pub network_id: u16,
}

/// The reason given for a `Goodbye` message.
///
/// Note: any unknown `u64::into(n)` will resolve to `Goodbye::Unknown` for any unknown `n`,
/// however `GoodbyeReason::Unknown.into()` will go into `0_u64`. Therefore de-serializing then
/// re-serializing may not return the same bytes.
#[derive(Debug, Clone, PartialEq,Serialize, Deserialize)]
pub enum GoodbyeReason {
    /// This node has shutdown.
    ClientShutdown = 1,

    /// Incompatible networks.
    IrrelevantNetwork = 2,

    /// Error/fault in the P2P.
    Fault = 3,

    /// Unknown reason.
    Unknown = 0,
}

impl From<u64> for GoodbyeReason {
    fn from(id: u64) -> GoodbyeReason {
        match id {
            1 => GoodbyeReason::ClientShutdown,
            2 => GoodbyeReason::IrrelevantNetwork,
            3 => GoodbyeReason::Fault,
            _ => GoodbyeReason::Unknown,
        }
    }
}

impl Into<u64> for GoodbyeReason {
    fn into(self) -> u64 {
        self as u64
    }
}

/// Request a number of beacon block roots from a peer.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BlocksByRangeRequest {
    /// The hash tree root of a block on the requested chain.
    pub head_block_root: Hash,

    /// The starting slot to request blocks.
    pub start_slot: u64,

    /// The number of blocks from the start slot.
    pub count: u64,

    /// The step increment to receive blocks.
    ///
    /// A value of 1 returns every block.
    /// A value of 2 returns every second block.
    /// A value of 3 returns every third block and so on.
    pub step: u64,
}

/// Request a number of beacon block bodies from a peer.
#[derive(Serialize, Deserialize,Clone, Debug, PartialEq)]
pub struct BlocksByRootRequest {
    /// The list of beacon block bodies being requested.
    pub block_roots: Vec<Hash>,
}

/* P2P Handling and Grouping */
// Collection of enums and structs used by the Codecs to encode/decode P2P messages

#[derive(Serialize, Deserialize,Debug, Clone, PartialEq)]
pub enum P2PResponse {
    /// A HELLO message.
    Status(StatusMessage),

    /// A response to a get BLOCKS_BY_RANGE request. A None response signifies the end of the
    /// batch.
    BlocksByRange(Vec<u8>),

    /// A response to a get BLOCKS_BY_ROOT request.
    BlocksByRoot(Vec<u8>),
}

/// Indicates which response is being terminated by a stream termination response.
#[derive(Debug)]
pub enum ResponseTermination {
    /// Blocks by range stream termination.
    BlocksByRange,

    /// Blocks by root stream termination.
    BlocksByRoot,
}

#[derive(Debug)]
pub enum P2PErrorResponse {
    /// The response is a successful.
    Success(P2PResponse),

    /// The response was invalid.
    InvalidRequest(ErrorMessage),

    /// The response indicates a server error.
    ServerError(ErrorMessage),

    /// There was an unknown response.
    Unknown(ErrorMessage),

    /// Received a stream termination indicating which response is being terminated.
    StreamTermination(ResponseTermination),
}

impl P2PErrorResponse {
    /// Used to encode the response in the codec.
    pub fn as_u8(&self) -> Option<u8> {
        match self {
            P2PErrorResponse::Success(_) => Some(0),
            P2PErrorResponse::InvalidRequest(_) => Some(1),
            P2PErrorResponse::ServerError(_) => Some(2),
            P2PErrorResponse::Unknown(_) => Some(255),
            P2PErrorResponse::StreamTermination(_) => None,
        }
    }

    /// Tells the codec whether to decode as an RPCResponse or an error.
    pub fn is_response(response_code: u8) -> bool {
        match response_code {
            0 => true,
            _ => false,
        }
    }

    /// Builds an RPCErrorResponse from a response code and an ErrorMessage
    pub fn from_error(response_code: u8, err: ErrorMessage) -> Self {
        match response_code {
            1 => P2PErrorResponse::InvalidRequest(err),
            2 => P2PErrorResponse::ServerError(err),
            _ => P2PErrorResponse::Unknown(err),
        }
    }

    /// Specifies which response allows for multiple chunks for the stream handler.
    pub fn multiple_responses(&self) -> bool {
        match self {
            P2PErrorResponse::Success(resp) => match resp {
                P2PResponse::Status(_) => false,
                P2PResponse::BlocksByRange(_) => true,
                P2PResponse::BlocksByRoot(_) => true,
            },
            P2PErrorResponse::InvalidRequest(_) => true,
            P2PErrorResponse::ServerError(_) => true,
            P2PErrorResponse::Unknown(_) => true,
            // Stream terminations are part of responses that have chunks
            P2PErrorResponse::StreamTermination(_) => true,
        }
    }

    /// Returns true if this response is an error. Used to terminate the stream after an error is
    /// sent.
    pub fn is_error(&self) -> bool {
        match self {
            P2PErrorResponse::Success(_) => false,
            _ => true,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[derive(Debug)]
pub struct ErrorMessage {
    /// The UTF-8 encoded Error message string.
    pub error_message: Vec<u8>,
}

impl ErrorMessage {
    pub fn as_string(&self) -> String {
        String::from_utf8(self.error_message.clone()).unwrap_or_else(|_| "".into())
    }
}

impl std::fmt::Display for StatusMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Status Message: Genesis hash: {:?}, Finalized Root: {}, Finalized number: {}, Head Root: {}, Network ID: {}", self.genesis_hash, self.finalized_root, self.finalized_number, self.head_root, self.network_id)
    }
}

impl std::fmt::Display for P2PResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            P2PResponse::Status(status) => write!(f, "{}", status),
            P2PResponse::BlocksByRange(_) => write!(f, "<BlocksByRange>"),
            P2PResponse::BlocksByRoot(_) => write!(f, "<BlocksByRoot>"),
        }
    }
}

impl std::fmt::Display for P2PErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            P2PErrorResponse::Success(res) => write!(f, "{}", res),
            P2PErrorResponse::InvalidRequest(err) => write!(f, "Invalid Request: {:?}", err),
            P2PErrorResponse::ServerError(err) => write!(f, "Server Error: {:?}", err),
            P2PErrorResponse::Unknown(err) => write!(f, "Unknown Error: {:?}", err),
            P2PErrorResponse::StreamTermination(_) => write!(f, "Stream Termination"),
        }
    }
}

impl std::fmt::Display for GoodbyeReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoodbyeReason::ClientShutdown => write!(f, "Client Shutdown"),
            GoodbyeReason::IrrelevantNetwork => write!(f, "Irrelevant Network"),
            GoodbyeReason::Fault => write!(f, "Fault"),
            GoodbyeReason::Unknown => write!(f, "Unknown Reason"),
        }
    }
}

impl std::fmt::Display for BlocksByRangeRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Head Block Root: {},  Start Slot: {}, Count: {}, Step: {}",
            self.head_block_root, self.start_slot, self.count, self.step
        )
    }
}
