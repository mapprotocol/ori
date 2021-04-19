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

extern crate core;
extern crate ed25519;

use super::{traits::IConsensus,ConsensusErrorKind};
use map_core::block::{self,Block,BlockProof,VerificationItem};
use map_core::types::{Hash,Address};
use map_core::genesis::{ed_genesis_priv_key,ed_genesis_pub_key};
use ed25519::{pubkey::Pubkey,privkey::PrivKey,signature::SignatureInfo};
use std::cmp::Ordering;
use errors::Error;

#[allow(non_upper_case_globals)]
const poa_Version: u32 = 1;

pub struct POA {
    validator: [u8;32],
}

impl IConsensus for POA {
    fn version() -> u32 {
        poa_Version
    }
}

impl POA {
    pub fn new(v: Option<[u8;32]>) -> Self {
        if let Some(val) = v {
            Self{
                validator:  val,
            }
        } else {
            Self{
                validator:  ed_genesis_priv_key,
            }
        }
    }
    pub fn new_from_string(priv_key : String) -> Self {
        if priv_key.len() < 32 {
            return  POA::new(None);
        }
        match PrivKey::from_hex(&priv_key) {
            Ok(pkey) => POA::new(Some(pkey.to_bytes())),
            Err(e) => {
                info!("it's wrong pirv_key in new_from_string function, err={:?}, key={}", e, priv_key);
                POA::new(None)
            },
        }
    }
    fn get_local_pk(&self) -> Option<Vec<u8>> {
        if let Ok(pk) = PrivKey::from_bytes(&self.validator[..]).to_pubkey() {
            Some(pk.to_bytes())
        } else {
            None
        }
    }
    fn is_poa_sign(&self,pk: Vec<u8>) -> bool {
        if let Some(lpk) = self.get_local_pk() {
            if lpk.len() == pk.len() && Ordering::Equal == lpk.cmp(&pk) {
                return true
            }
        }
        false
    }
    pub fn sign_block(t: u8,pkey: Option<PrivKey>, b: Block) -> Result<Block,Error> {
        let _h = b.get_hash();
        match pkey {
            Some(p) => {
                if t == 0u8 {
                    let h = b.get_hash();
                    let signs = p.sign(h.to_slice())?;
                    info!("sign block with genesis privkey, height={}, hash={}", b.height(), h);
                    POA::add_signs_to_block(h,signs,b)
                } else {
                    Ok(b)
                }
            },
            None => {
                if t == 0u8 {
                    let pkey = PrivKey::from_bytes(&ed_genesis_priv_key);
                    let h = b.get_hash();
                    let signs = pkey.sign(h.to_slice())?;
                    info!("sign block with genesis privkey, height={}, hash={}", b.height(), h);
                    POA::add_signs_to_block(h,signs,b)
                } else {
                    Ok(b)
                }
            },
        }
    }
    fn add_signs_to_block(h:Hash,signs: SignatureInfo,mut b: Block) -> Result<Block,Error> {
        let signs = VerificationItem::new(h,signs);
        b.add_verify_item(signs);
        let signs = b.get_signs();
        let h = block::get_hash_from_signs(signs);
        b.set_sign_hash(h);
        Ok(b)
    }
    #[allow(dead_code)]
    fn add_proof_to_block(t: u8,pk: &[u8],mut b: Block) -> Result<Block,Error> {
        let proof = BlockProof::new(t,pk);
        b.add_proof(proof);
        Ok(b)
    }
    pub fn finalize_block(&self,mut b: Block,h: Hash) -> Result<Block,Error> {
        // sign with default priv key
        b.set_state_root(h);
        POA::sign_block(0u8,Some(PrivKey::from_bytes(&self.validator[..])),b)
    }
    pub fn verify(&self,b: &Block) -> Result<(),Error> {
        let proof = b.proof_one();
        match proof {
            Some(&v) => {
                let sign_info = b.sign_one();
                match sign_info {
                    Some(&v2) => self.poa_verify(&v,&v2),
                    None => Err(ConsensusErrorKind::NoneSign.into()),
                }
            },
            None => {
                // get proof from genesis
                if let Some(pk) = self.get_local_pk() {
                    let proof = BlockProof::new(0u8,pk.as_slice());
                    let sign_info = b.sign_one();
                    match sign_info {
                        Some(&v2) => self.poa_verify(&proof,&v2),
                        None => Err(ConsensusErrorKind::NoneSign.into()),
                    }
                } else {
                    Err(ConsensusErrorKind::InvalidProof.into())
                }
            },
        }
    }

    #[allow(non_snake_case)]
    fn poa_verify(&self,proof: &BlockProof, vInfo: &VerificationItem) -> Result<(),Error> {
        let pk0 = &mut [0u8;64];
        let t = proof.get_pk(pk0);
        if t == 0u8 {       // ed25519
            let p_pk = pk0[0..32].to_vec();
            if !self.is_poa_sign(p_pk) {
                return Err(ConsensusErrorKind::AnotherPk.into());
            }
            let mut a1 = [0u8;32];
            a1[..].copy_from_slice(&pk0[0..32]);
            let pk = Pubkey::from_bytes(&a1);
            let msg = vInfo.to_msg();
            pk.verify(&msg,&vInfo.signs)
        } else {
            Ok(())
        }
    }
    pub fn get_interval() -> u64 {
        2000u64
    }
    pub fn get_default_miner() -> Address {
        let mut pk = [0u8;32];
        pk[..].copy_from_slice(&ed_genesis_pub_key[..]);
        Pubkey::from_bytes(&pk[..]).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    #[test]
    fn test_verify() {
        println!("begin verify");
        let h = Hash([0u8;32]);
        let pkey = PrivKey::from_bytes(&ed_genesis_priv_key);
        let signs = pkey.sign(&h.0).unwrap();

        let pk = Pubkey::from_bytes(&ed_genesis_pub_key);
        let msg = h.to_msg();
        let res = pk.verify(&msg,&signs);
        match res {
            Ok(()) => println!("verify ok"),
            Err(e) => println!("Error: {:?}", e),
        }
        println!("end verify");
    }
    #[test]
    pub fn test_cmp() {
        let f = POA::new_from_string("2afa6bd56b12f68f95129addfb6a98e4d49aa423b73cec6ca160d2259c4b3d04".to_string());
        let mut b = Block::default();
        let bb = f.finalize_block(b, Hash([0u8;32])).unwrap();
        match f.verify(&bb) {
            Ok(()) => println!("verify seccess......"),
            Err(e) => println!("verify failed,err={:?}",e),
        }
    }
}
