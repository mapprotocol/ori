use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use libp2p::PeerId;
use rand::prelude::*;
use slog::{crit, info, debug, warn};
use tokio::sync::mpsc;

use chain::blockchain::BlockChain;
use map_core::block::Block;
use map_core::types::Hash as Hash256;

use crate::p2p::RequestId;
use crate::sync::block_processor::{BatchProcessResult, ProcessId, spawn_block_processor};
use crate::sync::network_context::SyncNetworkContext;
use crate::sync::SyncMessage;

use super::batch::{Batch, BatchId, PendingBatches};

/// Blocks are downloaded in batches from peers. This constant specifies how many blocks per batch
/// is requested. There is a timeout for each batch request. If this value is too high, we will
/// downvote peers with poor bandwidth. This can be set arbitrarily high, in which case the
/// responder will fill the response up to the max request size, assuming they have the bandwidth
/// to do so.
pub const BLOCKS_PER_BATCH: u64 = 5;

/// The number of times to retry a batch before the chain is considered failed and removed.
const MAX_BATCH_RETRIES: u8 = 5;

/// The maximum number of batches to queue before requesting more.
const BATCH_BUFFER_SIZE: u8 = 5;

/// Invalid batches are attempted to be re-downloaded from other peers. If they cannot be processed
/// after `INVALID_BATCH_LOOKUP_ATTEMPTS` times, the chain is considered faulty and all peers will
/// be downvoted.
const INVALID_BATCH_LOOKUP_ATTEMPTS: u8 = 3;

/// A return type for functions that act on a `Chain` which informs the caller whether the chain
/// has been completed and should be removed or to be kept if further processing is
/// required.
pub enum ProcessingResult {
    KeepChain,
    RemoveChain,
}

/// A chain of blocks that need to be downloaded. Peers who claim to contain the target head
/// root are grouped into the peer pool and queried for batches when downloading the
/// chain.
pub struct SyncingChain {
    /// The original start slot when this chain was initialised.
    pub start_numer: u64,

    /// The target head slot.
    pub target_head_slot: u64,

    /// The target head root.
    pub target_head_root: Hash256,

    /// The batches that are currently awaiting a response from a peer. An RPC request for these
    /// have been sent.
    pub pending_batches: PendingBatches,

    /// The batches that have been downloaded and are awaiting processing and/or validation.
    completed_batches: Vec<Batch>,

    /// Batches that have been processed and awaiting validation before being removed.
    processed_batches: Vec<Batch>,

    /// The peers that agree on the `target_head_slot` and `target_head_root` as a canonical chain
    /// and thus available to download this chain from.
    pub peer_pool: HashSet<PeerId>,

    /// The next batch_id that needs to be downloaded.
    to_be_downloaded_id: BatchId,

    /// The next batch id that needs to be processed.
    to_be_processed_id: BatchId,

    /// The current state of the chain.
    pub state: ChainSyncingState,

    /// A random id given to a batch process request. This is None if there is no ongoing batch
    /// process.
    current_processing_batch: Option<Batch>,

    /// A send channel to the sync manager. This is given to the batch processor thread to report
    /// back once batch processing has completed.
    sync_send: mpsc::UnboundedSender<SyncMessage>,

    chain: Arc<RwLock<BlockChain>>,

    /// A reference to the sync logger.
    log: slog::Logger,
}

#[derive(PartialEq)]
pub enum ChainSyncingState {
    /// The chain is not being synced.
    Stopped,
    /// The chain is undergoing syncing.
    Syncing,
}

impl SyncingChain {
    pub fn new(
        start_numer: u64,
        target_head_slot: u64,
        target_head_root: Hash256,
        sync_send: mpsc::UnboundedSender<SyncMessage>,
        block_chain: Arc<RwLock<BlockChain>>,
        log: slog::Logger,
    ) -> Self {
        let peer_pool = HashSet::new();

        SyncingChain {
            start_numer,
            target_head_slot,
            target_head_root,
            pending_batches: PendingBatches::new(),
            completed_batches: Vec::new(),
            processed_batches: Vec::new(),
            peer_pool,
            to_be_downloaded_id: BatchId(1),
            to_be_processed_id: BatchId(1),
            state: ChainSyncingState::Stopped,
            current_processing_batch: None,
            sync_send,
            chain: block_chain,
            log,
        }
    }

    /// Returns the latest slot number that has been processed.
    fn current_processed_slot(&self) -> u64 {
        // println!("current_processed_slot {:?}",self.to_be_processed_id);
        self.start_numer
            .saturating_add(self.to_be_processed_id.saturating_sub(1u64) * BLOCKS_PER_BATCH)
    }

    /// A batch of blocks has been received. This function gets run on all chains and should
    /// return Some if the request id matches a pending request on this chain, or None if it does
    /// not.
    ///
    /// If the request corresponds to a pending batch, this function processes the completed
    /// batch.
    pub fn on_block_response(
        &mut self,
        network: &mut SyncNetworkContext,
        request_id: RequestId,
        beacon_block: &Option<Block>,
    ) -> Option<()> {
        if let Some(block) = beacon_block {
            // This is not a stream termination, simply add the block to the request
            self.pending_batches.add_block(request_id, block.clone())
        } else {
            // A stream termination has been sent. This batch has ended. Process a completed batch.
            let batch = self.pending_batches.remove(request_id)?;
            self.handle_completed_batch(network, batch);
            Some(())
        }
    }

    /// A completed batch has been received, process the batch.
    /// This will return `ProcessingResult::KeepChain` if the chain has not completed or
    /// failed indicating that further batches are required.
    fn handle_completed_batch(
        &mut self,
        network: &mut SyncNetworkContext,
        batch: Batch,
    ) {
        // An entire batch of blocks has been received. This functions checks to see if it can be processed,
        // remove any batches waiting to be verified and if this chain is syncing, request new
        // blocks for the peer.
        debug!(self.log, "Completed batch received"; "id"=> *batch.id, "blocks" => &batch.downloaded_blocks.len(), "awaiting_batches" => self.completed_batches.len());

        // verify the range of received blocks
        // Note that the order of blocks is verified in block processing
        if let Some(last_slot) = batch.downloaded_blocks.last().map(|b| b.height()) {
            // the batch is non-empty
            let first_slot = batch.downloaded_blocks[0].height();
            if batch.start_numer > first_slot || batch.end_number < last_slot {
                warn!(self.log, "BlocksByRange response returned out of range blocks";
                          "response_initial_slot" => first_slot,
                          "requested_initial_slot" => batch.start_numer);
                network.downvote_peer(batch.current_peer);
                self.to_be_processed_id = batch.id; // reset the id back to here, when incrementing, it will check against completed batches
                return;
            }
        }

        // Add this completed batch to the list of completed batches. This list will then need to
        // be checked if any batches can be processed and verified for errors or invalid responses
        // from peers. The logic is simpler to create this ordered batch list and to then process
        // the list.

        let insert_index = self
            .completed_batches
            .binary_search(&batch)
            .unwrap_or_else(|index| index);
        self.completed_batches.insert(insert_index, batch);

        // We have a list of completed batches. It is not sufficient to process batch successfully
        // to consider the batch correct. This is because batches could be erroneously empty, or
        // incomplete. Therefore, a batch is considered valid, only if the next sequential batch is
        // processed successfully. Therefore the `completed_batches` will store batches that have
        // already be processed but not verified and therefore have Id's less than
        // `self.to_be_processed_id`.

        // pre-emptively request more blocks from peers whilst we process current blocks,
        self.request_batches(network);

        // Try and process any completed batches. This will spawn a new task to process any blocks
        // that are ready to be processed.
        self.process_completed_batches();
    }

    /// Tries to process any batches if there are any available and we are not currently processing
    /// other batches.
    fn process_completed_batches(&mut self) {
        // Only process batches if this chain is Syncing
        if self.state != ChainSyncingState::Syncing {
            return;
        }

        // Only process one batch at a time
        if self.current_processing_batch.is_some() {
            return;
        }

        // Check if there is a batch ready to be processed
        if !self.completed_batches.is_empty()
            && self.completed_batches[0].id == self.to_be_processed_id
        {
            let batch = self.completed_batches.remove(0);

            // Note: We now send empty batches to the processor in order to trigger the block
            // processor result callback. This is done, because an empty batch could end a chain
            // and the logic for removing chains and checking completion is in the callback.

            // send the batch to the batch processor thread
            return self.process_batch(batch);
        }
    }

    /// Sends a batch to the batch processor.
    fn process_batch(&mut self, mut batch: Batch) {
        let downloaded_blocks = std::mem::replace(&mut batch.downloaded_blocks, Vec::new());
        let batch_id = ProcessId::RangeBatchId(batch.id.clone());
        self.current_processing_batch = Some(batch);
        spawn_block_processor(
            self.chain.clone(),
            batch_id,
            downloaded_blocks,
            self.sync_send.clone(),
            self.log.clone(),
        );
    }

    /// The block processor has completed processing a batch. This function handles the result
    /// of the batch processor.
    pub fn on_batch_process_result(
        &mut self,
        network: &mut SyncNetworkContext,
        batch_id: BatchId,
        downloaded_blocks: &mut Option<Vec<Block>>,
        result: &BatchProcessResult,
    ) -> Option<ProcessingResult> {
        if let Some(current_batch) = &self.current_processing_batch {
            if current_batch.id != batch_id {
                // batch process does not belong to this chain
                return None;
            }
            // Continue. This is our processing request
        } else {
            // not waiting on a processing result
            return None;
        }

        // claim the result by consuming the option
        let downloaded_blocks = downloaded_blocks.take().or_else(|| {
            // if taken by another chain, we are no longer waiting on a result.
            self.current_processing_batch = None;
            crit!(self.log, "Processed batch taken by another chain");
            None
        })?;

        // No longer waiting on a processing result
        let mut batch = self.current_processing_batch.take().unwrap();
        // These are the blocks of this batch
        batch.downloaded_blocks = downloaded_blocks;

        // double check batches are processed in order TODO: Remove for prod
        if batch.id != self.to_be_processed_id {
            crit!(self.log, "Batch processed out of order";
                "processed_batch_id" => *batch.id,
                "expected_id" => *self.to_be_processed_id);
        }

        let res = match result {
            BatchProcessResult::Success => {
                *self.to_be_processed_id += 1;

                // If the processed batch was not empty, we can validate previous invalidated
                // blocks
                if !batch.downloaded_blocks.is_empty() {
                    // Remove any batches awaiting validation.
                    //
                    // All blocks in processed_batches should be prior batches. As the current
                    // batch has been processed with blocks in it, all previous batches are valid.
                    //
                    // If a previous batch has been validated and it had been re-processed, downvote
                    // the original peer.
                    while !self.processed_batches.is_empty() {
                        let processed_batch = self.processed_batches.remove(0);
                        if *processed_batch.id >= *batch.id {
                            crit!(self.log, "A processed batch had a greater id than the current process id";
                                "processed_id" => *processed_batch.id,
                                "current_id" => *batch.id);
                        }

                        if let Some(prev_hash) = processed_batch.original_hash {
                            // The validated batch has been re-processed
                            if prev_hash != processed_batch.hash() {
                                // The re-downloaded version was different
                                if processed_batch.current_peer != processed_batch.original_peer {
                                    // A new peer sent the correct batch, the previous peer did not
                                    // downvote the original peer
                                    //
                                    // If the same peer corrected it's mistake, we allow it.... for
                                    // now.
                                    debug!(self.log, "Re-processed batch validated. Downvoting original peer";
                                        "batch_id" => *processed_batch.id,
                                        "original_peer" => format!("{}",processed_batch.original_peer),
                                        "new_peer" => format!("{}", processed_batch.current_peer));
                                    network.downvote_peer(processed_batch.original_peer);
                                }
                            }
                        }
                    }
                }

                // println!("on_batch_process_result {:?}",batch.end_number);
                // Add the current batch to processed batches to be verified in the future. We are
                // only uncertain about this batch, if it has not returned all blocks.
                if batch.downloaded_blocks.last().map(|block| block.height())
                    != Some(batch.end_number.saturating_sub(1u64))
                {
                    self.processed_batches.push(batch);
                }

                // check if the chain has completed syncing
                if self.current_processed_slot() >= self.target_head_slot {
                    // chain is completed
                    ProcessingResult::RemoveChain
                } else {
                    // chain is not completed

                    // attempt to request more batches
                    self.request_batches(network);

                    // attempt to process more batches
                    self.process_completed_batches();

                    // keep the chain
                    ProcessingResult::KeepChain
                }
            }
            BatchProcessResult::Failed => {
                warn!(self.log, "Batch processing failed"; "id" => *batch.id, "peer" => format!("{}", batch.current_peer));
                // The batch processing failed
                // This could be because this batch is invalid, or a previous invalidated batch
                // is invalid. We need to find out which and downvote the peer that has sent us
                // an invalid batch.

                // check that we have no exceeded the re-process retry counter
                if batch.reprocess_retries > INVALID_BATCH_LOOKUP_ATTEMPTS {
                    // if a batch has exceeded the invalid batch lookup attempts limit, it means
                    // that it is likely all peers in this chain are are sending invalid batches
                    // repeatedly and are either malicious or faulty. We drop the chain and
                    // downvote all peers.
                    warn!(self.log, "Batch failed to download. Dropping chain and downvoting peers"; "id"=> *batch.id);
                    for peer_id in self.peer_pool.drain() {
                        network.downvote_peer(peer_id);
                    }
                    ProcessingResult::RemoveChain
                } else {
                    ProcessingResult::KeepChain
                }
            }
        };

        Some(res)
    }

    /// Add a peer to the chain.
    ///
    /// If the chain is active, this starts requesting batches from this peer.
    pub fn add_peer(&mut self, network: &mut SyncNetworkContext, peer_id: PeerId) {
        self.peer_pool.insert(peer_id.clone());
        // do not request blocks if the chain is not syncing
        if let ChainSyncingState::Stopped = self.state {
            debug!(self.log, "Peer added to a non-syncing chain"; "peer_id" => format!("{}", peer_id));
            return;
        }

        // find the next batch and request it from any peers if we need to
        self.request_batches(network);
    }

    pub fn start_syncing(&mut self, network: &mut SyncNetworkContext, local_finalized_number: u64) {
        if local_finalized_number > self.current_processed_slot() {
            debug!(self.log, "Updating chain's progress";
                "prev_completed_slot" => self.current_processed_slot(),
                "new_completed_slot" => local_finalized_number);
            // Re-index batches
            *self.to_be_downloaded_id = 1;
            *self.to_be_processed_id = 1;

            // remove any completed or processed batches
            self.completed_batches.clear();
            self.processed_batches.clear();
        }
        warn!(self.log, "Start syncing chain";"local_slot" => local_finalized_number);

        self.state = ChainSyncingState::Syncing;

        // start processing batches if needed
        self.process_completed_batches();

        // begin requesting blocks from the peer pool, until all peers are exhausted.
        self.request_batches(network);
    }

    /// Sends a STATUS message to all peers in the peer pool.
    pub fn status_peers(&self, network: &mut SyncNetworkContext) {
        for peer_id in self.peer_pool.iter() {
            network.status_peer(self.chain.clone(), peer_id.clone());
        }
    }

    /// Attempts to request the next required batches from the peer pool if the chain is syncing. It will exhaust the peer
    /// pool and left over batches until the batch buffer is reached or all peers are exhausted.
    fn request_batches(&mut self, network: &mut SyncNetworkContext) {
        if let ChainSyncingState::Syncing = self.state {
            while self.send_range_request(network) {}
        }
    }

    /// Requests the next required batch from a peer. Returns true, if there was a peer available
    /// to send a request and there are batches to request, false otherwise.
    fn send_range_request(&mut self, network: &mut SyncNetworkContext) -> bool {
        // find the next pending batch and request it from the peer
        if let Some(peer_id) = self.get_next_peer() {
            if let Some(batch) = self.get_next_batch(peer_id) {
                info!(self.log, "Requesting batch";
                    "start_numer" => batch.start_numer,
                    "end_number" => batch.end_number,
                    "id" => *batch.id,
                    "peer" => format!("{}", batch.current_peer),
                    "head_root"=> format!("{}", batch.head_root));
                // send the batch
                self.send_batch(network, batch);
                return true;
            }
        }
        false
    }

    /// Returns a peer if there exists a peer which does not currently have a pending request.
    ///
    /// This is used to create the next request.
    fn get_next_peer(&self) -> Option<PeerId> {
        // TODO: Optimize this by combining with above two functions.
        // randomize the peers for load balancing
        let mut rng = rand::thread_rng();
        let mut peers = self.peer_pool.iter().collect::<Vec<_>>();
        peers.shuffle(&mut rng);
        for peer in peers {
            if self.pending_batches.peer_is_idle(peer) {
                return Some(peer.clone());
            }
        }
        debug!(self.log, "Select peer end";);
        None
    }

    /// Returns the next required batch from the chain if it exists. If there are no more batches
    /// required, `None` is returned.
    fn get_next_batch(&mut self, peer_id: PeerId) -> Option<Batch> {
        // only request batches up to the buffer size limit
        if self
            .completed_batches
            .len()
            .saturating_add(self.pending_batches.len())
            > BATCH_BUFFER_SIZE as usize
        {
            return None;
        }

        // println!("get_next_batch {:?}",self.to_be_downloaded_id);
        // don't request batches beyond the target head slot
        let batch_start_numer =
            self.start_numer + self.to_be_downloaded_id.saturating_sub(1) * BLOCKS_PER_BATCH + 1;
        if batch_start_numer > self.target_head_slot {
            return None;
        }

        // truncate the batch to the target head of the chain
        let batch_end_number = std::cmp::min(
            batch_start_numer + BLOCKS_PER_BATCH,
            self.target_head_slot.saturating_add(1u64),
        );

        let batch_id = self.to_be_downloaded_id;

        // Find the next batch id. The largest of the next sequential id, or the next uncompleted
        // id
        let max_completed_id = self
            .completed_batches
            .iter()
            .last()
            .map(|x| x.id.0)
            .unwrap_or_else(|| 0);
        // TODO: Check if this is necessary
        self.to_be_downloaded_id = BatchId(std::cmp::max(
            self.to_be_downloaded_id.0 + 1,
            max_completed_id + 1,
        ));

        Some(Batch::new(
            batch_id,
            batch_start_numer,
            batch_end_number,
            self.target_head_root,
            peer_id,
        ))
    }

    /// Requests the provided batch from the provided peer.
    fn send_batch(&mut self, network: &mut SyncNetworkContext, batch: Batch) {
        let request = batch.to_blocks_by_range_request();
        if let Ok(request_id) = network.blocks_by_range_request(batch.current_peer.clone(), request)
        {
            // add the batch to pending list
            self.pending_batches.insert(request_id, batch);
        }
    }
}
