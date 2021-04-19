#![allow(clippy::type_complexity)]

use std::io;
use std::time::Duration;

use futures::{
    future::{self, FutureResult},
    sink, Sink, stream, Stream,
};
use libp2p::core::{InboundUpgrade, OutboundUpgrade, ProtocolName, upgrade, UpgradeInfo};
use tokio::codec::Framed;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::prelude::*;
use tokio::timer::timeout;
use tokio::util::FutureExt;
use tokio_io_timeout::TimeoutStream;

use crate::p2p::{
    codec::{
        base::{BaseInboundCodec, BaseOutboundCodec},
        InboundCodec,
        OutboundCodec, bin::{BINInboundCodec, BINOutboundCodec},
    },
    methods::ResponseTermination,
};

use super::methods::*;
use serde::{Serialize, Deserialize};

/// The maximum bytes that can be sent across the P2P.
const MAX_P2P_SIZE: usize = 4_194_304;
// 4M
/// The protocol prefix the P2P protocol id.
const PROTOCOL_PREFIX: &str = "/map/req";
/// Time allowed for the first byte of a request to arrive before we time out (Time To First Byte).
const TTFB_TIMEOUT: u64 = 5;
/// The number of seconds to wait for the first bytes of a request once a protocol has been
/// established before the stream is terminated.
const REQUEST_TIMEOUT: u64 = 15;

/// Protocol names to be used.
/// The Status protocol name.
pub const RPC_STATUS: &str = "status";
/// The Goodbye protocol name.
pub const RPC_GOODBYE: &str = "goodbye";
/// The `BlocksByRange` protocol name.
pub const RPC_BLOCKS_BY_RANGE: &str = "map_blocks_by_range";
/// The `BlocksByRoot` protocol name.
pub const RPC_BLOCKS_BY_ROOT: &str = "map_blocks_by_root";

#[derive(Debug, Clone)]
pub struct P2PProtocol;

impl UpgradeInfo for P2PProtocol {
    type Info = ProtocolId;
    type InfoIter = Vec<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        vec![
            ProtocolId::new(RPC_STATUS, "1", "bin"),
            ProtocolId::new(RPC_GOODBYE, "1", "bin"),
            ProtocolId::new(RPC_BLOCKS_BY_RANGE, "1", "bin"),
            ProtocolId::new(RPC_BLOCKS_BY_ROOT, "1", "bin"),
        ]
    }
}

/// Tracks the types in a protocol id.
#[derive(Clone)]
pub struct ProtocolId {
    /// The p2p message type/name.
    pub message_name: String,

    /// The version of the P2P.
    pub version: String,

    /// The encoding of the P2P.
    pub encoding: String,

    /// The protocol id that is formed from the above fields.
    protocol_id: String,
}

/// An P2P protocol ID.
impl ProtocolId {
    pub fn new(message_name: &str, version: &str, encoding: &str) -> Self {
        let protocol_id = format!(
            "{}/{}/{}/{}",
            PROTOCOL_PREFIX, message_name, version, encoding
        );

        ProtocolId {
            message_name: message_name.into(),
            version: version.into(),
            encoding: encoding.into(),
            protocol_id,
        }
    }
}

impl ProtocolName for ProtocolId {
    fn protocol_name(&self) -> &[u8] {
        self.protocol_id.as_bytes()
    }
}

/* Inbound upgrade */

// The inbound protocol reads the request, decodes it and returns the stream to the protocol
// handler to respond to once ready.

pub type InboundOutput<TSocket> = (P2PRequest, InboundFramed<TSocket>);
pub type InboundFramed<TSocket> = Framed<TimeoutStream<upgrade::Negotiated<TSocket>>, InboundCodec>;
type FnAndThen<TSocket> = fn(
    (Option<P2PRequest>, InboundFramed<TSocket>),
) -> FutureResult<InboundOutput<TSocket>, P2PError>;
type FnMapErr<TSocket> = fn(timeout::Error<(P2PError, InboundFramed<TSocket>)>) -> P2PError;

impl<TSocket> InboundUpgrade<TSocket> for P2PProtocol
    where
        TSocket: AsyncRead + AsyncWrite,
{
    type Output = InboundOutput<TSocket>;
    type Error = P2PError;

    type Future = future::AndThen<
        future::MapErr<
            timeout::Timeout<stream::StreamFuture<InboundFramed<TSocket>>>,
            FnMapErr<TSocket>,
        >,
        FutureResult<InboundOutput<TSocket>, P2PError>,
        FnAndThen<TSocket>,
    >;

    fn upgrade_inbound(
        self,
        socket: upgrade::Negotiated<TSocket>,
        protocol: ProtocolId,
    ) -> Self::Future {
        match protocol.encoding.as_str() {
            "bin" | _ => {
                let bin_codec = BaseInboundCodec::new(BINInboundCodec::new(protocol, MAX_P2P_SIZE));
                let codec = InboundCodec::BIN(bin_codec);
                let mut timed_socket = TimeoutStream::new(socket);
                timed_socket.set_read_timeout(Some(Duration::from_secs(TTFB_TIMEOUT)));
                Framed::new(timed_socket, codec)
                    .into_future()
                    .timeout(Duration::from_secs(REQUEST_TIMEOUT))
                    .map_err(P2PError::from as FnMapErr<TSocket>)
                    .and_then({
                        |(req, stream)| match req {
                            Some(req) => futures::future::ok((req, stream)),
                            None => futures::future::err(P2PError::Custom(
                                "Stream terminated early".into(),
                            )),
                        }
                    } as FnAndThen<TSocket>)
            }
        }
    }
}

/* Outbound request */

// Combines all the P2P requests into a single enum to implement `UpgradeInfo` and
// `OutboundUpgrade`

#[derive(Serialize, Deserialize,Debug, Clone, PartialEq)]
pub enum P2PRequest {
    Status(StatusMessage),
    Goodbye(GoodbyeReason),
    BlocksByRange(BlocksByRangeRequest),
    BlocksByRoot(BlocksByRootRequest),
}

impl UpgradeInfo for P2PRequest {
    type Info = ProtocolId;
    type InfoIter = Vec<Self::Info>;

    // add further protocols as we support more encodings/versions
    fn protocol_info(&self) -> Self::InfoIter {
        self.supported_protocols()
    }
}

/// Implements the encoding per supported protocol for RPCRequest.
impl P2PRequest {
    pub fn supported_protocols(&self) -> Vec<ProtocolId> {
        match self {
            // add more protocols when versions/encodings are supported
            P2PRequest::Status(_) => vec![ProtocolId::new(RPC_STATUS, "1", "bin")],
            P2PRequest::Goodbye(_) => vec![ProtocolId::new(RPC_GOODBYE, "1", "bin")],
            P2PRequest::BlocksByRange(_) => vec![ProtocolId::new(RPC_BLOCKS_BY_RANGE, "1", "bin")],
            P2PRequest::BlocksByRoot(_) => vec![ProtocolId::new(RPC_BLOCKS_BY_ROOT, "1", "bin")],
        }
    }

    /* These functions are used in the handler for stream management */

    /// This specifies whether a stream should remain open and await a response, given a request.
    /// A GOODBYE request has no response.
    pub fn expect_response(&self) -> bool {
        match self {
            P2PRequest::Status(_) => true,
            P2PRequest::Goodbye(_) => false,
            P2PRequest::BlocksByRange(_) => true,
            P2PRequest::BlocksByRoot(_) => true,
        }
    }

    /// Returns which methods expect multiple responses from the stream. If this is false and
    /// the stream terminates, an error is given.
    pub fn multiple_responses(&self) -> bool {
        match self {
            P2PRequest::Status(_) => false,
            P2PRequest::Goodbye(_) => false,
            P2PRequest::BlocksByRange(_) => true,
            P2PRequest::BlocksByRoot(_) => true,
        }
    }

    /// Returns the `ResponseTermination` type associated with the request if a stream gets
    /// terminated.
    pub fn stream_termination(&self) -> ResponseTermination {
        match self {
            // this only gets called after `multiple_responses()` returns true. Therefore, only
            // variants that have `multiple_responses()` can have values.
            P2PRequest::BlocksByRange(_) => ResponseTermination::BlocksByRange,
            P2PRequest::BlocksByRoot(_) => ResponseTermination::BlocksByRoot,
            P2PRequest::Status(_) => unreachable!(),
            P2PRequest::Goodbye(_) => unreachable!(),
        }
    }
}

/* P2P Response type - used for outbound upgrades */

/* Outbound upgrades */

pub type OutboundFramed<TSocket> = Framed<upgrade::Negotiated<TSocket>, OutboundCodec>;

impl<TSocket> OutboundUpgrade<TSocket> for P2PRequest
    where
        TSocket: AsyncRead + AsyncWrite,
{
    type Output = OutboundFramed<TSocket>;
    type Error = P2PError;
    type Future = sink::Send<OutboundFramed<TSocket>>;
    fn upgrade_outbound(
        self,
        socket: upgrade::Negotiated<TSocket>,
        protocol: Self::Info,
    ) -> Self::Future {
        match protocol.encoding.as_str() {
            "bin" | _ => {
                let bin_codec =
                    BaseOutboundCodec::new(BINOutboundCodec::new(protocol, MAX_P2P_SIZE));
                let codec = OutboundCodec::BIN(bin_codec);
                Framed::new(socket, codec).send(self)
            }
        }
    }
}

/// Error in RPC Encoding/Decoding.
#[derive(Debug)]
pub enum P2PError {
    /// Error when reading the packet from the socket.
    ReadError(upgrade::ReadOneError),
    /// Invalid Protocol ID.
    InvalidProtocol(&'static str),
    /// IO Error.
    IoError(io::Error),
    /// Waiting for a request/response timed out, or timer error'd.
    StreamTimeout,
    /// The peer returned a valid RPCErrorResponse but the response was an error.
    P2PErrorResponse,
    /// Custom message.
    Custom(String),
}

impl From<upgrade::ReadOneError> for P2PError {
    #[inline]
    fn from(err: upgrade::ReadOneError) -> Self {
        P2PError::ReadError(err)
    }
}

impl<T> From<tokio::timer::timeout::Error<T>> for P2PError {
    fn from(err: tokio::timer::timeout::Error<T>) -> Self {
        if err.is_elapsed() {
            P2PError::StreamTimeout
        } else {
            P2PError::Custom("Stream timer failed".into())
        }
    }
}

impl From<()> for P2PError {
    fn from(_err: ()) -> Self {
        P2PError::Custom("".into())
    }
}

impl From<io::Error> for P2PError {
    fn from(err: io::Error) -> Self {
        P2PError::IoError(err)
    }
}

// Error trait is required for `ProtocolsHandler`
impl std::fmt::Display for P2PError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            P2PError::ReadError(ref err) => write!(f, "Error while reading from socket: {}", err),
            P2PError::InvalidProtocol(ref err) => write!(f, "Invalid Protocol: {}", err),
            P2PError::IoError(ref err) => write!(f, "IO Error: {}", err),
            P2PError::P2PErrorResponse => write!(f, "P2P Response Error"),
            P2PError::StreamTimeout => write!(f, "Stream Timeout"),
            P2PError::Custom(ref err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for P2PError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            P2PError::ReadError(ref err) => Some(err),
            P2PError::InvalidProtocol(_) => None,
            P2PError::IoError(ref err) => Some(err),
            P2PError::StreamTimeout => None,
            P2PError::P2PErrorResponse => None,
            P2PError::Custom(_) => None,
        }
    }
}

impl std::fmt::Display for P2PRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            P2PRequest::Status(status) => write!(f, "Status Message: {}", status),
            P2PRequest::Goodbye(reason) => write!(f, "Goodbye: {}", reason),
            P2PRequest::BlocksByRange(req) => write!(f, "Blocks by range: {}", req),
            P2PRequest::BlocksByRoot(req) => write!(f, "Blocks by root: {:?}", req),
        }
    }
}
