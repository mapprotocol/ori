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

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::rc::Rc;
use std::cell::RefCell;

use errors::Error;
use map_consensus::poa;
use map_core;
use map_core::trie::NULL_ROOT;
use map_core::block::{Block, Header};
use map_core::genesis;
#[allow(unused_imports)]
use map_core::state::{ArchiveDB, StateDB};
use map_core::types::{Hash, Address};
use map_core::runtime::Interpreter;
use map_core::balance::Balance;
use executor::Executor;
use map_store;
use map_store::mapdb::MapDB;
use crate::store::ChainDB;

use super::BlockChainErrorKind;

pub struct BlockChain {
    db: ChainDB,
    state_backend: ArchiveDB,
    validator: Validator,
    genesis: Block,
    #[allow(dead_code)]
    consensus: poa::POA
}

impl BlockChain {
    pub fn new(datadir: PathBuf, key: String) -> Self {
        info!("using datadir {}", datadir.display());
        let db_cfg = map_store::Config::new(datadir.clone());
        let backend;
        {
            let mut dir = datadir.clone();
            dir.push("data");
            let db = MapDB::open(map_store::Config::new(dir.clone())).unwrap();
            let kv: Arc<RwLock<dyn map_store::KVDB>> = Arc::new(RwLock::new(db));
            backend = ArchiveDB::new(Arc::clone(&kv));
        }

        BlockChain {
            db: ChainDB::new(db_cfg).unwrap(),
            genesis: genesis::to_genesis(),
            state_backend: backend,
            validator: Validator{},
            consensus: poa::POA::new_from_string(key),
        }
    }

    pub fn setup_genesis(&mut self) -> Hash {
        let state_db = Rc::new(RefCell::new(StateDB::from_existing(&self.state_backend, NULL_ROOT)));
        let root = genesis::setup_allocation(state_db.clone());
        self.genesis.set_state_root(root);

        self.db.write_block(&self.genesis).expect("can not write block");
        self.db.write_head_hash(self.genesis.hash()).expect("can not wirte head");
        info!("setup genesis hash={}", self.genesis.hash());
        self.genesis.hash()
    }

    pub fn load(&mut self) {
        let block_zero = self.get_block_by_number(0);
        if block_zero.is_none() {
            self.setup_genesis();
        } else {
            self.genesis = block_zero.unwrap();
            let current = self.current_block();
            info!("load genesis hash={}", self.genesis.hash());
            info!("load block height={} hash={}", current.height(), current.hash());
        }
    }

    pub fn statedb(&self) -> &ArchiveDB {
        &self.state_backend
    }

    pub fn state_at(&self, root: Hash) -> Rc<RefCell<StateDB>> {
        Rc::new(RefCell::new(StateDB::from_existing(&self.state_backend, root)))
    }

    pub fn genesis_hash(&self) -> Hash {
        self.genesis.hash()
    }

    pub fn current_block(&self) -> Block {
        self.db.head_block().unwrap()
    }

    #[allow(unused_variables)]
    pub fn exits_block(&self, h: Hash, num: u64) -> bool {
        self.db.get_block_by_number(num).is_some()
    }

    pub fn check_previous(&self, header: &Header) -> bool {
        self.db.get_block(&header.parent_hash).is_some()
    }

    pub fn get_block_by_number(&self, num: u64) -> Option<Block> {
        self.db.get_block_by_number(num)
    }

    pub fn get_block(&self, hash: Hash) -> Option<Block> {
        self.db.get_block(&hash)
    }

    pub fn get_header_by_number(&self, num: u64) -> Option<Header> {
        self.db.get_header_by_number(num)
    }

    pub fn apply_transactions(&self, root: Hash, b: &Block) -> Hash {
        let statedb = self.state_at(root);
        let h = Executor::exc_txs_in_block(&b, &mut Balance::new(Interpreter::new(statedb)), &Address::default()).unwrap();
        h
    }

    pub fn insert_block(&mut self, block: Block) -> Result<(), Error> {
        self.import_block(&block)
    }

    pub fn import_block(&mut self, block: &Block) -> Result<(), Error> {
        // Already in chain
        if self.exits_block(block.hash(), block.height()) {
            return Err(BlockChainErrorKind::KnownBlock.into());
        }

        if !self.check_previous(&block.header) {
            return Err(BlockChainErrorKind::UnknownAncestor.into());
        }

        let current = self.current_block();

        if block.header.parent_hash != current.hash() {
            return Err(BlockChainErrorKind::UnknownAncestor.into());
        }

        self.validator.validate_header(self, &block.header)?;
        self.validator.validate_block(self, block)?;

        if block.state_root() != self.apply_transactions(current.state_root(), block) {
            return Err(BlockChainErrorKind::InvalidState.into());
        }

        self.db.write_block(&block).expect("can not write block");
        self.db.write_head_hash(block.header.hash()).expect("can not wirte head");
        info!("insert block, height={}, hash={}, previous={}", block.height(), block.hash(), block.header.parent_hash);
        Ok(())
    }

}

pub struct Validator;

impl Validator {
    #[allow(unused_variables)]
    pub fn validate_block(&self, chain: &BlockChain, block: &Block) -> Result<(), Error> {
        if block.header.sign_root != map_core::block::get_hash_from_signs(block.signs.clone()) {
            return Err(BlockChainErrorKind::MismatchHash.into());
        }

        if block.header.tx_root != map_core::block::get_hash_from_txs(&block.txs) {
            return Err(BlockChainErrorKind::MismatchHash.into());
        }

        Ok(())
    }

    pub fn validate_header(&self, chain: &BlockChain, header: &Header) -> Result<(), Error> {
        // Ensure block parent exists on chain
        let pre = match chain.get_block(header.parent_hash) {
            Some(b) => b,
            None => return Err(BlockChainErrorKind::UnknownAncestor.into()),
        };

        // Ensure block height increase by one
        if header.height != pre.header.height + 1 {
            return Err(BlockChainErrorKind::InvalidBlockHeight.into());
        }

        // Ensure block time interval
        if header.time <= pre.header.time {
            return Err(BlockChainErrorKind::InvalidBlockTime.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use map_core::balance::Balance;
    use std::time::SystemTime;

    #[test]
    fn test_init() {
        let mut chain = BlockChain::new(PathBuf::from("./mapdata"),"".to_string());
        let mut state = Balance::new(PathBuf::from("./balance"));
        chain.load(&mut state);
        assert_eq!(chain.genesis.height(), 0);
        assert_eq!(chain.genesis.header.parent_hash, Hash::default());
        assert!(chain.get_block_by_number(0).is_some());
    }

    #[test]
    fn test_insert_empty() {
        let mut chain = BlockChain::new(PathBuf::from("./mapdata"),"".to_string());
        let mut state = Balance::new(PathBuf::from("./balance"));
        chain.load(&mut state);
        {
            let block = Block {
                header: Header{
                    height: 1,
                    ..Default::default()
                },
                ..Block::default()
            };
            let ret = chain.insert_block(block);
            assert!(ret.is_err());
        }

        {
            let mut block = Block::default();
            block.header.height = 1;
            block.header.parent_hash = chain.genesis_hash();
            let ret = chain.insert_block(block);
            assert!(ret.is_err());
        }

        {
            let mut block = Block::default();
            block.header.height = 1;
            block.header.parent_hash = chain.genesis_hash();
            block.header.time = SystemTime::now().duration_since(
                SystemTime::UNIX_EPOCH).unwrap().as_secs();
            let ret = chain.insert_block(block);
            assert!(ret.is_err());
        }
    }
}
