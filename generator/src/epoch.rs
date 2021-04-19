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

use std::sync::{Arc, RwLock};
// use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, Instant};

#[allow(unused_imports)]
use crate::{apos::{self, EpochPoS}, types};
use chain::blockchain::BlockChain;
use pool::tx_pool::TxPoolManager;
use tokio::prelude::*;
use tokio::timer::{self, Delay};
use tokio::sync::oneshot;
use tokio::runtime;
use tokio::sync::mpsc;
#[allow(unused_imports)]
use ed25519::{privkey::PrivKey, pubkey::Pubkey};
#[allow(unused_imports)]
use errors::{Error, ErrorKind};
use executor::Executor;
use map_crypto::vrf;
#[allow(unused_imports)]
use map_consensus::ConsensusErrorKind;
use map_network::manager::{self, NetworkMessage};
#[allow(unused_imports)]
use map_core::block::{Block, VRFProof, Header, BlockProof, VerificationItem};
use map_core::balance::Balance;
use map_core::transaction::Transaction;
use map_core::runtime::Interpreter;
use map_core::types::{Hash, Address};
use map_core::genesis::GENESIS_TIME;
// use super::fts;

/// Slots per epoch constant
pub const EPOCH_LENGTH: u64 = 64;
pub const SLOT_DURATION: u64 = 6;

// type TypeNewBlockEvent = Receiver<Block>;
// type TypeNewTimerIntervalEvent = Receiver<Instant>;
// type TypeTickEvent = Receiver<Instant>;
// pub type TypeStopEpoch = Sender<()>;

// Chain bulder to make proposer block
#[derive(Clone)]
pub struct Builder {
    chain: Arc<RwLock<BlockChain>>,
    tx_pool : Arc<RwLock<TxPoolManager>>,
}

impl Builder {
    pub fn new(chain: Arc<RwLock<BlockChain>>, tx_pool: Arc<RwLock<TxPoolManager>>) -> Self {
        Builder {
            chain: chain,
            tx_pool: tx_pool,
        }
    }
    // Proposal new block from certain slot
    pub fn produce_block(&self, slot: u64, parent: Hash, vrf_output: vrf::Value, vrf_proof: vrf::Proof) -> Block {
        let pre = self.chain.read().unwrap().get_block(parent).unwrap();

        let txs = self.prepare_transactions();
        let tx_len = txs.len();
        let mut block = Block::new(Header::default(), txs, Vec::new(), Vec::new());
        let state_root = self.apply_block(pre.state_root(), &block);

        block.header.parent_hash = parent;
        block.header.height = pre.height() + 1;
        block.header.slot = slot;
        block.header.vrf_output = vrf_output.0;
        block.header.vrf_proof = VRFProof::new(vrf_proof.0);
        block.header.state_root = state_root;
        block.header.time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        info!("Seal state root pre={}, post={} tx={}", pre.state_root(), state_root, tx_len);
        block
    }

    pub fn apply_block(&self, root: Hash, b: &Block) -> Hash {
        let statedb = self.chain.read().unwrap().state_at(root);
        let h = Executor::exc_txs_in_block(&b, &mut Balance::new(Interpreter::new(statedb)), &Address::default()).unwrap();
        h
    }

    pub fn prepare_transactions(&self) -> Vec<Transaction> {
        self.tx_pool.read().unwrap().get_pending()
    }

    pub fn get_current_height(&self) -> u64 {
        self.chain.read().unwrap().current_block().height()
    }

    pub fn get_head_block(&self) -> Block {
        self.chain.read().unwrap().current_block()
    }

    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        self.chain.read().unwrap().get_block_by_number(height)
    }

    pub fn get_sid_from_current_block(&self) -> u64 {
        self.chain.read().unwrap().current_block().height() + 1
    }

    #[allow(unused_variables)]
    pub fn get_best_chain(&self, height: u64) -> Option<Block> {
        Some(Block::default())
    }

    pub fn get_blockchain(&self) ->  Arc<RwLock<BlockChain>> {
        // self.chain.write().unwrap().import_block(block);
        self.chain.clone()
    }
}

/// Epoch defines a number of slot duration
pub struct EpochId(u64);

impl EpochId {
    pub fn epoch_from_height(h: u64) -> u64 {
        let eid: u64 = h / EPOCH_LENGTH;
        eid
    }

    pub fn epoch_from_id(sid: u64) -> u64 {
        // We may skip block from slot
        let eid: u64 = sid / EPOCH_LENGTH;
        eid
    }

    pub fn get_height_from_eid(eid: u64) -> (u64, u64) {
        if eid as i64 <= 0 {
            return (0, 0);
        }
        let low: u64 = (eid - 1) * EPOCH_LENGTH as u64;
        let hi: u64 = eid * EPOCH_LENGTH as u64 - 1;
        (low, hi)
    }
}

#[derive(Debug, Clone)]
pub struct Slot {
    timeout: u32, // millsecond
    id: i32,
    vindex: u32,
}

impl Slot {
    pub fn new(sid: i32, index: u32) -> Self {
        Slot {
            timeout: 5000,
            id: sid,
            vindex: index,
        }
    }
}

/// Returns duration since unix epoch.
pub fn duration_now() -> Duration {
    let now = SystemTime::now();
    now.duration_since(SystemTime::UNIX_EPOCH).unwrap()
}

/// A clock tick that wake every time there is a new time slot.
pub struct SlotTick {
    slot_duration: u64,
    delay: Delay,
    genesis_duration: Duration,
}

impl SlotTick {
    pub fn new(duration: u64, genesis: Duration) -> Self {
        let mut timeout = Instant::now();
        let now = duration_now();
        if now < genesis {
            timeout = timeout + (genesis - now);
        }

        SlotTick {
            slot_duration: duration,
            delay: Delay::new(timeout),
            genesis_duration: genesis,
        }
    }
}

impl Stream for SlotTick {
    type Item = u64;
    type Error = timer::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // let delay = self.delay.as_mut().unwrap();
        // self.delay = match self.delay.take() {
        //     Some(d) => Some(d),
        //     None => {
        //         let timeout = Instant::now() + Duration::from_secs(self.slot_duration);
        //         info!("new slot deadline: timeout={:?}", timeout);
        //         Some(Delay::new(timeout))
        //     }
        // };

        let _ = try_ready!(self.delay.poll());

        let timeout = Delay::deadline(&self.delay);
        let deadline = timeout + Duration::from_secs(self.slot_duration);
        self.delay = Delay::new(deadline);

        let now = duration_now();
        let slot = (now.as_millis() - self.genesis_duration.as_millis()) / Duration::from_secs(self.slot_duration).as_millis();

        Ok(Async::Ready(Some(slot as u64)))
    }
}

// pub struct EpochProcess {
//     exit_event: Receiver<i32>,
//     myid: Pubkey,
//     cur_eid: u64,
//     #[allow(dead_code)]
//     cur_seed: u64,
//     #[allow(dead_code)]
//     slots: Vec<Slot>,
//     block_chain: Builder,
//     tx_pool : Arc<RwLock<TxPoolManager>>,
//     network: NetworkExecutor,
// }

// impl EpochProcess {
    // pub fn new(
    //     mid: Pubkey,
    //     eid: u64,
    //     chain: Arc<RwLock<BlockChain>>,
    //     p2p: NetworkExecutor,
    //     tx_pool: Arc<RwLock<TxPoolManager>>,
    //     exit: Receiver<i32>
    // ) -> Self {
    //     EpochProcess {
    //         myid: mid,
    //         cur_eid: eid,
    //         cur_seed: 0,
    //         slots: Vec::new(),
    //         block_chain: Builder::new(chain.clone(), tx_pool.clone()),
    //         tx_pool: tx_pool,
    //         exit_event: exit,
    //         network: p2p,
    //     }
    // }

    // pub fn start(
    //     self,
    //     state: Arc<RwLock<EpochPoS>>,
    // ) -> JoinHandle<()> {
    //     let (_, new_block) = unbounded();

    //     // Get start slot on node lanuch
    //     let sid = self.block_chain.get_sid_from_current_block();
    //     self.start_slot_tick_in_epoch(sid, new_block, state.clone())
    // }

  //   pub fn is_proposer(&self, sid: u64, state: Arc<RwLock<EpochPoS>>) -> bool {
		// if state.read().unwrap().dev_node() {
		// 	return true
		// }

  //       if let Some(item) = state
  //           .read()
  //           .unwrap()
  //           .get_slot_proposer(sid, self.cur_eid)
  //       {
  //           let pk: Pubkey = Pubkey::from_bytes(&item.pubkey);
  //           let is_proposer = self.myid.equal(&item.into());
  //           info!("is_proposer:{}, my={} proposer={}", is_proposer, self.myid, pk);
  //           is_proposer
  //       } else {
  //           false
  //       }
  //   }

    // pub fn get_my_pk(&self) -> Option<Pubkey> {
    //     Some(self.myid.clone())
    // }

    // #[allow(unused_variables)]
    // pub fn next_epoch(&mut self, sid: u64, state: Arc<RwLock<EpochPoS>>) -> Result<bool, Error> {
    //     let next_eid = EpochId::epoch_from_id(sid);
    //     if next_eid == self.cur_eid + 1 {
    //         self.cur_eid = next_eid;
    //         Ok(true)
    //     } else {
    //         Ok(false)
    //     }
    // }
    // pub fn assign_validator(&mut self, state: Arc<RwLock<EpochPoS>>) -> Result<(), Error> {
        // if let Some(vals) = state.read()
        // .expect("acquiring apos read lock")
        // .get_epoch_staking(self.cur_eid){
        //     self.slots.clear();
        //     let mut validators = vals;
        //     let seed = self.cur_seed;
        //     // fts::assign_valditator_to_slot(&mut validators, seed)?;
        //     // for (i,v) in validators.iter().enumerate() {
        //     //     self.slots.push(
        //     //         slot::new(v.get_sid(),i as u32)
        //     //     );
        //     // }
        //     Ok(())
        // }
        // Err(ConsensusErrorKind::NotMatchEpochID.into())
        // let pos = state.read().unwrap();
        // let _committee = pos.get_epoch_info(self.cur_eid).unwrap();

        // Ok(())
    // }

    /// Compute if node is propser of the slot by apply vrf
    // #[allow(unused_variables)]
    // pub fn make_slot_proposer(&self, sid: u64, state: Arc<RwLock<EpochPoS>>) -> Option<()> {

    //     let eid: u64 = sid / EPOCH_LENGTH;

    //     let epoch_data = match state.read().unwrap().get_epoch_info(eid) {
    //         Some(epoch) => {
    //             let i = sid % EPOCH_LENGTH;
    //             let proposer = state.read().unwrap().get_proposer_index(&epoch, i, epoch.rng_seed);
    //             if proposer >= epoch.validators.len() as u64 {
    //                 error!("Proposer index out of validator boudry, slot={}", sid);
    //                 return None
    //             }
    //             info!("calculate proposer index slot={}, index={}", sid, proposer);
    //             Some(epoch.validators[proposer as usize].clone())
    //         }
    //         None => return None,
    //     };

    //     let threshold = self.calc_epoch_threshold(1, state.clone());
    //     None
    // }

    /// Compute if node is propser of the slot by apply vrf
    // #[allow(unused_variables)]
    // pub fn calc_epoch_threshold(&self, c: u64, state: Arc<RwLock<EpochPoS>>) -> u64 {
    //     0
    // }

    // pub fn new_slot_handle(&mut self, sid: u64, state: Arc<RwLock<EpochPoS>>) {
    //     info!("new slot id={}", sid);
    //     if self.is_proposer(sid, state) {
    //         let current = self.block_chain.get_head_block();
    //         let b = self
    //             .block_chain
    //             .produce_block(current.height(), current.hash());

    //         info!("make new block hash={} num={}", b.hash(), b.height());
    //         {
    //             let block_chain = self.block_chain.get_blockchain();
    //             let mut chain = block_chain.write().unwrap();
    //             if let Err(e) = chain.insert_block(b.clone()) {
    //                 error!("insert_block Error: {:?}", e);
    //                 return;
    //             }
    //         }
    //         self.tx_pool.write().unwrap().reset_pool(&b);
    //         // boradcast and import the block
    //         self.network.publish_block(b);
    //     }
    // }

    // pub fn slot_handle(&mut self, sid: u64, state: Arc<RwLock<EpochPoS>>) {
    //     if self.is_proposer(sid, state) {
    //         let current = self.block_chain.get_head_block();
    //         let b = self
    //             .block_chain
    //             .produce_block(current.height(), current.hash());
    //         info!("make new block hash={}", b.hash());
    //         // boradcast and import the block
    //     }
    // }

    // #[allow(unused_variables)]
    // pub fn start_slot_tick_in_epoch(
    //     mut self,
    //     sid: u64,
    //     new_block: TypeNewBlockEvent,
    //     state: Arc<RwLock<EpochPoS>>,
    // ) -> JoinHandle<()> {
    //     let (stop_epoch_send, stop_epoch_receiver) = bounded::<()>(1);
    //     let mut walk_pos: u64 = sid;
    //     let thread_builder = thread::Builder::new();
    //     // let start_slot = sid;
    //     let new_interval = tick(Duration::new(SLOT_DURATION, 0));
    //     let slot_interval = Interval::new(Instant::now(), Duration::from_secs(5));

    //     // let start = Instant::now();
    //     // let now = Instant::now();
    //     // let elapse = now.duration_since(start);

    //     let join_handle = thread_builder
    //         .spawn(move || loop {

    //             select! {
    //                 // recv(stop_epoch_receiver) -> _ => {
    //                 //     // end of slot
    //                 //     // break;
    //                 //     warn!("stop receiver");
    //                 // },
    //                 recv(new_interval) -> _ => {
    //                     // TODO: replace slot with wall clock
    //                     walk_pos = self.block_chain.get_current_height() + 1;
    //                     self.handle_new_time_interval_event(walk_pos, state.clone());
    //                     // walk_pos = walk_pos + 1;
    //                 },
    //                 recv(self.exit_event) -> _ => {
    //                     warn!("slot tick task exit");
    //                     if !self.network.exit_signal.is_closed() {
    //                         self.network.exit_signal.send(1).expect("network exit error");
    //                     }
    //                     break;
    //                 },
    //                 // recv(new_block) -> msg => {
    //                 //     self.handle_new_block_event(msg, &walk_pos, state.clone());
    //                 //     walk_pos = walk_pos + 1;
    //                 // },
    //             }
    //             // new epoch
    //             // match self.next_epoch(walk_pos + 1, state.clone()) {
    //             //     Err(e) => {
    //             //         println!(
    //             //             "start_slot_tick_in_epoch is quit,cause next epoch is err:{:?}",
    //             //             e
    //             //         );
    //             //         return;
    //             //     },
    //             //     _ => (),
    //             // }

    //             // No skipping empty slot right now
    //             // walk_pos = self.block_chain.get_sid_from_current_block();
    //         })
    //         .expect("Start slot_walk failed");
    //     join_handle
    // }

    // #[allow(unused_variables)]
    // pub fn process_slot_tick(
    //     &mut self,
    //     state: Arc<RwLock<EpochPoS>>,
    // ) {
    //     let walk_pos = self.block_chain.get_current_height() + 1;
    //     self.handle_new_time_interval_event(walk_pos, state.clone());
    // }

    // pub fn start_tick_service(
    //     &mut self,
    //     state: Arc<RwLock<EpochPoS>>,
    // ) {
    //     let slot_interval = timer::Interval::new(Instant::now(), Duration::from_secs(6));

    //     let tick = slot_interval
    //         .map_err(|e| panic!("interval errored; err={:?}", e))
    //         .for_each(move |_| {
    //             // self.process_slot_tick(state.clone());
    //             Ok(())
    //         });

    //     // thread::spawn(|| {
    //     //     // spawn on the executor
    //     //     tokio::run(tick);
    //     // });

    //     tokio::run(tick);
    // }

    // pub fn slot_tick_task(
    //     &mut self,
    //     state: Arc<RwLock<EpochPoS>>,
    // ) -> impl Future<Item = (), Error = ()> + '_ {
    //     let slot_interval = timer::Interval::new(Instant::now(), Duration::from_secs(6));

    //     let tick = slot_interval
    //         .map_err(|e| panic!("interval tick errored; err={:?}", e))
    //         .for_each(move |_| {
    //             self.process_slot_tick(state.clone());
    //             Ok(())
    //         });

    //     tick
    // }

    // fn handle_new_time_interval_event(&mut self, sid: u64, state: Arc<RwLock<EpochPoS>>) {
    //     self.new_slot_handle(sid, state);
    // }
// }

#[derive(Clone)]
pub struct EpochProposal {
    executor: runtime::TaskExecutor,
    myid: PrivKey,
    pubkey: Pubkey,
    chain: Arc<RwLock<BlockChain>>,
    block_chain: Builder,
    stake: Arc<RwLock<EpochPoS>>,
    tx_pool: Arc<RwLock<TxPoolManager>>,
    network_send: mpsc::UnboundedSender<NetworkMessage>,
}

impl EpochProposal {
    pub fn new(
        mid: PrivKey,
        chain: Arc<RwLock<BlockChain>>,
        stake: Arc<RwLock<EpochPoS>>,
        tx_pool: Arc<RwLock<TxPoolManager>>,
        network_send: mpsc::UnboundedSender<NetworkMessage>,
        executor: runtime::TaskExecutor
    ) -> Self {
        EpochProposal {
            myid: mid,
            pubkey: mid.to_pubkey().unwrap(),
            chain: chain.clone(),
            block_chain: Builder::new(chain.clone(), tx_pool.clone()),
            stake: stake,
            tx_pool: tx_pool.clone(),
            network_send: network_send,
            executor: executor,
        }
    }

    /// Run block proposal service
    pub fn start(&self) -> oneshot::Sender<()> {
        let (exit_signal, exit_rx) = oneshot::channel();
        let worker = self.make_proposal()
            .select(exit_rx.then(|_| {
                info!("Stop slot clock");
                Ok(())
            }))
            .then(|_| {
                info!("Stop block proposal");
                Ok(())
            });
        self.executor.spawn(worker);

        exit_signal
    }

    /// Make block proposal from random validator
    fn make_proposal(&self) -> impl Future<Item = (), Error = ()> {
        // future::ok::<Duration, ()>(now)
        //     .and_then(move |_| {
        //         Ok(())
        //     })
        let genesis_duration = Duration::from_secs(GENESIS_TIME);
        let mut proposal = self.clone();
        let tick = SlotTick::new(SLOT_DURATION, genesis_duration)
            .map_err(move |_| {
                error!("tick error");
            })
            .for_each(move |slot| {
                info!("slot tick instant {:?}", slot);
                let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).ok().unwrap();
                info!("time current {:?}", now);
                // let walk_pos = proposal.block_chain.get_current_height() + 1;
                proposal.on_slot(slot);
                Ok(())
            });

        tick
    }

    fn on_slot(&mut self, sid: u64) {
        info!("new slot id={}", sid);
        // match self.stake.read().unwrap().make_slot_proposer(sid, self.myid) {
        //     Some((value, proof)) => {
        //         info!("VRF value hash={:?}", value);
        //     },
        //     None => {
        //         info!("Not proposer key={}", self.myid.to_pubkey().unwrap());
        //     },
        // }
        // if self.is_proposer(sid, self.stake.clone()) {

        if let Some((value, proof)) =  self.stake.read().unwrap().make_slot_proposer(sid, self.myid) {
            info!("Make proposer vrf value={:?} pk={}", value, self.pubkey);
            let current = self.block_chain.get_head_block();
            let b = self
                .block_chain
                .produce_block(sid, current.hash(), value, proof);

            info!("make new block hash={} num={}", b.hash(), b.height());
            {
                let block_chain = self.block_chain.get_blockchain();
                let mut chain = block_chain.write().unwrap();
                if let Err(e) = chain.insert_block(b.clone()) {
                    error!("insert_block Error: {:?}", e);
                    return;
                }
            }
            self.tx_pool.write().unwrap().reset_pool(&b);
            // boradcast and import the block
            manager::publish_block(&mut self.network_send, b);
        }
    }

    #[allow(dead_code)]
    fn is_proposer(&self, sid: u64, state: Arc<RwLock<EpochPoS>>) -> bool {
        if state.read().unwrap().dev_node() {
            return true
        }

        if let Some(item) = state
            .read()
            .unwrap()
            .get_slot_proposer(sid, sid / EPOCH_LENGTH)
        {
            let pk: Pubkey = Pubkey::from_bytes(&item.pubkey);
            let is_proposer = self.myid.to_pubkey().unwrap().equal(&item.into());
            info!("is_proposer:{}, my={} proposer={}", is_proposer, self.myid.to_pubkey().unwrap(), pk);
            is_proposer
        } else {
            false
        }
    }
}



#[cfg(test)]
pub mod tests {
    use tokio::prelude::*;
    use tokio;
    use super::{SlotTick, duration_now};

    #[test]
    fn slot_tick() {
        let tick = SlotTick::new(1, duration_now())
            .take(2)
            .map_err(move |_| {
                println!("tick error");
            })
            .for_each(|instant| {
                println!("tick instant {:?}", instant);
                Ok(())
            });
        tokio::run(tick);
    }
}
