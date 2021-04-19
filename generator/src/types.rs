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

use map_core::types::Hash;
use ed25519::{pubkey::Pubkey};

pub type RngSeed = [u8; 32];

/// Validator's stake and crypto
#[derive(Debug, Clone)]
pub struct ValidatorStake {
    pub pubkey: [u8; 32],
    pub stake_amount: u128,
    pub sid:        u64,
    pub validator:  bool,
}

impl ValidatorStake {
    pub fn set_sid(&mut self, i: u64) {
        self.sid = i;
    }

    pub fn get_sid(&self) -> u64 {
        self.sid
    }

    pub fn is_validator(&self) -> bool {
        self.validator
    }

    pub fn get_my_id(&self) -> Hash {
        Hash::make_hash(&self.pubkey[..])
    }

    pub fn get_pubkey(&self) -> Pubkey {
        Pubkey::from_bytes(&self.pubkey)
    }
}

impl From<ValidatorStake> for Pubkey {
    fn from(v: ValidatorStake) -> Self {
        Pubkey::from_bytes(&v.pubkey)
    }
}

impl From<ValidatorStake> for Stakeholder {
    fn from(v: ValidatorStake) -> Self {
        Stakeholder{
            name:   String::from_utf8_lossy(&v.pubkey[..4]).to_string(),
            coins:  v.stake_amount,
            index:  -1 as i32,
        }
    }
}

/// Staking item used to calculate fts
#[derive(Debug, Clone)]
pub struct Stakeholder {
    pub name:   String,
    pub coins:  u128,
    pub index:  i32,
}

impl Stakeholder {
    pub fn get_name(&self) -> String {
        return self.name.clone()
    }
    pub fn get_coins(&self) -> u128 {
        return self.coins
    }
    pub fn to_bytes(&self) -> Vec<u8>{
        format!("{}{}",self.name,self.coins).into_bytes()
    }
    pub fn to_string(&self) -> String {
        return self.name.clone()
    }
    pub fn clone(&self) -> Self {
        return Stakeholder{
            name:	self.name.clone(),
            coins: 	self.coins,
            index:  self.index,
        }
    }
    pub fn get_index(&self) -> i32 {
        self.index
    }
    pub fn set_index(&mut self,i: i32) {
        self.index = i;
    }
}
