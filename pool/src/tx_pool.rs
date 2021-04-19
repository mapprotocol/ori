use std::collections::{HashMap, BinaryHeap};
use std::sync::{Arc, RwLock};
use std::cmp;

use map_core::balance::Balance;
use map_core::block::Block;
use map_core::transaction::Transaction;
use map_core::types::{Address, Hash};
use map_core::runtime::Interpreter;
use chain::blockchain::BlockChain;

/// Max of block transactin limit
const MAX_BLOCK_TX: u32 = 500;
/// Max transaction pool limit
const MAX_QUEUE_TX: u32 = 2048;

#[derive(Clone)]
pub struct TxPoolManager {
    pending: HashMap<Hash, Transaction>,
    pool: HashMap<Hash, Transaction>,
    blockchain: Arc<RwLock<BlockChain>>,
    ordered_queue: BinaryHeap<PriorityRef>,
    block_limit: usize,
    queue_limit: usize,
}

#[derive(Clone)]
pub struct PriorityRef {
    tx_hash: Hash,
    price: u64,
    // tx: Arc<Transaction>,
}

impl Ord for PriorityRef {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        // self.tx.get_gas_price().cmp(&other.tx.get_gas_price())
        self.price.cmp(&other.price).reverse()
    }
}

impl PartialOrd for PriorityRef {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PriorityRef {
    fn eq(&self, other: &Self) -> bool {
        // self.tx.get_gas_price() == other.tx.get_gas_price()
        self.price == other.price
    }
}

impl Eq for PriorityRef {}


impl TxPoolManager {
    pub fn add_tx(&mut self, tx: Transaction) -> bool {
        match self.validate_tx(&tx) {
            Ok(_) => self.pending.insert(tx.hash(), tx.clone()),
            Err(e) => {
                error!("Submit tx {}", e.as_str());
                return false
            }
        };
        // let mut send = self.network_send.as_mut().unwrap();
        // manager::publish_transaction(&mut send, tx)
        true
    }

    pub fn insert_tx(&mut self, tx: Transaction) {
        match self.validate_tx(&tx) {
            Err(e) => {
                return info!("Submit tx {}", e.as_str());
            },
            _ => {},
        };

        if self.all_transactions().len() > self.block_limit + self.queue_limit {
            // Replace or drop new transaction
            info!("Reject transaction {}", tx.hash());
            if let Some(removed) = self.pop_back() {
                if self.pending.remove(&removed).is_some() {
                    info!("Dequeu pending transaction {}", removed);
                } else {
                    self.pool.remove(&removed);
                    info!("Dequeu queued transaction {}", removed);
                }
            }
        }

        let tx_hash = tx.hash();
        let tx_price = tx.get_gas_price();
        if self.pending.len() > self.block_limit {
            self.pool.insert(tx.hash(), tx);
        } else {
            self.pending.insert(tx.hash(), tx);
        }

        self.ordered_queue.push(PriorityRef{
            tx_hash: tx_hash,
            price: tx_price,
        });
    }

    fn pop_back(&mut self) -> Option<Hash> {
        if self.ordered_queue.len() == 0 {
            return None;
        }

        let last = self.ordered_queue.pop().unwrap();

        if self.pool.remove(&last.tx_hash).is_none() {
            self.pending.remove(&last.tx_hash).unwrap();
        }

        Some(last.tx_hash)
    }


    pub fn get_pending(&self) -> Vec<Transaction> {
        self.pending.values().cloned().collect()
    }

    pub fn remove_tx(&mut self, tx_hash: Hash) {
        if self.pending.remove(&tx_hash).is_some() {
        } else {
            info!("Clean stale transaction {}", tx_hash);
            self.pool.remove(&tx_hash);
        }
    }

    pub fn all_transactions(&self) -> Vec<Transaction> {
        let mut all: Vec<Transaction> = self.pending.values().cloned().collect();
        let queued: Vec<Transaction> = self.pool.values().cloned().collect();
        all.extend(queued);
        all
    }

    pub fn reset_pool(&mut self, b: &Block) {
        let state = self.blockchain.read().unwrap().state_at(b.state_root());
        let runtime = Balance::new(Interpreter::new(state));
        self.pending.retain(|_, tx| {
            let account = runtime.get_account(tx.sender);
            tx.get_nonce() > account.get_nonce()
        });
    }

    pub fn new(chain: Arc<RwLock<BlockChain>>) -> Self {
        TxPoolManager {
            pending: HashMap::new(),
            pool: HashMap::new(),
            blockchain: chain,
            ordered_queue: BinaryHeap::new(),
            block_limit: MAX_BLOCK_TX as usize,
            queue_limit: MAX_QUEUE_TX as usize,
        }
    }

    // pub fn start(&mut self, network: mpsc::UnboundedSender<NetworkMessage>) {
    //     self.network_send = Some(network);
    // }

    fn validate_tx(&self, tx: &Transaction) -> Result<(), String> {
        let chain = self.blockchain.read().unwrap();
        let state = chain.state_at(chain.current_block().state_root());
        let runtime = Balance::new(Interpreter::new(state));
        let account = runtime.get_account(tx.sender);

        if account.get_balance() < tx.get_value() {
            return Err(format!("not sufficient funds {}, tx value {}", account.get_balance(), tx.get_value()));
        }

        if account.get_nonce() + 1 != tx.get_nonce() {
            return Err(format!("invalid nonce {}, tx value {}", account.get_nonce(), tx.get_nonce()));
        }
        Ok(())
    }

    pub fn get_nonce(&self, addr: &Address) -> u64 {
        let chain = self.blockchain.read().unwrap();
        let state = chain.state_at(chain.current_block().state_root());
        let runtime = Balance::new(Interpreter::new(state));
        let account = runtime.get_account(addr.clone());

        account.get_nonce()
    }
}