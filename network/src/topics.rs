use libp2p::gossipsub::Topic;
use serde::{Serialize, Deserialize};

/// The gossipsub topic names.
// These constants form a topic name of the form /TOPIC_PREFIX/TOPIC/ENCODING_POSTFIX
// For example /map/block/bin
pub const TOPIC_PREFIX: &str = "map";
pub const TOPIC_ENCODING_POSTFIX: &str = "bin";
pub const MAP_BLOCK_TOPIC: &str = "block";
pub const MAP_TRANSACTION_TOPIC: &str = "transaction";
pub const SHARD_TOPIC_PREFIX: &str = "shard";

/// Enum that brings these topics into the rust type system.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GossipTopic {
    MapBlock,
    Transaction,
    Shard,
    Unknown(String),
}

impl From<&str > for GossipTopic {
    fn from(topic: &str) -> GossipTopic {
        let topic_parts: Vec<&str> = topic.split('/').collect();
        if topic_parts.len() == 4
            && topic_parts[1] == TOPIC_PREFIX
            && topic_parts[3] == TOPIC_ENCODING_POSTFIX
        {
            match topic_parts[2] {
                MAP_BLOCK_TOPIC => GossipTopic::MapBlock,
                MAP_TRANSACTION_TOPIC => GossipTopic::Transaction,
                unknown_topic => GossipTopic::Unknown(unknown_topic.into()),
            }
        } else {
            GossipTopic::Unknown(topic.into())
        }
    }
}

impl Into<Topic> for GossipTopic {
    fn into(self) -> Topic {
        Topic::new(self.into())
    }
}

impl Into<String> for GossipTopic {
    fn into(self) -> String {
        match self {
            GossipTopic::MapBlock => topic_builder(MAP_BLOCK_TOPIC),
            GossipTopic::Transaction => topic_builder(MAP_TRANSACTION_TOPIC),
            GossipTopic::Shard => topic_builder(SHARD_TOPIC_PREFIX),
            GossipTopic::Unknown(topic) => topic,
        }
    }
}

fn topic_builder(topic: &'static str) -> String {
    format!("/{}/{}/{}", TOPIC_PREFIX, topic, TOPIC_ENCODING_POSTFIX,)
}
