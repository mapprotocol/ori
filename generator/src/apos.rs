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

use std::collections::HashMap;
// use std::cell::RefCell;
// use std::rc::Rc;
use std::sync::{Arc, RwLock};

use num_traits::{cast::ToPrimitive, identities::One};
use num_rational::BigRational;
use num_bigint::BigUint;
use hash;
// use map_consensus::ConsensusErrorKind;
use map_crypto::vrf;
use map_core::staking::Staking;
// use map_core::state::StateDB;
use map_core::runtime::Interpreter;
use chain::blockchain::BlockChain;
#[allow(unused_imports)]
use crate::types::{ValidatorStake, RngSeed};
use crate::epoch::EPOCH_LENGTH;
#[allow(unused_imports)]
use ed25519::{privkey::PrivKey, pubkey::Pubkey};
#[allow(unused_imports)]
use errors::{Error, ErrorKind};

/// Epoch staking and committee info
#[derive(Debug, Clone)]
pub struct EpochInfo {
    pub seed: u64,
    pub rng_seed: RngSeed,
    pub validators: Vec<ValidatorStake>,
}

/// Return VRF threshold of epoch validator set
pub fn calc_random_threshold(empty: u64, num: u64) -> u128 {
    let base  = 1000u64;
    let pskip = empty as f64 / base as f64;
    let p = BigRational::from_float(1f64 - pskip.powf(1 as f64 / num as f64)).unwrap();

    let numer = p.numer().to_biguint().unwrap();
    let denom = p.denom().to_biguint().unwrap();
    ((BigUint::one() << 128) * numer / denom).to_u128().unwrap_or(u128::max_value())
}

/// Returns true if the VRF value is little than the given threshold,
pub(super) fn cmp_random_threshold(random_value: &vrf::Value, threshold: u128) -> bool {
    let mut b: [u8; 16] = Default::default();
    b.copy_from_slice(&random_value.0[..16]);
    u128::from_be_bytes(b) < threshold
}

pub struct EpochPoS {
    epoch_infos: HashMap<u64, EpochInfo>,
    eid: u64, // current epoch id
    #[allow(dead_code)]
    dev_mode: bool,
    chain: Arc<RwLock<BlockChain>>,
    // node_key: Pubkey,
    // genesis_block: Block,
}

impl EpochPoS {
    pub fn new(chain: Arc<RwLock<BlockChain>>, dev_mode: bool) -> Self {
        EpochPoS {
            epoch_infos: HashMap::default(),
            eid: 0,
            dev_mode: dev_mode,
            chain: chain,
            // node_key: local_key,
        }
    }

    fn genesis_epoch(&self) -> Option<EpochInfo> {
        let chain = self.chain.read().unwrap();
        let cur_block = chain.current_block();
        let statedb = chain.state_at(cur_block.state_root());
        let state = Staking::new(Interpreter::new(statedb.clone()));

        let validators = state.validator_set();
        let mut holders: Vec<ValidatorStake> = Vec::new();

        if self.dev_mode {
            // let mut pk: [u8; 32] = [0; 32];
            // pk.copy_from_slice(&self.node_key.to_bytes());

            // holders.push(ValidatorStake {
            //     pubkey: pk,
            //     stake_amount: 0,
            //     sid: 0,
            //     validator: true,
            // });

            // Devmode single validator
            if validators.len() > 0 {
                let mut pk: [u8; 32] = [0; 32];
                pk.copy_from_slice(&validators[0].pubkey);
                holders.push(ValidatorStake {
                    pubkey: pk,
                    stake_amount: validators[0].effective_balance,
                    sid: 0,
                    validator: true,
                });
            }
        } else {
            for v in validators {
                let mut pk: [u8; 32] = [0; 32];
                pk.copy_from_slice(&v.pubkey);

                holders.push(ValidatorStake {
                    pubkey: pk,
                    stake_amount: v.effective_balance,
                    sid: 0,
                    validator: true,
                });
            }
        }


        Some(EpochInfo {
            seed: 0,
            validators: holders,
            rng_seed: [0; 32],
        })
    }

    pub fn next_epoch(&mut self) {
        self.eid = self.eid + 1
    }

	pub fn dev_node(&self) -> bool {
		self.dev_mode
	}

    pub fn get_epoch_info(&self, eid: u64) -> Option<EpochInfo> {
        // Retrive committee by epoch from caching
        // match self.epoch_infos.get(&eid) {
        //     Some(v) => Some(v.clone()),
        //     None => None,
        // }

        if let Some(v) = self.epoch_infos.get(&eid) {
            return Some(v.clone());
        }

        // Genesis epoch committee at start
        let mut epoch = self.genesis_epoch().unwrap().clone();
        if eid > 0 {
            epoch.rng_seed = self.compute_epoch_seed(eid).unwrap();
        }

        // sid not used in epoch validators
        // for (i, val) in epoch.validators.iter_mut().enumerate() {
        //     val.sid = i as u64 + eid * EPOCH_LENGTH;
        // }

        Some(epoch)
    }

    pub fn get_proposer_index(&self, stake: &EpochInfo, slot_index: u64, seed: RngSeed) -> u64 {
        let count = stake.validators.len() as u64;
        let mut slot = Vec::new();
        slot.extend_from_slice(&seed);
        slot.extend_from_slice(&slot_index.to_be_bytes());

        // Compute proposer from seed
        let slot_seed = hash::blake2b_256(slot);
        let mut bytes: [u8; 8] = [0; 8];
        bytes.copy_from_slice(&slot_seed[24..]);
        warn!("slot seed index={} seed={:2x?}", slot_index, bytes);
        u64::from_be_bytes(bytes) % count
    }

    pub fn get_slot_proposer(&self, index: u64, eid: u64) -> Option<ValidatorStake> {
        match self.get_epoch_info(eid) {
            Some(epoch) => {
                let i = index % EPOCH_LENGTH;
                let proposer = self.get_proposer_index(&epoch, i, epoch.rng_seed);
                if proposer >= epoch.validators.len() as u64 {
                    error!("Proposer index out of validator boudry, slot={}", index);
                    return None
                }
                info!("calculate proposer index slot={}, index={}", index, proposer);
                Some(epoch.validators[proposer as usize].clone())
            }
            None => None,
        }
    }

    pub fn get_epoch_staking(&self, eid: u64) -> Option<Vec<ValidatorStake>> {
        match self.get_epoch_info(eid) {
            Some(items) => {
                let mut vv: Vec<ValidatorStake> = Vec::new();
                for v in items.validators.iter() {
                    if v.is_validator() {
                        vv.push(v.clone());
                    }
                }
                if vv.len() > 0 {
                    Some(vv)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    /// Compute if node is propser of the slot by apply vrf
    pub fn calc_epoch_threshold(&self, _eid: u64, epoch: &EpochInfo) -> u128 {
        calc_random_threshold(200, epoch.validators.len() as u64)
    }

    /// Compute if node is propser of the slot by apply vrf
    pub fn make_slot_proposer(&self, sid: u64, private_key: PrivKey) -> Option<(vrf::Value, vrf::Proof)> {
        let eid: u64 = sid / EPOCH_LENGTH;
        let epoch_data = match self.get_epoch_info(eid) {
            Some(epoch) => epoch,
            None => return None,
        };

        if epoch_data.validators.iter().find(
            |&x| Pubkey::from_bytes(&x.pubkey).equal(&private_key.to_pubkey().unwrap())).is_none() {
            return None;
        }

        let mut seed = Vec::new();
        seed.extend_from_slice(&epoch_data.rng_seed);
        seed.extend_from_slice(&sid.to_be_bytes());

        let secret_key = vrf::convert_secret_key(&private_key.to_bytes());
        let (vrf_value, vrf_proof) = secret_key.compute_vrf_with_proof(&hash::blake2b_256(&seed));
        let threshold = self.calc_epoch_threshold(eid, &epoch_data);
        info!("Calc vrf value={:?}, threshold={:x}", vrf_value, threshold);
        if cmp_random_threshold(&vrf_value, threshold) {
            return Some((vrf_value, vrf_proof));
        }
        None
    }

    pub fn get_seed_by_epochid(&self, eid: u64) -> u64 {
        if let Some(items) = self.get_epoch_info(eid) {
            items.seed
        } else {
            0
        }
    }

    #[allow(unused_variables)]
    pub fn compute_epoch_seed(&self, epoch: u64) -> Option<RngSeed> {
        let num = epoch * EPOCH_LENGTH;
        let num = 0u64;
        let block = self.chain.read().unwrap().get_block_by_number(num);
        let mut seed = match block {
            Some(b)  => b.hash().0,
            None    => return None,
        };
        if epoch == 0 {
            seed = [0; 32];
        }
        Some(hash::blake2b_256(seed))
    }

    // fn from_genesis(&mut self,genesis: &Block,state: &Balance) {
    //     let proofs = genesis.get_proofs();
    //     let mut vals: Vec<ValidatorStake> = Vec::new();
    //     let seed: u64 = 0;
    //     for (i,proof) in proofs.iter().enumerate() {
    //         if self.lock_info.equal_pk_by_slice(&proof.0[..]) {
    //             self.lindex = i as i32;
    //         }
    //         vals.push(ValidatorStake{
    //             pubkey:         proof.0,
    //             stake_amount:    state.balance(proof.to_address()),
    //             sid:            -1 as i32,
    //             // seedVerifyPk:   P256PK::default(),
    //             // seedPk:         None,
    //             validator:      true,
    //         });
    //     }
    //     self.epoch_infos.insert(0,EpochInfo{
    //         seed:       seed,
    //         validators: vals,
    //     });
    // }

    // pub fn is_validator(&self) -> bool {
    //     match self.get_epoch_staking(self.eid) {
    //         Some(vv) => match vv.get(self.lindex as usize) {
    //             Some(v) => {
    //                 return self.lock_info.equal_pk(&v.get_pubkey());
    //             }
    //             None => false,
    //         },
    //         None => false,
    //     }
    // }
}
