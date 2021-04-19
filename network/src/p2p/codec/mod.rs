pub(crate) mod base;
pub(crate) mod bin;

use self::base::{BaseInboundCodec, BaseOutboundCodec};
use self::bin::{BINInboundCodec, BINOutboundCodec};
use crate::p2p::protocol::P2PError;
use crate::p2p::{P2PErrorResponse, P2PRequest};
use libp2p::bytes::BytesMut;
use tokio::codec::{Decoder, Encoder};

// Known types of codecs
pub enum InboundCodec {
    BIN(BaseInboundCodec<BINInboundCodec>),
}

pub enum OutboundCodec {
    BIN(BaseOutboundCodec<BINOutboundCodec>),
}

impl Encoder for InboundCodec {
    type Item = P2PErrorResponse;
    type Error = P2PError;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match self {
            InboundCodec::BIN(codec) => codec.encode(item, dst),
        }
    }
}

impl Decoder for InboundCodec {
    type Item = P2PRequest;
    type Error = P2PError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self {
            InboundCodec::BIN(codec) => codec.decode(src),
        }
    }
}

impl Encoder for OutboundCodec {
    type Item = P2PRequest;
    type Error = P2PError;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match self {
            OutboundCodec::BIN(codec) => codec.encode(item, dst),
        }
    }
}

impl Decoder for OutboundCodec {
    type Item = P2PErrorResponse;
    type Error = P2PError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self {
            OutboundCodec::BIN(codec) => codec.decode(src),
        }
    }
}
