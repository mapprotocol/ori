
/// Transaction Pool options.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
	/// Maximal number of transactions in the pool.
	pub max_count: usize,
	/// Maximal number of transactions from single sender.
	pub max_per_sender: usize,
	/// Maximal memory usage.
	pub max_mem_usage: usize,
}

impl Default for Options {
	fn default() -> Self {
		Options { max_count: 1024, max_per_sender: 16, max_mem_usage: 8 * 1024 * 1024 }
	}
}
