use std::sync::{Arc, RwLock};

use slog::{debug};
use tokio::sync::mpsc;

use chain::blockchain::BlockChain;

use crate::sync::manager::SyncMessage;
use crate::sync::range_sync::BatchId;
use map_core::block::Block;

/// Id associated to a block processing request, either a batch or a single block.
#[derive(Clone, Debug, PartialEq)]
pub enum ProcessId {
    /// Processing Id of a range syncing batch.
    RangeBatchId(BatchId),
}

/// The result of a block processing request.
// TODO: When correct batch error handling occurs, we will include an error type.
#[derive(Debug)]
pub enum BatchProcessResult {
    /// The batch was completed successfully.
    Success,
    /// The batch processing failed.
    Failed,
}

/// Spawns a thread handling the block processing of a request: range syncing or parent lookup.
pub fn spawn_block_processor(
    chain: Arc<RwLock<BlockChain>>,
    process_id: ProcessId,
    downloaded_blocks: Vec<Block>,
    mut sync_send: mpsc::UnboundedSender<SyncMessage>,
    log: slog::Logger,
) {
    std::thread::spawn(move || {
        match process_id {
            // this a request from the range sync
            ProcessId::RangeBatchId(batch_id) => {
                debug!(log, "Processing batch"; "id" => *batch_id, "blocks" => downloaded_blocks.len());
                let result = match process_blocks(chain, downloaded_blocks.iter(), &log) {
                    Ok(_) => {
                        debug!(log, "Batch processed"; "id" => *batch_id );
                        BatchProcessResult::Success
                    }
                    Err(e) => {
                        debug!(log, "Batch processing failed"; "id" => *batch_id, "error" => e);
                        BatchProcessResult::Failed
                    }
                };

                let msg = SyncMessage::BatchProcessed {
                    batch_id: batch_id,
                    downloaded_blocks: downloaded_blocks,
                    result,
                };
                sync_send.try_send(msg).unwrap_or_else(|_| {
                    debug!(
                        log,
                        "Block processor could not inform range sync result. Likely shutting down."
                    );
                });
            }
        }
    });
}

/// Helper function to process blocks batches which only consumes the chain and blocks to process.
fn process_blocks<
    'a,
    I: Iterator<Item=&'a Block>,
>(
    chain: Arc<RwLock<BlockChain>>,
    downloaded_blocks: I,
    log: &slog::Logger,
) -> Result<(), String> {
    let current = chain.read().unwrap().current_block().height();
    for block in downloaded_blocks {
        println!("processor block block={}, local={}", block.height(), current);
        match chain.write().expect("block processor").import_block(block) {
            Ok(_) => {
                true
            }
            Err(e) => {
                println!("process_blocks error");
                break
            }
        };
    }
    Ok(())
}
