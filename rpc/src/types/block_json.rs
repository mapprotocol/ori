use map_core::block::{Block};
use serde::ser::Error;
use serde::{Serialize, Serializer};
use std::ops::Deref;
use std::collections::BTreeMap;
use maplit::btreemap;

/// Block representation with additional info.
#[derive(Debug, Clone)]
pub struct BlockJson {
    /// Standard block.
    pub inner: Block,
    /// Fields with additional description.
    pub extra_info: BTreeMap<String, String>,
}

impl BlockJson {
    fn default_extra_info(b: &Block) -> BTreeMap<String, String> {
        btreemap![
			"hash".into() => format!("{}", b.hash()),
		]
    }
}

impl From<Block> for BlockJson {
    fn from(b: Block) -> Self {
        BlockJson{
            extra_info:BlockJson::default_extra_info(&b),
            inner:b,
        }
    }
}

impl Deref for BlockJson {
    type Target = Block;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Serialize for BlockJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        use serde_json::{to_value, Value};

        let serialized = (to_value(&self.inner), to_value(&self.extra_info));
        if let (Ok(Value::Object(mut value)), Ok(Value::Object(extras))) = serialized {
            // join two objects
            value.extend(extras);
            // and serialize
            value.serialize(serializer)
        } else {
            Err(S::Error::custom("Unserializable structures: expected objects"))
        }
    }
}