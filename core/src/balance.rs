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

use std::cell::RefCell;
use std::rc::Rc;
use serde::{Serialize, Deserialize};
use bincode;
use hash;
use crate::types::{Hash, Address};
use crate::state::StateDB;
use crate::trie::NULL_ROOT;
use crate::transaction;
use crate::runtime::Interpreter;

const BALANCE_POS: u64 = 1;
const NONCE_POS: u64 = 2;

#[derive(Serialize, Deserialize)]
#[derive(Default, Copy, Clone, Debug, PartialEq)]
pub struct Account {
    // Available balance of the account
    balance: u128,
    // Nonce of the account transaction count
    nonce: u64,
    // Balance which is reserved by other modules
    locked_balance: u128,
}

impl Account {
    pub fn get_balance(&self) -> u128 {
        self.balance
    }
    pub fn get_nonce(&self) -> u64 {
        self.nonce
    }
}

#[allow(dead_code)]
pub struct Balance {
    // cache: HashMap<Hash, Account>,
    treedb: Rc<RefCell<StateDB>>,
    interpreter: Interpreter,
    root_hash: Hash,
}

impl Balance {
    pub fn new(runner: Interpreter) -> Self {
        Balance {
            // cache: HashMap::new(),
            treedb: runner.statedb(),
            interpreter: runner,
            root_hash: NULL_ROOT,
        }
    }

    pub fn from_state(runner: Interpreter) -> Self {
        Balance {
            // cache: HashMap::new(),
            treedb: runner.statedb(),
            interpreter: runner,
            root_hash: NULL_ROOT,
        }
    }

    pub fn balance(&self, addr: Address) -> u128 {
        // let addr_hash = Self::address_key(addr);
        // let account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let account = self.load_account(addr);
        account.balance
    }

    pub fn nonce(&self, addr: Address) -> u64 {
        // let addr_hash = Self::address_key(addr);
        // let account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let account = self.load_account(addr);
        account.nonce
    }

    pub fn locked(&self, addr: Address) -> u128 {
        // let addr_hash = Self::address_key(addr);
        // let account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let account = self.load_account(addr);
        account.locked_balance
    }

    pub fn get_account(&self, addr: Address) -> Account {
        // let addr_hash = Self::address_key(addr);
        // let account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let account = self.load_account(addr);
        account
    }

    pub fn inc_nonce(&mut self, addr: Address) {
        // let addr_hash = Self::address_key(addr);
        // let mut account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let mut account = self.load_account(addr);
        account.nonce += 1;
        self.set_account(addr, &account);
        // self.cache.insert(addr_hash, account);
    }

    pub fn add_balance(&mut self, addr: Address, value: u128) {
        // let addr_hash = Self::address_key(addr);
        // let mut account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let mut account = self.load_account(addr);
        account.balance += value;
        self.set_account(addr, &account);
        // self.cache.insert(addr_hash, account);
    }

    pub fn sub_balance(&mut self, addr: Address, value: u128) {
        // let addr_hash = Self::address_key(addr);
        // let mut account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let mut account = self.load_account(addr);
        account.balance -= value;
        self.set_account(addr, &account);
        // self.cache.insert(addr_hash, account);
    }

    pub fn slash(&mut self, addr: Address, value: u128) {
        // let addr_hash = Self::address_key(addr);
        // let mut account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let mut account = self.load_account(addr);
        account.locked_balance -= value;
        self.set_account(addr, &account);
        // self.cache.insert(addr_hash, account);
    }

    pub fn lock_balance(&mut self, addr: Address, value: u128) {
        // let addr_hash = Self::address_key(addr);
        // let mut account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let mut account = self.load_account(addr);
        account.balance -= value;
        account.locked_balance += value;
        self.set_account(addr, &account);
        // self.cache.insert(addr_hash, account);
    }

    pub fn unlock_balance(&mut self, addr: Address, value: u128) {
        // let addr_hash = Self::address_key(addr);
        // let mut account = match self.cache.get(&addr_hash) {
        //     Some(v) => v.clone(),
        //     None => self.load_account(addr),
        // };
        let mut account = self.load_account(addr);
        account.balance += value;
        account.locked_balance -= value;
        self.set_account(addr, &account);
        // self.cache.insert(addr_hash, account);
    }

    // pub fn reset(&mut self) {
    //     self.cache.clear();
    // }

    pub fn transfer(&mut self, from_addr: Address, to_addr: Address, amount: u128) {
        if self.balance(from_addr) >= amount {
            self.sub_balance(from_addr, amount);
            self.add_balance(to_addr, amount);
        } else {
            // take transaction fee
        }
    }

    pub fn exec_transfer(&mut self, from_addr: Address, input: Vec<u8>) {
        let msg: transaction::balance_msg::MsgTransfer = bincode::deserialize(&input).unwrap();
        self.transfer(from_addr, msg.receiver, msg.value);
    }

    pub fn commit(&mut self) -> Hash {
        // for (addr_hash, account) in self.cache.iter() {
        //     let encoded: Vec<u8> = bincode::serialize(&account).unwrap();
        //     self.treedb.borrow_mut().set_storage(*addr_hash, &encoded);
        // }
        self.treedb.borrow_mut().commit();
        // self.cache.clear();
        self.root_hash = self.treedb.borrow().root();
        self.root_hash
    }

    pub fn load_account(&self, addr: Address) -> Account {
        // let serialized = match self.cache.get(&Self::address_key(addr)) {
        //     Some(s) => s,
        //     None => return Account::default(),
        // };
        let serialized = match self.treedb.borrow().get_storage(&Self::address_key(addr)) {
            Some(s) => s,
            None => return Account::default(),
        };

        let obj: Account = bincode::deserialize(&serialized).unwrap();
        obj
    }

    pub fn set_account(&mut self, addr: Address, account: &Account) {
        let encoded: Vec<u8> = bincode::serialize(account).unwrap();
        self.treedb.borrow_mut().set_storage(Self::address_key(addr), &encoded);
    }

    pub fn load_root(&mut self, root: Hash) {
        self.root_hash = root;
    }

    /// Storage hash key of account
    pub fn address_key(addr: Address) -> Hash {
        let h = Hash::from_bytes(addr.as_slice());
        Hash(hash::blake2b_256(&h.to_slice()))
    }

    /// Storage hash key of account balance
    pub fn balance_key(addr: Address) -> Hash {
        let mut raw = vec![];
        {
            let h = Hash::from_bytes(addr.as_slice());
            raw.extend_from_slice(h.to_slice());
        }
        {
            let h = Hash::from_bytes(&BALANCE_POS.to_be_bytes()[..]);
            raw.extend_from_slice(h.to_slice());
        }
        Hash(hash::blake2b_256(&raw))
    }

    /// Storage hash key of account nonce
    pub fn nonce_key(addr: Address) -> Hash {
        let mut raw = vec![];
        {
            let h = Hash::from_bytes(addr.as_slice());
            raw.extend_from_slice(h.to_slice());
        }
        {
            let h = Hash::from_bytes(&NONCE_POS.to_be_bytes()[..]);
            raw.extend_from_slice(h.to_slice());
        }
        Hash(hash::blake2b_256(&raw))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};
    use std::cell::RefCell;
    use std::rc::Rc;
    use env_logger;
    use map_store::{MemoryKV, KVDB};
    use crate::state::{ArchiveDB, StateDB};
    use crate::types::Address;
    use crate::runtime::Interpreter;
    use crate::trie::NULL_ROOT;
    use super::{Balance, Account};

    #[test]
    fn account_set() {
        let backend: Arc<RwLock<dyn KVDB>> = Arc::new(RwLock::new(MemoryKV::new()));
        let db = ArchiveDB::new(Arc::clone(&backend));
        let state_db = Rc::new(RefCell::new(StateDB::from_existing(&db, NULL_ROOT)));
        let mut state = Balance::new(Interpreter::new(state_db.clone()));

        let addr = Address::default();
        let mut account = state.load_account(addr);
        assert_eq!(account, Account::default());

        let v1 = Account {
            balance: 1,
            nonce: 1,
            locked_balance: 0,
        };
        state.set_account(addr, &v1);
        account = state.load_account(addr);
        assert_eq!(account, v1);
    }

    #[test]
    fn account_transfer() {
        env_logger::init();
        let backend: Arc<RwLock<dyn KVDB>> = Arc::new(RwLock::new(MemoryKV::new()));
        let db = ArchiveDB::new(Arc::clone(&backend));
        let state_db = Rc::new(RefCell::new(StateDB::from_existing(&db, NULL_ROOT)));
        let mut state = Balance::new(Interpreter::new(state_db.clone()));

        let addr = Address::default();
        state.set_account(addr, &Account {
            balance: 1,
            nonce: 1,
            locked_balance: 0,
        });

        let receiver = Address([1; 20]);
        state.set_account(receiver, &Account {
            balance: 0,
            nonce: 0,
            locked_balance: 0,
        });

        state.transfer(addr, receiver, 1);
        state.commit();

        {
            // Reload statedb
            let state = Balance::new(Interpreter::new(state_db.clone()));
            let account = state.load_account(receiver);
            assert_eq!(account.balance, 1);
            assert_eq!(state.balance(receiver), 1);
        }
    }

    #[test]
    fn account_lock() {
        let backend: Arc<RwLock<dyn KVDB>> = Arc::new(RwLock::new(MemoryKV::new()));
        let db = ArchiveDB::new(Arc::clone(&backend));
        let state_db = Rc::new(RefCell::new(StateDB::from_existing(&db, NULL_ROOT)));
        let mut state = Balance::new(Interpreter::new(state_db.clone()));
        let addr = Address::default();
        let lock_1: u128 = 1;

        let v1 = Account {
            balance: 1,
            nonce: 1,
            locked_balance: 0,
        };
        state.set_account(addr, &v1);
        state.lock_balance(addr, lock_1);
        // state.commit();

        assert_eq!(state.locked(addr), lock_1);
    }
}
