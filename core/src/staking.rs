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
use crate::storage::{List, ListEntry};
use crate::state::StateDB;
use crate::balance::Balance;
use crate::runtime::Interpreter;

#[derive(Copy, Clone)]
enum StatePrefix {
    /// Validators list key
    Validator = 2,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq)]
pub struct LockingBalance {
    pub amount: u128,
    pub height: u64,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq)]
pub struct Validator {
    pub address: Address,
    pub pubkey: Vec<u8>,
    pub balance: u128,
    pub effective_balance: u128,
    pub activate_height: u64,
    pub exit_height: u64,
    pub deposit_queue: Vec<LockingBalance>,
    pub unlocked_queue: Vec<LockingBalance>,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug, PartialEq)]
pub struct MsgValidatorCreate {
    pub pubkey: Vec<u8>,
    pub amount: u128,
}

impl Validator {
    pub fn create(addr: Address) -> Self {
        Validator {
            address: addr,
            pubkey: Vec::new(),
            balance: 0,
            effective_balance: 0,
            activate_height: 0,
            exit_height: 0,
            deposit_queue: Vec::new(),
            unlocked_queue: Vec::new(),
        }
    }

    pub fn map_key(&self) -> Hash {
        let mut raw = vec![];
        raw.extend_from_slice(Hash::from_bytes(self.address.as_slice()).as_bytes());
        let position = Hash::from_bytes(&(StatePrefix::Validator as u64).to_be_bytes()[..]);
        raw.extend_from_slice(position.as_bytes());

        Hash(hash::blake2b_256(&raw))
    }

    pub fn key_index(addr: &Address) -> Hash {
        let mut raw = vec![];
        raw.extend_from_slice(Hash::from_bytes(addr.as_slice()).as_bytes());
        let position = Hash::from_bytes(&(StatePrefix::Validator as u64).to_be_bytes()[..]);
        raw.extend_from_slice(position.as_bytes());

        Hash(hash::blake2b_256(&raw))
    }
}

pub struct Staking {
    pub validators: List<Validator>,
    pub state_db: Rc<RefCell<StateDB>>,
    pub interpreter: Interpreter,
}

impl Staking {
    pub fn new(runner: Interpreter) -> Self {
        let head_key = Hash::from_bytes(&(StatePrefix::Validator as u64).to_be_bytes()[..]);
        Staking {
            validators: List::new(head_key),
            state_db: runner.statedb(),
            interpreter: runner,
        }
    }

    pub fn from_state(runner: Interpreter) -> Self {
        let head_key = Hash::from_bytes(&(StatePrefix::Validator as u64).to_be_bytes()[..]);
        Staking {
            validators: List::new(head_key),
            state_db: runner.statedb(),
            interpreter: runner,
        }
    }

    pub fn insert(&mut self, item: &Validator) {
        let head = self.state_db.borrow_mut().get_storage(&self.validators.head_key);
        if head.is_none() {
            let entry = ListEntry {
                pre: None,
                next: None,
                payload: item,
            };
            let encoded: Vec<u8> = bincode::serialize(&entry).unwrap();
            self.state_db.borrow_mut().set_storage(item.map_key(), &encoded);
            self.state_db.borrow_mut().set_storage(self.validators.head_key, item.map_key().as_bytes());
        } else {
            let head_ref = Hash::from_bytes(&head.unwrap()[..]);

            let entry = ListEntry {
                pre: None,
                next: Some(head_ref),
                payload: item,
            };
            self.state_db.borrow_mut().set_storage(item.map_key(), &bincode::serialize(&entry).unwrap());
            {
                // replace next entry of inserted item
                let encoded = self.state_db.borrow().get_storage(&head_ref).unwrap();
                let mut next: ListEntry<Validator> = bincode::deserialize(&encoded).unwrap();
                next.pre = Some(item.map_key());
                let serialized: Vec<u8> = bincode::serialize(&next).unwrap();
                self.state_db.borrow_mut().set_storage(next.payload.map_key(), &serialized);
            }
            // place reference of first item at head
            self.state_db.borrow_mut().set_storage(self.validators.head_key, item.map_key().as_bytes());
        }
    }

    pub fn set_item(&mut self, item: &Validator) {
        let encoded = self.state_db.borrow().get_storage(&item.map_key());
        if encoded.is_none() {
            self.insert(item);
        } else {
            let mut entry: ListEntry<Validator> = bincode::deserialize(&encoded.unwrap()).unwrap();
            entry.payload = item.clone();
            self.state_db.borrow_mut().set_storage(item.map_key(), &bincode::serialize(&entry).unwrap());
        }
    }

    pub fn delete(&mut self, addr: &Address) {
        let encoded = match self.state_db.borrow().get_storage(&Validator::key_index(addr)) {
            Some(i) => i,
            None => return,
        };

        let item: ListEntry<Validator> = bincode::deserialize(&encoded).unwrap();
        if item.pre.is_some() {
            let encoded = self.state_db.borrow().get_storage(&item.pre.unwrap()).unwrap();
            let mut pre_node: ListEntry<Validator> = bincode::deserialize(&encoded).unwrap();
            pre_node.next = item.next;
            self.state_db.borrow_mut().set_storage(item.pre.unwrap(), &bincode::serialize(&pre_node).unwrap());
        } else {
            if let Some(next) = item.next {
                // set head to next item
                self.state_db.borrow_mut().set_storage(self.validators.head_key, next.as_bytes());
            } else {
                // EMPTY list, remove head ref
                self.state_db.borrow_mut().remove_storage(self.validators.head_key);
            }
        }

        if item.next.is_some() {
            let encoded = self.state_db.borrow().get_storage(&item.next.unwrap()).unwrap();
            let mut next_node: ListEntry<Validator> = bincode::deserialize(&encoded).unwrap();
            next_node.pre = item.pre;
            self.state_db.borrow_mut().set_storage(item.next.unwrap(), &bincode::serialize(&next_node).unwrap());
        }
        // delete target from trie db
        self.state_db.borrow_mut().remove_storage(Validator::key_index(addr));
    }

    pub fn validator_set(&self) -> Vec<Validator> {
        let mut items = Vec::new();

        let head_ref = match self.state_db.borrow().get_storage(&self.validators.head_key) {
            Some(i) => i,
            None => return items,
        };

        // iterate list items
        let mut next_ref = Some(Hash::from_bytes(&head_ref));
        while next_ref.is_some() {
            let encoded = self.state_db.borrow().get_storage(&next_ref.unwrap()).unwrap();
            let item: ListEntry<Validator> = bincode::deserialize(&encoded).unwrap();
            items.push(item.payload);
            next_ref = item.next;
        }
        items
    }

    pub fn get_validator(&self, addr: &Address) -> Option<Validator> {
        // let head = self.state().get_storage(&self.validators.head_key);
        // if head.is_none() {
        //     return
        // }
        let encoded = match self.state_db.borrow().get_storage(&Validator::key_index(addr)) {
            Some(i) => i,
            None => return None,
        };
        let obj: ListEntry<Validator> = bincode::deserialize(&encoded).unwrap();
        Some(obj.payload)
    }

    #[allow(unused_variables)]
    pub fn validate(&mut self, addr: &Address, pubkey: Vec<u8>, amount: u128) {
        if self.get_validator(addr).is_some() {
            // the address already joined the validator
            return
        }
        // mark the epoch in which validator take effect
        let activate: u64 = 0;
        // create and initialize validator
        let validator = Validator {
            address: *addr,
            pubkey: pubkey,
            balance: amount,
            effective_balance: amount,
            activate_height: 0,
            exit_height: 0,
            deposit_queue: Vec::new(),
            unlocked_queue: Vec::new(),
        };
        self.insert(&validator);

        // self.interpreter.lock_balance(*addr, amount);
        {
            let mut state = Balance::from_state(self.interpreter.clone());
            state.lock_balance(*addr, amount);
        }
    }

    pub fn deposit(&mut self, addr: &Address, amount: u128) {
        let mut validator = match self.get_validator(&addr) {
            Some(i) => i,
            None => return,
        };
        validator.deposit_queue.push(LockingBalance{amount: amount, height: 0});
        validator.balance += amount;
        self.set_item(&validator);

        // self.interpreter.lock_balance(*addr, amount);
        {
            let mut state = Balance::from_state(self.interpreter.clone());
            state.lock_balance(*addr, amount);
        }
    }

    pub fn activate_deposit(&mut self, addr: &Address) {
        let mut validator = match self.get_validator(&addr) {
            Some(i) => i,
            None => return,
        };
        // available deposit take queue util active epoch
        let epoch: u64 = 0;
        let offset: usize = 0;
        while offset < validator.deposit_queue.len() {
            if validator.deposit_queue[offset].height > epoch {
                break;
            }
            validator.effective_balance += validator.deposit_queue[offset].amount;
        }
        validator.deposit_queue = validator.deposit_queue[..offset].to_vec();
        self.set_item(&validator);
    }

    pub fn exit(&mut self, addr: &Address) {
        let mut validator = match self.get_validator(&addr) {
            Some(i) => i,
            None => return,
        };

        // mark the epoch in which validator exit make block
        validator.exit_height = 0;
        self.set_item(&validator);
    }

    pub fn exec_validate(&mut self, addr: &Address, input: Vec<u8>) {
        let msg: MsgValidatorCreate = match bincode::deserialize(&input) {
            Ok(m) => m,
            Err(_) => return,
        };
        self.validate(addr, msg.pubkey, msg.amount);
    }

    pub fn exec_deposit(&mut self, addr: &Address, input: Vec<u8>) {
        let msg: u128 = match bincode::deserialize(&input) {
            Ok(m) => m,
            Err(_) => return,
        };
        self.deposit(addr, msg);
    }

    #[allow(unused_variables)]
    pub fn exec_exit(&mut self, addr: &Address, input: Vec<u8>) {
        self.exit(addr);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};
    use std::rc::Rc;
    use std::cell::RefCell;
    use env_logger;
    use map_store::{MemoryKV, KVDB};
    use crate::runtime::Interpreter;
    use crate::state::{ArchiveDB, StateDB};
    use crate::types::Address;
    use crate::trie::NULL_ROOT;
    use super::{Validator, Staking};

    #[test]
    fn validator_insert() {
        env_logger::init();
        let backend: Arc<RwLock<dyn KVDB>> = Arc::new(RwLock::new(MemoryKV::new()));
        let db = ArchiveDB::new(Arc::clone(&backend));
        let state_db = Rc::new(RefCell::new(StateDB::from_existing(&db, NULL_ROOT)));
        let addr = Address::default();
        let first_addr = Address::from_hex("0x0000000000000000000000000000000000000001").unwrap();

        let validator = Validator {
            address: addr,
            pubkey: Vec::new(),
            balance: 1,
            effective_balance: 0,
            activate_height: 1,
            exit_height: 0,
            deposit_queue: Vec::new(),
            unlocked_queue: Vec::new(),
        };

        let mut stake = Staking::new(Interpreter::new(state_db.clone()));
        stake.insert(&validator);

        let first = Validator {
            address: first_addr,
            pubkey: Vec::new(),
            balance: 1,
            effective_balance: 0,
            activate_height: 1,
            exit_height: 0,
            deposit_queue: Vec::new(),
            unlocked_queue: Vec::new(),
        };

        stake.insert(&first);

        let item = stake.get_validator(&first_addr).unwrap();
        assert_eq!(item, first);
    }

    #[test]
    fn validator_iter() {
        env_logger::init();
        let backend: Arc<RwLock<dyn KVDB>> = Arc::new(RwLock::new(MemoryKV::new()));
        let db = ArchiveDB::new(Arc::clone(&backend));
        let state_db = Rc::new(RefCell::new(StateDB::from_existing(&db, NULL_ROOT)));
        let addr = Address::default();

        let validator = Validator {
            address: addr,
            pubkey: Vec::new(),
            balance: 1,
            effective_balance: 0,
            exit_height: 0,
            activate_height: 1,
            deposit_queue: Vec::new(),
            unlocked_queue: Vec::new(),
        };

        let mut stake = Staking::new(Interpreter::new(state_db.clone()));
        stake.insert(&validator);

        let items = stake.validator_set();
        assert_eq!(items.len(), 1);
        assert_eq!(stake.get_validator(&addr).unwrap(), validator);
    }

    #[test]
    fn validator_delete() {
        env_logger::init();
        let backend: Arc<RwLock<dyn KVDB>> = Arc::new(RwLock::new(MemoryKV::new()));
        let db = ArchiveDB::new(Arc::clone(&backend));
        let state_db = Rc::new(RefCell::new(StateDB::from_existing(&db, NULL_ROOT)));
        let addr = Address::default();
        let addr_1 = Address::from_hex("0x0000000000000000000000000000000000000001").unwrap();

        let validator = Validator {
            address: addr,
            pubkey: Vec::new(),
            balance: 1,
            effective_balance: 0,
            activate_height: 1,
            exit_height: 0,
            deposit_queue: Vec::new(),
            unlocked_queue: Vec::new(),
        };

        let validator_1 = Validator {
            address: addr_1,
            pubkey: Vec::new(),
            balance: 1,
            effective_balance: 0,
            activate_height: 1,
            exit_height: 0,
            deposit_queue: Vec::new(),
            unlocked_queue: Vec::new(),
        };

        let mut stake = Staking::new(Interpreter::new(state_db.clone()));
        stake.insert(&validator);
        stake.insert(&validator_1);

        stake.delete(&addr);
        let item = stake.get_validator(&addr);

        stake.delete(&addr_1);
        let item = stake.get_validator(&addr_1);

        assert_eq!(stake.validator_set().len(), 0);
        assert_eq!(stake.get_validator(&addr), None);
        assert_eq!(stake.get_validator(&addr_1), None);
    }
}
