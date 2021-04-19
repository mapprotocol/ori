use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use libp2p::PeerId;
use slog::{debug};
use tokio::sync::mpsc;

use chain::blockchain::BlockChain;
use map_core::types::Hash as Hash256;

use crate::handler_processor::PeerSyncInfo;
use crate::p2p::RequestId;
use crate::sync::block_processor::BatchProcessResult;
use crate::sync::manager::SyncMessage;
use crate::sync::network_context::SyncNetworkContext;

use super::BatchId;
use super::chain::{SyncingChain, ChainSyncingState};
use super::chain::ProcessingResult;
use map_core::block::Block;

/// The primary object dealing with long range/batch syncing. This contains all the active and
/// non-active chains that need to be processed before the syncing is considered complete. This
/// holds the current state of the long range sync.
pub struct RangeSync {
    chain: Arc<RwLock<BlockChain>>,
    /// A collection of chains that need to be downloaded. This stores any head or finalized chains
    /// that need to be downloaded.
    chains: SyncingChain,

    /// Peers that join whilst a finalized chain is being download, sit in this set. Once the
    /// finalized chain(s) complete, these peer's get STATUS'ed to update their head slot before
    /// the head chains are formed and downloaded.
    awaiting_head_peers: HashSet<PeerId>,
    /// The syncing logger.
    log: slog::Logger,
}

impl RangeSync {
    pub fn new(
        block_chain: Arc<RwLock<BlockChain>>,
        sync_send: mpsc::UnboundedSender<SyncMessage>,
        log: slog::Logger,
    ) -> Self {
        let current = block_chain.read().unwrap().current_block().height();
        let h = Hash256([0u8; 32]);
        RangeSync {
            chain: block_chain.clone(),
            chains: SyncingChain::new(current, 0, h, sync_send.clone(), block_chain, log.clone()),
            awaiting_head_peers: HashSet::new(),
            log,
        }
    }

    /// A useful peer has been added. The SyncManager has identified this peer as needing either
    /// a finalized or head chain sync. This processes the peer and starts/resumes any chain that
    /// may need to be synced as a result. A new peer, may increase the peer pool of a finalized
    /// chain, this may result in a different finalized chain from syncing as finalized chains are
    /// prioritised by peer-pool size.
    pub fn add_peer(
        &mut self,
        network: &mut SyncNetworkContext,
        peer_id: PeerId,
        remote: PeerSyncInfo,
    ) {
        // evaluate which chain to sync from

        // determine if we need to run a sync to the nearest finalized state or simply sync to
        // its current head

        // remove peer from any chains
        self.remove_peer(network, &peer_id);

        // The new peer has the same finalized (earlier filters should prevent a peer with an
        // earlier finalized chain from reaching here).
        debug!(self.log, "New peer added for sync"; "head_root" => format!("{}",remote.head_root), "head_slot" => remote.finalized_number, "peer_id" => format!("{:?}", peer_id));

        // add the peer to the head's pool
        self.chains.target_head_slot = remote.finalized_number;
        self.chains.target_head_root = remote.finalized_root;
        self.chains.add_peer(network, peer_id);
        let local = self.chain.read().unwrap().current_block().height();
        self.chains.start_syncing(network, local);
    }

    /// A `BlocksByRange` response has been received from the network.
    ///
    /// This function finds the chain that made this request. Once found, processes the result.
    /// This request could complete a chain or simply add to its progress.
    pub fn blocks_by_range_response(
        &mut self,
        network: &mut SyncNetworkContext,
        peer_id: PeerId,
        request_id: RequestId,
        beacon_block: Option<Block>,
    ) {
        // Find the request. Most likely the first finalized chain (the syncing chain). If there
        // are no finalized chains, then it will be a head chain. At most, there should only be
        // `connected_peers` number of head chains, which should be relatively small and this
        // lookup should not be very expensive. However, we could add an extra index that maps the
        // request id to index of the vector to avoid O(N) searches and O(N) hash lookups.

        let id_not_found = self
            .chains.on_block_response(network, request_id, &beacon_block)
            .is_none();
        if id_not_found {
            // The request didn't exist in any `SyncingChain`. Could have been an old request or
            // the chain was purged due to being out of date whilst a request was pending. Log
            // and ignore.
            debug!(self.log, "Range response without matching request"; "peer" => format!("{:?}", peer_id), "request_id" => request_id);
        }
    }

    pub fn handle_block_process_result(
        &mut self,
        network: &mut SyncNetworkContext,
        batch_id: BatchId,
        downloaded_blocks: Vec<Block>,
        result: BatchProcessResult,
    ) {
        // build an option for passing the downloaded_blocks to each chain
        let mut downloaded_blocks = Some(downloaded_blocks);

        match self.chains.on_batch_process_result(network, batch_id, &mut downloaded_blocks, &result) {
            Some(ProcessingResult::RemoveChain) => {
                // the chain is complete, re-status it's peers
                self.chains.status_peers(network);
                self.chains.state = ChainSyncingState::Stopped;
                debug!(self.log, "remove chain"; "id" => *batch_id);
            }
            Some(ProcessingResult::KeepChain) => {}
            None => {
                match self.chains.on_batch_process_result(
                    network,
                    batch_id,
                    &mut downloaded_blocks,
                    &result,
                ) {
                    Some(ProcessingResult::RemoveChain) => {
                        debug!(self.log, "Head chain completed"; "start_numer" => self.chains.start_numer, "end_slot" => self.chains.target_head_slot);
                        // the chain is complete, re-status it's peers and remove it
                    }
                    Some(ProcessingResult::KeepChain) => {}
                    None => {
                        // This can happen if a chain gets purged due to being out of date whilst a
                        // batch process is in progress.
                        debug!(self.log, "No chains match the block processing id"; "id" => *batch_id);
                    }
                }
            }
        }
    }

    pub fn is_syncing(&self) -> bool {
        match self.chains.state {
            ChainSyncingState::Syncing => true,
            ChainSyncingState::Stopped => false,
        }
    }

    #[allow(dead_code)]
    pub fn update_finalized(&mut self, network: &mut SyncNetworkContext, block: Block) {
        if self.chains.state == ChainSyncingState::Syncing {
            return;
        }
        let local = self.chain.read().unwrap().current_block().height();

        self.chains.target_head_slot = block.height();
        self.chains.target_head_root = block.hash();
        self.chains.start_syncing(network, local);
    }

    /// A peer has disconnected. This removes the peer from any ongoing chains and mappings. A
    /// disconnected peer could remove a chain
    pub fn peer_disconnect(&mut self, network: &mut SyncNetworkContext, peer_id: &PeerId) {
        // if the peer is in the awaiting head mapping, remove it
        self.awaiting_head_peers.remove(&peer_id);

        // remove the peer from any peer pool
        self.remove_peer(network, peer_id);
    }

    /// When a peer gets removed, both the head and finalized chains need to be searched to check which pool the peer is in. The chain may also have a batch or batches awaiting
    /// for this peer. If so we mark the batch as failed. The batch may then hit it's maximum
    /// retries. In this case, we need to remove the chain and re-status all the peers.
    fn remove_peer(&mut self, network: &mut SyncNetworkContext, peer_id: &PeerId) {}
}
