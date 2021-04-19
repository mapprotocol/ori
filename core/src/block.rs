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

extern crate ed25519;
extern crate hash;

use std::fmt;
use serde::{Serialize, Deserialize};
// use super::traits::{TxMsg};
use super::transaction::{Transaction};
use super::types::{Hash,Address};
use ed25519::{signature::SignatureInfo,Message,pubkey::Pubkey};
// use hash;
use bincode;

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct VRFProof ([u8; 32], [u8; 32]);

impl VRFProof {
    pub fn new(proof: [u8; 64]) -> Self {
        let mut obj = VRFProof([0; 32], [0; 32]);
        obj.0.copy_from_slice(&proof[..32]);
        obj.1.copy_from_slice(&proof[32..]);
        obj
    }

    pub fn bytes(&self) -> [u8; 64] {
        let mut out = [0u8; 64];
        out[..32].copy_from_slice(&self.0);
        out[32..].copy_from_slice(&self.1);
        out
    }
}

impl fmt::Debug for VRFProof {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.0.iter() {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

/// Block header
#[derive(Serialize, Deserialize, Debug,PartialEq, Eq, Hash)]
#[derive(Copy, Clone)]
pub struct Header {
    pub height: u64,
    pub parent_hash: Hash,
    pub slot: u64,
    pub vrf_output: [u8; 32],
    pub vrf_proof: VRFProof,
    pub tx_root: Hash,
    pub sign_root: Hash,
    pub state_root: Hash,
    pub time: u64,
}

impl Default for Header {
	fn default() -> Self {
		Header {
			height: 0,
            slot: 0,
            vrf_output: [0; 32],
            vrf_proof: VRFProof::new([0; 64]),
            parent_hash: Hash([0; 32]),
            tx_root:  Hash([0;32]),
            sign_root:  Hash([0;32]),
            state_root:  Hash([0;32]),
			time: 0,
		}
	}
}

impl Header {
    pub fn hash(&self) -> Hash {
        let encoded: Vec<u8> = bincode::serialize(&self).unwrap();
        Hash(hash::blake2b_256(encoded))
    }
}

#[derive(Serialize, Deserialize)]
#[derive(Clone,Copy, Default, Debug,PartialEq, Eq, Hash)]
pub struct VerificationItem {
    pub msg:    Hash,
    pub signs:  SignatureInfo,
}

impl VerificationItem {
    pub fn to_msg(&self) -> Message {
        self.msg.to_msg()
    }
    pub fn new(msg: Hash,signs: SignatureInfo) -> Self {
        VerificationItem{msg,signs}
    }
}

#[derive(Serialize, Deserialize)]
#[derive(Debug, Default,Copy, Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct BlockProof(pub [u8;32],pub [u8;32],pub u8);

impl BlockProof {
    pub fn new(t: u8,pk: &[u8]) -> Self {
        if t == 0u8 {
            let mut o1 = [0u8;32];
            o1[..].copy_from_slice(&pk[0..32]);
            BlockProof(o1,[0u8;32],t)
        } else {
            let mut o1 = [0u8;32];
            let mut o2 = [0u8;32];
            o1[..].copy_from_slice(&pk[0..32]);
            o2[..].copy_from_slice(&pk[32..64]);
            BlockProof(o1,o2,t)
        }
    }
    pub fn get_pk(&self, pk: &mut [u8;64]) -> u8 {
        if self.2 == 0u8 {
            pk[0..32].copy_from_slice(&self.0[..]);
        } else {
            pk[0..32].copy_from_slice(&self.0[..]);
            pk[32..64].copy_from_slice(&self.1[..]);
        }
        self.2
    }
    pub fn to_address(&self) -> Address {
        if self.2 == 0u8 {
            Pubkey::from_bytes(&self.0[..]).into()
        } else {
            Address([0u8;20])
        }
    }
}

pub fn get_hash_from_txs(txs: &Vec<Transaction>) -> Hash {
    let data = bincode::serialize(txs).unwrap();
    Hash(hash::blake2b_256(data))
}
pub fn get_hash_from_signs(signs: Vec<VerificationItem>) -> Hash {
    let data = bincode::serialize(&signs).unwrap();
    Hash(hash::blake2b_256(data))
}

#[derive(Debug, Clone, Serialize, Deserialize,PartialEq, Eq, Hash)]
pub struct Block {
    pub header: Header,
    pub signs: Vec<VerificationItem>,
    pub txs:  Vec<Transaction>,
    pub proofs: Vec<BlockProof>,
}

impl Default for Block {
    fn default() -> Self {
        Block {
            header: Default::default(),
            signs:  Vec::new(),
            txs:    Vec::new(),
            proofs: Vec::new(),
        }
    }
}

impl  Block {
    pub fn new(mut header: Header,txs: Vec<Transaction>,signs: Vec<VerificationItem>,proofs: Vec<BlockProof>) -> Self {
        header.tx_root = get_hash_from_txs(&txs);
        header.sign_root = get_hash_from_signs(signs.clone());
        Block{header,signs,txs,proofs}
    }

    #[allow(dead_code)]
    fn header(&self) -> &Header {
		&self.header
    }
    pub fn set_sign_hash(&mut self, h: Hash) {
        self.header.sign_root = h;
    }
    pub fn set_state_root(&mut self, h:Hash) {
        self.header.state_root = h;
    }
    pub fn height(&self) -> u64 {
        self.header.height
    }

    pub fn hash(&self) -> Hash {
        self.header.hash()
    }

    pub fn state_root(&self) -> Hash {
        self.header.state_root
    }

    pub fn get_hash(&self) -> Hash {
       self.header.hash()
    }
    pub fn add_proof(&mut self,proof: BlockProof) {
        self.proofs.push(proof);
    }
    pub fn proof_one(&self) -> Option<&BlockProof> {
        self.proofs.get(0)
    }
    pub fn get_proofs(&self) -> &Vec<BlockProof> {
        &self.proofs
    }
    pub fn add_verify_item(&mut self,item: VerificationItem) {
        self.signs.push(item)
    }
    pub fn get_signs(&self) -> Vec<VerificationItem> {
        self.signs.clone()
    }
    pub fn sign_one(&self) ->Option<&VerificationItem> {
        self.signs.get(0)
    }
    pub fn get_txs(&self) -> &Vec<Transaction> {
        &self.txs
    }
}

pub fn is_equal_hash(hash1: Option<Hash>,hash2: Option<Hash>) -> bool {
    hash1.map_or(false,|v|{hash2.map_or(false,|v2|{ if v == v2 {return true;} else {return false;}})})
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::bincode;

    #[test]
    fn test_header_hash() {
        let head: Header = Default::default();
        let encoded: Vec<u8> = bincode::serialize(&head).unwrap();
        assert_eq!(encoded, vec![0; 48]);
    }

    #[test]
    fn test_encode_option() {
        // The object that we will serialize.
        {
            let target: Option<String>  = Some("hello".to_string());

            let encoded: Vec<u8> = bincode::serialize(&target).unwrap();
            let decoded: Option<String> = bincode::deserialize(&encoded[..]).unwrap();
            assert_eq!(target, decoded);
        }
        {
            let target: Option<String>  = None;

            let encoded: Vec<u8> = bincode::serialize(&target).unwrap();
            assert_eq!(encoded, [0]);
        }
        {
            let target: (Option<String>, Option<String>)  = (None, None);

            let encoded: Vec<u8> = bincode::serialize(&target).unwrap();
            assert_eq!(encoded, [0, 0]);
        }
    }
}
