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

#[macro_use]
extern crate log;
extern crate errors;

use core::transaction::Transaction;
use core::balance::Balance;
use core::types::{Hash, Address};
use core::block::{Block};
use errors::{Error,InternalErrorKind};

#[allow(non_upper_case_globals)]
const transfer_fee: u128 = 10000;


pub struct Executor;

impl Executor {
    pub fn exc_txs_in_block(b: &Block, state: &mut Balance, miner_addr: &Address) -> Result<Hash,Error> {
        let txs = b.get_txs();
        // let mut h = Hash([0u8;32]);
        for tx in txs {
            Executor::exc_transfer_tx(tx,state)?;
            state.add_balance(*miner_addr, transfer_fee);
        }

        Ok(state.commit())
    }

    // handle the state for the tx,caller handle the gas of tx
    pub fn exc_transfer_tx(tx: &Transaction, state: &mut Balance) -> Result<Hash, Error> {
        let from_addr = tx.get_from_address();
        let to_addr = tx.get_to_address();

        Executor::verify_tx_sign(&tx)?;
        // Ensure balance and nance field available
        let from_account = state.get_account(from_addr);
        if tx.get_nonce() != from_account.get_nonce() + 1 {
            return Err(InternalErrorKind::InvalidTxNonce.into());
        }
        if tx.get_value() + transfer_fee > from_account.get_balance() {
            return Err(InternalErrorKind::BalanceNotEnough.into());
        }

        state.sub_balance(from_addr, transfer_fee);
        state.inc_nonce(from_addr);

        state.transfer(from_addr, to_addr, tx.get_value());
        debug!("Apply transaction send={}", from_addr);
        Ok(Hash::default())
    }

    // handle the state for the contract
    pub fn exc_contract_tx() -> Result<(),Error> {
        Ok(())
    }
    fn verify_tx_sign(tx: &Transaction) -> Result<(),Error> {
        tx.verify_sign()
    }
}

#[cfg(test)]
pub mod tests {
    extern crate ed25519;
    use ed25519::{privkey::PrivKey,pubkey::Pubkey,generator::Generator};
    use core::balance::Balance;
    use core::types::{Hash, Address};
    use core::transaction::Transaction;
    use std::path::PathBuf;
    use super::Executor;
    use bytes::Bytes;

    pub fn get_pair() -> (PrivKey,Pubkey) {
        Generator::default().new()
    }
    #[test]
    pub fn test_tx_execute() {
        let path = PathBuf::from("testdb_04".to_string());
        let mut state = Balance::new(path);
        let user1 = get_pair();
        let addr1 = user1.1.into();
        let const_value = 100000u128;
        let tval = 100u128;
        state.add_balance(addr1, const_value);
        state.inc_nonce(addr1);
        let hex_addr = "0000000000000000000000000000000000000001";
        let addr2 = Address::from_hex(hex_addr).unwrap();
        let tx = Transaction::new(addr1, addr2, 2,
             10, 10, tval, Bytes::new());
        match Executor::exc_transfer_tx(&tx, &mut state) {
            Ok(h) => println!("root:{:?}",h),
            Err(e) => {println!("err:{:?}",e);return;},
        };
        let val2 = state.balance(addr2);
        assert_eq!(val2,tval);
    }
}
