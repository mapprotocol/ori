
//! When queue limits are reached, decide whether to replace an existing transaction from the pool

use super::{pool::Transaction, scoring::Choice};

/// Encapsulates a transaction to be compared, along with pooled transactions from the same sender
pub struct ReplaceTransaction<'a, T> {
	/// The transaction to be compared for replacement
	pub transaction: &'a Transaction<T>,
	/// Other transactions currently in the pool for the same sender
	pub pooled_by_sender: Option<&'a [Transaction<T>]>,
}

impl<'a, T> ReplaceTransaction<'a, T> {
	/// Creates a new `ReplaceTransaction`
	pub fn new(transaction: &'a Transaction<T>, pooled_by_sender: Option<&'a [Transaction<T>]>) -> Self {
		ReplaceTransaction { transaction, pooled_by_sender }
	}
}

impl<'a, T> ::std::ops::Deref for ReplaceTransaction<'a, T> {
	type Target = Transaction<T>;
	fn deref(&self) -> &Self::Target {
		&self.transaction
	}
}

/// Chooses whether a new transaction should replace an existing transaction if the pool is full.
pub trait ShouldReplace<T> {
	/// Decides if `new` should push out `old` transaction from the pool.
	///
	/// NOTE returning `InsertNew` here can lead to some transactions being accepted above pool limits.
	fn should_replace(&self, old: &ReplaceTransaction<'_, T>, new: &ReplaceTransaction<'_, T>) -> Choice;
}
