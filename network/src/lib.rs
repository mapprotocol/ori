pub use libp2p::{
    gossipsub::{GossipsubConfig, GossipsubConfigBuilder},
    PeerId,
};
pub use libp2p::gossipsub::{Topic, TopicHash};
pub use libp2p::multiaddr;
pub use libp2p::multiaddr::Multiaddr;

pub use config::Config as NetworkConfig;
pub use topics::GossipTopic;
pub use handler_processor::MessageProcessor;

pub mod service;
pub mod transport;
pub mod behaviour;
pub mod config;
pub mod manager;
pub mod error;
pub mod p2p;
pub mod topics;
pub mod handler;
pub mod handler_processor;
pub mod sync;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
