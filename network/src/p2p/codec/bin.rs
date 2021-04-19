use crate::p2p::{
    codec::base::OutboundCodec,
    protocol::{
        ProtocolId, P2PError, RPC_BLOCKS_BY_RANGE, RPC_BLOCKS_BY_ROOT, RPC_GOODBYE, RPC_STATUS,
    },
};
use crate::p2p::{ErrorMessage, P2PErrorResponse, P2PRequest, P2PResponse};
use libp2p::bytes::{BufMut, Bytes, BytesMut};
use tokio::codec::{Decoder, Encoder};
use unsigned_varint::codec::UviBytes;

/* Inbound Codec */

pub struct BINInboundCodec {
    inner: UviBytes,
    protocol: ProtocolId,
}

impl BINInboundCodec {
    pub fn new(protocol: ProtocolId, max_packet_size: usize) -> Self {
        let mut uvi_codec = UviBytes::default();
        uvi_codec.set_max_len(max_packet_size);

        // this encoding only applies to bin.
        debug_assert!(protocol.encoding.as_str() == "bin");

        BINInboundCodec {
            inner: uvi_codec,
            protocol,
        }
    }
}

// Encoder for inbound streams: Encodes P2P Responses sent to peers.
impl Encoder for BINInboundCodec {
    type Item = P2PErrorResponse;
    type Error = P2PError;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes = match item {
            P2PErrorResponse::Success(resp) => {
                match resp {
                    P2PResponse::Status(res) => bincode::serialize(&res).unwrap(),
                    P2PResponse::BlocksByRange(res) => res, // already raw bytes
                    P2PResponse::BlocksByRoot(res) => res,  // already raw bytes
                }
            }
            P2PErrorResponse::InvalidRequest(err) => bincode::serialize(&err).unwrap(),
            P2PErrorResponse::ServerError(err) => bincode::serialize(&err).unwrap(),
            P2PErrorResponse::Unknown(err) => bincode::serialize(&err).unwrap(),
            P2PErrorResponse::StreamTermination(_) => {
                unreachable!("Code error - attempting to encode a stream termination")
            }
        };
        if !bytes.is_empty() {
            // length-prefix and return
            return self
                .inner
                .encode(Bytes::from(bytes), dst)
                .map_err(P2PError::from);
        } else {
            // payload is empty, add a 0-byte length prefix
            dst.reserve(1);
            dst.put_u8(0);
        }
        Ok(())
    }
}

// Decoder for inbound streams: Decodes P2P requests from peers
impl Decoder for BINInboundCodec {
    type Item = P2PRequest;
    type Error = P2PError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.inner.decode(src).map_err(P2PError::from) {

            Ok(Some(packet)) => match self.protocol.message_name.as_str() {
                RPC_STATUS => match self.protocol.version.as_str() {
                    "1" => Ok(Some(P2PRequest::Status(bincode::deserialize(&packet[..]).unwrap()))),
                    _ => unreachable!("Cannot negotiate an unknown version"),
                },
                RPC_GOODBYE => match self.protocol.version.as_str() {
                    "1" => Ok(Some(P2PRequest::Goodbye(bincode::deserialize(&packet[..]).unwrap()))),
                    _ => unreachable!("Cannot negotiate an unknown version"),
                },
                RPC_BLOCKS_BY_RANGE => match self.protocol.version.as_str() {
                    "1" => Ok(Some(P2PRequest::BlocksByRange(bincode::deserialize(&packet[..]).unwrap()))),
                    _ => unreachable!("Cannot negotiate an unknown version"),
                },
                RPC_BLOCKS_BY_ROOT => match self.protocol.version.as_str() {
                    "1" => Ok(Some(P2PRequest::BlocksByRoot(bincode::deserialize(&packet[..]).unwrap()))),
                    _ => unreachable!("Cannot negotiate an unknown version"),
                },
                _ => unreachable!("Cannot negotiate an unknown protocol"),
            },
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/* Outbound Codec: Codec for initiating P2P requests */

pub struct BINOutboundCodec {
    inner: UviBytes,
    protocol: ProtocolId,
}

impl BINOutboundCodec {
    pub fn new(protocol: ProtocolId, max_packet_size: usize) -> Self {
        let mut uvi_codec = UviBytes::default();
        uvi_codec.set_max_len(max_packet_size);

        // this encoding only applies to bin.
        debug_assert!(protocol.encoding.as_str() == "bin");

        BINOutboundCodec {
            inner: uvi_codec,
            protocol,
        }
    }
}

// Encoder for outbound streams: Encodes P2P Requests to peers
impl Encoder for BINOutboundCodec {
    type Item = P2PRequest;
    type Error = P2PError;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes = match item {
            P2PRequest::Status(req) => bincode::serialize(&req).unwrap(),
            P2PRequest::Goodbye(req) => bincode::serialize(&req).unwrap(),
            P2PRequest::BlocksByRange(req) => bincode::serialize(&req).unwrap(),
            P2PRequest::BlocksByRoot(req) => bincode::serialize(&req.block_roots).unwrap(),
        };
        // length-prefix
        self.inner
            .encode(libp2p::bytes::Bytes::from(bytes), dst)
            .map_err(P2PError::from)
    }
}

// Decoder for outbound streams: Decodes P2P responses from peers.
//
// The majority of the decoding has now been pushed upstream due to the changing specification.
// We prefer to decode blocks and attestations with extra knowledge about the chain to perform
// faster verification checks before decoding entire blocks/attestations.
impl Decoder for BINOutboundCodec {
    type Item = P2PResponse;
    type Error = P2PError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() == 1 && src[0] == 0_u8 {
            // the object is empty. We return the empty object if this is the case
            // clear the buffer and return an empty object
            src.clear();
            match self.protocol.message_name.as_str() {
                RPC_STATUS => match self.protocol.version.as_str() {
                    "1" => Err(P2PError::Custom(
                        "Status stream terminated unexpectedly".into(),
                    )), // cannot have an empty HELLO message. The stream has terminated unexpectedly
                    _ => unreachable!("Cannot negotiate an unknown version"),
                },
                RPC_GOODBYE => Err(P2PError::InvalidProtocol("GOODBYE doesn't have a response")),
                RPC_BLOCKS_BY_RANGE => match self.protocol.version.as_str() {
                    "1" => Ok(Some(P2PResponse::BlocksByRange(Vec::new()))),
                    _ => unreachable!("Cannot negotiate an unknown version"),
                },
                RPC_BLOCKS_BY_ROOT => match self.protocol.version.as_str() {
                    "1" => Ok(Some(P2PResponse::BlocksByRoot(Vec::new()))),
                    _ => unreachable!("Cannot negotiate an unknown version"),
                },
                _ => unreachable!("Cannot negotiate an unknown protocol"),
            }
        } else {
            match self.inner.decode(src).map_err(P2PError::from) {
                Ok(Some(mut packet)) => {
                    // take the bytes from the buffer
                    let raw_bytes = packet.take();

                    match self.protocol.message_name.as_str() {
                        RPC_STATUS => match self.protocol.version.as_str() {
                            "1" => Ok(Some(P2PResponse::Status(bincode::deserialize(&raw_bytes[..]).unwrap()))),
                            _ => unreachable!("Cannot negotiate an unknown version"),
                        },
                        RPC_GOODBYE => {
                            Err(P2PError::InvalidProtocol("GOODBYE doesn't have a response"))
                        }
                        RPC_BLOCKS_BY_RANGE => match self.protocol.version.as_str() {
                            "1" => Ok(Some(P2PResponse::BlocksByRange(raw_bytes.to_vec()))),
                            _ => unreachable!("Cannot negotiate an unknown version"),
                        },
                        RPC_BLOCKS_BY_ROOT => match self.protocol.version.as_str() {
                            "1" => Ok(Some(P2PResponse::BlocksByRoot(raw_bytes.to_vec()))),
                            _ => unreachable!("Cannot negotiate an unknown version"),
                        },
                        _ => unreachable!("Cannot negotiate an unknown protocol"),
                    }
                }
                Ok(None) => Ok(None), // waiting for more bytes
                Err(e) => Err(e),
            }
        }
    }
}

impl OutboundCodec for BINOutboundCodec {
    type ErrorType = ErrorMessage;

    fn decode_error(&mut self, src: &mut BytesMut) -> Result<Option<Self::ErrorType>, P2PError> {
        match self.inner.decode(src).map_err(P2PError::from) {
            Ok(Some(packet)) => Ok(Some(bincode::deserialize(&packet[..]).unwrap())),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
