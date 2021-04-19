/// Light pool status.
/// This status is cheap to compute and can be called frequently.
#[allow(dead_code)]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct LightStatus {
	/// Memory usage in bytes.
	pub mem_usage: usize,
	/// Total number of transactions in the pool.
	pub transaction_count: usize,
	/// Number of unique senders in the pool.
	pub senders: usize,
}

/// A full queue status.
/// To compute this status it is required to provide `Ready`.
/// NOTE: To compute the status we need to visit each transaction in the pool.
#[allow(dead_code)]
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Status {
	/// Number of stalled transactions.
	pub stalled: usize,
	/// Number of pending (ready) transactions.
	pub pending: usize,
	/// Number of future (not ready) transactions.
	pub future: usize,
}
