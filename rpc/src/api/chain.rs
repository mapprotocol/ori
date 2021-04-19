use std::sync::{Arc, RwLock, RwLockReadGuard};

use jsonrpc_core::Result;
use jsonrpc_derive::rpc;

use chain::blockchain::BlockChain;
use map_core::block::{Block, Header};
use map_core::types::Hash;

#[rpc(server)]
pub trait ChainRpc {
    #[rpc(name = "map_getHeaderByNumber")]
    fn get_header_by_number(&self, num: u64) -> Result<Option<Header>>;

    #[rpc(name = "map_getBlock")]
    fn get_block(&self, hash: Hash) -> Result<Option<Block>>;

    #[rpc(name = "map_getBlockByNumber")]
    fn get_block_by_number(&self, num: u64) -> Result<Option<Block>>;

    #[rpc(name = "map_getTransaction")]
    fn get_transaction(&self, hash: Hash) -> Result<Option<String>>;
}

pub(crate) struct ChainRpcImpl {
    pub block_chain: Arc<RwLock<BlockChain>>,
}

impl ChainRpc for ChainRpcImpl {
    fn get_block(&self, hash: Hash) -> Result<Option<Block>> {
        Ok(self.get_blockchain().get_block(hash))
    }

    fn get_block_by_number(&self, num: u64) -> Result<Option<Block>> {
        Ok(self.get_blockchain().get_block_by_number(num))
    }

    fn get_header_by_number(&self, num: u64) -> Result<Option<Header>> {
        Ok(self.get_blockchain().get_header_by_number(num))
    }

    fn get_transaction(&self, _hash: Hash) -> Result<Option<String>> {
        Ok(Some(format!("{}", "Success")))
    }
}

impl ChainRpcImpl {
    fn get_blockchain(&self) -> RwLockReadGuard<BlockChain> {
        self.block_chain.read().expect("acquiring block_chain read lock")
    }
}