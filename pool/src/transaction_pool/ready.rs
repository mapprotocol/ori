
/// Transaction readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
	/// The transaction is stale (and should/will be removed from the pool).
	Stale,
	/// The transaction is ready to be included in pending set.
	Ready,
	/// The transaction is not yet ready.
	Future,
}

/// A readiness indicator.
pub trait Ready<T> {
	/// Returns true if transaction is ready to be included in pending block,
	/// given all previous transactions that were ready are already included.
	///
	/// NOTE: readiness of transactions will be checked according to `Score` ordering,
	/// the implementation should maintain a state of already checked transactions.
	fn is_ready(&mut self, tx: &T) -> Readiness;
}

impl<T, F> Ready<T> for F
where
	F: FnMut(&T) -> Readiness,
{
	fn is_ready(&mut self, tx: &T) -> Readiness {
		(*self)(tx)
	}
}

impl<T, A, B> Ready<T> for (A, B)
where
	A: Ready<T>,
	B: Ready<T>,
{
	fn is_ready(&mut self, tx: &T) -> Readiness {
		match self.0.is_ready(tx) {
			Readiness::Ready => self.1.is_ready(tx),
			r => r,
		}
	}
}
