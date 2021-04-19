use super::VerifiedTransaction;

/// Transaction verification.
///
/// Verifier is responsible to decide if the transaction should even be considered for pool inclusion.
pub trait Verifier<U> {
	/// Verification error.
	type Error;

	/// Verified transaction.
	type VerifiedTransaction: VerifiedTransaction;

	/// Verifies a `UnverifiedTransaction` and produces `VerifiedTransaction` instance.
	fn verify_transaction(&self, tx: U) -> Result<Self::VerifiedTransaction, Self::Error>;
}
