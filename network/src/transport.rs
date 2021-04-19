use std::{{io::{Error, ErrorKind}}, time::Duration};

use libp2p::core::{
    identity::Keypair,
    muxing::StreamMuxerBox,
    transport::boxed::Boxed,
};
use libp2p::{core, PeerId, secio, Transport};

/// Builds the transport that serves as a common ground for all connections.
pub fn build_transport(private_key: Keypair) -> Boxed<(PeerId, StreamMuxerBox), Error> {
    // TODO: The Wire protocol currently doesn't specify encryption and this will need to be customised
    // in the future.
    let transport = libp2p::tcp::TcpConfig::new().nodelay(true);
    let transport = libp2p::dns::DnsConfig::new(transport);
    #[cfg(feature = "libp2p-websocket")]
        let transport = {
        let trans_clone = transport.clone();
        transport.or_transport(websocket::WsConfig::new(trans_clone))
    };
    transport
        .upgrade(core::upgrade::Version::V1)
        .authenticate(secio::SecioConfig::new(private_key))
        .multiplex(core::upgrade::SelectUpgrade::new(
            libp2p::yamux::Config::default(),
            libp2p::mplex::MplexConfig::new(),
        ))
        .map(|(peer, muxer), _| (peer, core::muxing::StreamMuxerBox::new(muxer)))
        .timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(20))
        .map_err(|err| Error::new(ErrorKind::Other, err))
        .boxed()
}
