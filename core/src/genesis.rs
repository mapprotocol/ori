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

use std::rc::Rc;
use std::cell::RefCell;

use ed25519::pubkey::Pubkey;
// use super::{traits::TxMsg};
use super::types::{Hash, Address};
use super::block;
use super::balance::Balance;
use super::block::{Block, BlockProof};
use super::runtime::Interpreter;
use super::state::StateDB;
use super::staking::{Validator, Staking};

#[allow(non_upper_case_globals)]
pub const ed_genesis_priv_key: [u8; 32] = [
    249, 203, 126, 161, 115, 132, 10, 235, 164, 252, 129, 70, 116, 52, 100, 205, 174, 62, 85,
    39, 65, 72, 114, 21, 95, 227, 49, 189, 42, 52, 84, 162,
];

#[allow(non_upper_case_globals)]
pub const ed_genesis_pub_key: [u8; 32] = [
    243, 168, 124, 46, 165, 43, 188, 124, 215, 100, 221, 215, 249, 71, 217, 60, 226, 13, 9,
    72, 114, 24, 80, 73, 118, 31, 251, 38, 82, 192, 147, 7,
];

pub const GENESIS_TIME: u64 = 1597916633;

const ALLOCATION: &[(&str, u128)] = &[
    ("0xd2480451ef35ff2fdd7c69cad058719b9dc4d631", 1000000000000000000),
    // ("0x7411794f635cf645408cd698d5be3a964b5963e1", 1000000000000000000),
];

// validator members (address, pubkey, stake)
const VALIDATORS: &[(&str, &str, u128)] = &[
    ("0xd2480451ef35ff2fdd7c69cad058719b9dc4d631", "0xf3a87c2ea52bbc7cd764ddd7f947d93ce20d094872185049761ffb2652c09307", 0),
    // ("0x7411794f635cf645408cd698d5be3a964b5963e1", "0x2f4037ff722f3dcdf5e3b17f09a16c662c74deb8e2da85086b1cc935c23b64a2", 0),
];

pub fn to_genesis() -> Block {
    let zore_hash = [0u8;32];
    let mut b = Block::default();
    b.header.height = 0;
    b.header.time = GENESIS_TIME;
    b.header.parent_hash = Hash(zore_hash);
    b.proofs.push(BlockProof(ed_genesis_pub_key,[0u8;32],0));
    b.header.tx_root = block::get_hash_from_txs(&b.txs);
    b.header.sign_root = block::get_hash_from_signs(b.signs.clone());
    return b
}

pub fn setup_allocation(db: Rc<RefCell<StateDB>>) -> Hash {
    {
        let interpreter = Interpreter::new(db.clone());
        let mut state = Balance::new(interpreter);
        for &(addr, value) in ALLOCATION {
            state.add_balance(Address::from_hex(addr).unwrap(), value);
        }
        state.commit();
    }
    {
        let interpreter = Interpreter::new(db.clone());
        let mut state = Staking::new(interpreter);
        for &(addr, pk, value) in VALIDATORS.iter().rev() {
            let validator = Validator {
                address: Address::from_hex(addr).unwrap(),
                pubkey: Pubkey::from_hex(pk).to_bytes(),
                balance: 0,
                effective_balance: value,
                activate_height: 0,
                exit_height: 0,
                deposit_queue: Vec::new(),
                unlocked_queue: Vec::new(),
            };
            state.insert(&validator);
        }
    }
    db.borrow_mut().commit();
    db.borrow().root()
}
