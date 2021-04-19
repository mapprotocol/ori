use jsonrpc_core::{IoHandler};
use tokio::sync::mpsc;

use chain::blockchain::BlockChain;
use pool::tx_pool::TxPoolManager;
use std::sync::{Arc, RwLock};

use network::manager::NetworkMessage;
use crate::api::{
    ChainRpc, ChainRpcImpl,
    AccountManager, AccountManagerImpl};

pub struct RpcBuilder {
    io_handler: IoHandler,
}

impl RpcBuilder {
    pub fn new() -> Self {
        Self {
            io_handler: IoHandler::new(),
        }
    }
    pub fn config_chain(mut self, block_chain: Arc<RwLock<BlockChain>>) -> Self {
        let chain = ChainRpcImpl { block_chain }.to_delegate();
        self.io_handler.extend_with(chain);
        self
    }

    pub fn config_account(
        mut self,
        tx_pool: Arc<RwLock<TxPoolManager>>,
        key : String,
        network_send: mpsc::UnboundedSender<NetworkMessage>
    ) -> Self {
        let pool = AccountManagerImpl::new(tx_pool, key, network_send).to_delegate();
        self.io_handler.extend_with(pool);
        self
    }

    pub fn build(self) -> IoHandler {
        self.io_handler
    }
}
