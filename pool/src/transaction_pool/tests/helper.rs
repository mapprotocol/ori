
use std::cmp;
use std::collections::HashMap;

use super::Transaction;
use super::{pool, scoring, Readiness, Ready, ReplaceTransaction, Scoring, ShouldReplace};
use map_core::types::{Hash,Address};

#[derive(Debug, Default)]
pub struct DummyScoring {
	always_insert: bool,
}

impl DummyScoring {
	pub fn always_insert() -> Self {
		DummyScoring { always_insert: true }
	}
}

impl Scoring<Transaction> for DummyScoring {
	type Score = u64;
	type Event = ();

	fn compare(&self, old: &Transaction, new: &Transaction) -> cmp::Ordering {
		old.nonce.cmp(&new.nonce)
	}

	fn choose(&self, old: &Transaction, new: &Transaction) -> scoring::Choice {
		if old.nonce == new.nonce {
			if new.gas_price > old.gas_price {
				scoring::Choice::ReplaceOld
			} else {
				scoring::Choice::RejectNew
			}
		} else {
			scoring::Choice::InsertNew
		}
	}

	fn update_scores(
		&self,
		txs: &[pool::Transaction<Transaction>],
		scores: &mut [Self::Score],
		change: scoring::Change,
	) {
		if let scoring::Change::Event(_) = change {
			// In case of event reset all scores to 0
			for i in 0..txs.len() {
				scores[i] = 0 as u64;
			}
		} else {
			// Set to a gas price otherwise
			for i in 0..txs.len() {
				scores[i] = txs[i].gas_price;
			}
		}
	}

	fn should_ignore_sender_limit(&self, _new: &Transaction) -> bool {
		self.always_insert
	}
}

impl ShouldReplace<Transaction> for DummyScoring {
	fn should_replace(
		&self,
		old: &ReplaceTransaction<'_, Transaction>,
		new: &ReplaceTransaction<'_, Transaction>,
	) -> scoring::Choice {
		if self.always_insert {
			scoring::Choice::InsertNew
		} else if new.gas_price > old.gas_price {
			scoring::Choice::ReplaceOld
		} else {
			scoring::Choice::RejectNew
		}
	}
}

#[derive(Default)]
pub struct NonceReady(HashMap<Address, u64>, u64);

impl NonceReady {
	pub fn new<T: Into<u64>>(min: T) -> Self {
		let mut n = NonceReady::default();
		n.1 = min.into();
		n
	}
}

impl Ready<Transaction> for NonceReady {
	fn is_ready(&mut self, tx: &Transaction) -> Readiness {
		let min = self.1;
		let nonce = self.0.entry(tx.sender).or_insert_with(|| min);
		match tx.nonce.cmp(nonce) {
			cmp::Ordering::Greater => Readiness::Future,
			cmp::Ordering::Equal => {
				*nonce += 1 as u64;
				Readiness::Ready
			}
			cmp::Ordering::Less => Readiness::Stale,
		}
	}
}



#[derive(Debug, Default, Clone)]
pub struct TransactionBuilder {
	nonce: u64,
	gas_price: u64,
	gas: u64,
	sender: Address,
	mem_usage: usize,
}

impl TransactionBuilder {
	pub fn tx(&self) -> Self {
		self.clone()
	}

	pub fn nonce(mut self, nonce: usize) -> Self {
		self.nonce = nonce as u64;
		self
	}

	pub fn gas_price(mut self, gas_price: usize) -> Self {
		self.gas_price = gas_price as u64;
		self
	}

	pub fn sender(mut self, sender: u64) -> Self {
		self.sender = Address::from_low_u64_be(sender);
		self
	}

	pub fn mem_usage(mut self, mem_usage: usize) -> Self {
		self.mem_usage = mem_usage;
		self
	}

	pub fn new(self) -> Transaction {
		let hash: u64 = self.nonce
			^ (100 as u64 * self.gas_price)
			^ (100_000 as u64 * self.sender.to_low_u64_be());
		Transaction {
			hash: Hash::from_u64(hash),
			nonce: self.nonce,
			gas_price: self.gas_price,
			gas: 21_000 as u64,
			sender: self.sender,
			mem_usage: self.mem_usage,
		}
	}
}
