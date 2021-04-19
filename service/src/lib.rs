// Copyright 2021 MAP Protocol Authors.
// This file is part of MAP Protocol.

// MAP Protocol is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// MAP Protocol is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with MAP Protocol.  If not, see <http://www.gnu.org/licenses/>.

extern crate chain;
extern crate consensus;
extern crate core;
extern crate errors;
extern crate executor;
// #[macro_use]
// extern crate log;
extern crate network;
extern crate rpc;

use std::{sync::mpsc, thread};
use std::path::PathBuf;
use std::time::Duration;
use std::sync::{Arc, RwLock, RwLockWriteGuard};

use futures::{Future};
use tokio::runtime::{Builder as RuntimeBuilder, TaskExecutor};

use chain::blockchain::BlockChain;
use ed25519::generator::create_key;
// use ed25519::pubkey::Pubkey;
use ed25519::privkey::PrivKey;
use generator::apos::EpochPoS;
use generator::epoch::EpochProposal;
use network::{manager as network_executor, Multiaddr, NetworkConfig};
use pool::tx_pool::TxPoolManager;
use rpc::http_server;

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub log: String,
    pub data_dir: PathBuf,
    pub rpc_addr: String,
    pub rpc_port: u16,
    pub key: String,
    pub poa_privkey: String,
    pub dev_mode: bool,
    /// List of p2p nodes to initially connect to.
    pub dial_addrs: Vec<Multiaddr>,
    pub p2p_port: u16,
    pub seal_block: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        NodeConfig {
            log: "info".into(),
            data_dir: PathBuf::from("."),
            rpc_addr: "127.0.0.1".into(),
            rpc_port: 9545,
            key: "".into(),
            poa_privkey: "".into(),
            dev_mode: false,
            dial_addrs: vec![],
            p2p_port: 40313,
            seal_block:false,
        }
    }
}

//#[derive(Debug, Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Service {
    pub block_chain: Arc<RwLock<BlockChain>>,
    pub tx_pool: Arc<RwLock<TxPoolManager>>,
    pub cfg: NodeConfig,
}

impl Service {
    pub fn new_service(cfg: NodeConfig) -> Self {
        let chain = Arc::new(RwLock::new(BlockChain::new(cfg.data_dir.clone(),cfg.poa_privkey.clone())));

        Service {
            block_chain: chain.clone(),
            tx_pool: Arc::new(RwLock::new(TxPoolManager::new(chain.clone()))),
            cfg:   cfg.clone(),
        }
    }

    // fn get_poa(&self) -> POA {
    //     let key = self.cfg.poa_privkey.clone();
    //     POA::new_from_string(key)
    // }

    pub fn start(&self, cfg: NodeConfig) -> mpsc::Sender<i32> {
		let runtime = RuntimeBuilder::new()
			.core_threads(1)
			.build()
			.map_err(|e| format!("Failed to start runtime: {:?}", e)).expect("Failed to start runtime");

        self.get_write_blockchain().load();
        let network_block_chain = self.block_chain.clone();
        let thread_executor: TaskExecutor = runtime.executor();

        let mut config = NetworkConfig::new();
        config.update_network_cfg(cfg.data_dir, cfg.dial_addrs, cfg.p2p_port).unwrap();
        let network_ref = network_executor::NetworkExecutor::new(
            config.clone(), network_block_chain, self.tx_pool.clone(), &thread_executor, cfg.log).expect("Network start error");

        let rpc_server = http_server::start_http(http_server::RpcConfig {
            rpc_addr: cfg.rpc_addr,
            rpc_port: cfg.rpc_port,
            key: cfg.key.clone(),
        }, self.block_chain.clone(), self.tx_pool.clone(), network_ref.network_send.clone());

        let (tx, rx): (mpsc::Sender<i32>,mpsc::Receiver<i32>) = mpsc::channel();

        let shared_block_chain = self.block_chain.clone();

        // Create random node key
        let node_key = match PrivKey::from_hex(&cfg.key.clone()) {
            Ok(k) => k,
            _ => {
                let (sk, _) = create_key();
                sk
            },
        };

        let stake = Arc::new(RwLock::new(EpochPoS::new(shared_block_chain.clone(), cfg.dev_mode)));
        let slot_clock = EpochProposal::new(
            node_key,
            shared_block_chain.clone(),
            stake.clone(),
            self.tx_pool.clone(),
            network_ref.network_send.clone(),
            thread_executor.clone(),
        );
        let slot_signal = slot_clock.start();

		// Cancel all tasks
		thread::spawn(move || {
			loop {
				if rx.try_recv().is_ok() {
					// Cancel slot tick service
                    slot_signal.send(()).unwrap();

					if !network_ref.exit_signal.is_closed() {
						network_ref.exit_signal.send(1).expect("network exit error");
					}

					runtime
						.shutdown_on_idle()
						.wait()
						.map_err(|e| format!("Tokio runtime shutdown returned an error: {:?}", e)).unwrap();
					rpc_server.close();
					break;
				}
                thread::sleep(Duration::from_millis(200));
			}
		});

        tx
    }

    // pub fn new_empty_block() -> Block {
    //     Block::default()
    // }

    // pub fn generate_block(&mut self) -> Result<Block,Error> {
    //     let cur_block = self.get_write_blockchain().current_block();
    //     let tx_pool = self.tx_pool.clone();
    //     let txs =
    //         tx_pool.read().expect("acquiring tx_pool read lock").get_pending();

    //     let txs_root = block::get_hash_from_txs(&txs);
    //     let header: Header = Header{
    //         height: cur_block.height() + 1,
    //         parent_hash: cur_block.get_hash().clone(),
    //         tx_root:    txs_root,
    //         state_root: Hash([0;32]),
    //         sign_root:  Hash([0;32]),
    //         time: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
    //     };
    //     info!("seal block, height={}, parent={}, tx={}", header.height, header.parent_hash, txs.len());
    //     let b = Block::new(header,txs,Vec::new(),Vec::new());
    //     let finalize = self.get_poa();
    //     let chain = self.block_chain.read().unwrap();
    //     let statedb = chain.state_at(cur_block.state_root());

    //     let h = Executor::exc_txs_in_block(&b, &mut Balance::new(Interpreter::new(statedb)), &POA::get_default_miner())?;
    //     // tx_pool.write().expect("acquiring tx_pool write lock").notify_block(&b);
    //     tx_pool.write().unwrap().reset_pool(&b);
    //     finalize.finalize_block(b,h)
    // }

    // pub fn get_current_block(&mut self) -> Block {
    //     self.get_write_blockchain().current_block()
    // }

    // pub fn get_current_height(&mut self) -> u64 {
    //     self.get_write_blockchain().current_block().height()
    // }

    // pub fn get_block_by_height(&self,height: u64) -> Option<Block> {
    //     self.get_readblockchain().get_block_by_number(height)
    // }

    // fn get_readblockchain(&self) -> RwLockReadGuard<BlockChain> {
    //     self.block_chain.read().expect("acquiring block_chain read lock")
    // }

    fn get_write_blockchain(&self) -> RwLockWriteGuard<BlockChain> {
        self.block_chain.write().expect("acquiring block_chain write lock")
    }
}


#[cfg(test)]
mod tests {
	use std::fmt;
	use std::time::Duration;

	use super::*;

	#[test]
    fn test_service() {
        println!("begin service,for 60 seconds");
        let mut config = NodeConfig::default();
        let service = Service::new_service(config.clone());
        let (tx,th_handle) = service.start(config.clone());
        thread::sleep(Duration::from_millis(60*1000));
        thread::spawn(move || {
            tx.send(1).unwrap();
        });
        th_handle.join();
        println!("end service");
    }
}
