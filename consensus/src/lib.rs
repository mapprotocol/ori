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


#[macro_use]
extern crate enum_display_derive;
#[macro_use]
extern crate log;
extern crate ed25519;

use failure::{Backtrace, err_msg, Context, Fail};
use std::fmt::{self, Display, Debug};
use errors::{Error, ErrorKind};

pub mod poa;
pub mod traits;

#[allow(dead_code)]
#[allow(non_upper_case_globals)]
const os_seed_share_count: i32 = 10;
//////////////////////////////////////////////////////////////////
#[derive(Debug)]
pub struct ConsensusError {
    kind: Context<ConsensusErrorKind>,
}

#[derive(Debug, Clone, Eq, PartialEq, Display)]
pub enum ConsensusErrorKind {
    NoneSign,
    AnotherPk,
    InvalidProof,
    InvalidKey,
    NotMatchEpochID,
    NoValidatorsInEpoch,
    EncryptedShareMsgError,
    DecryptShareMsgError,
    NotMatchLocalHolders,
    RecoverSharesError,
    NotEnoughShares,
    NotFoundSeedInfo,
    NotFetchAnyShares,
}

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(cause) = self.cause() {
            write!(f, "{}({})", self.kind(), cause)
        } else {
            write!(f, "{}", self.kind())
        }
    }
}

impl From<ConsensusError> for Error {
    fn from(error: ConsensusError) -> Self {
        error.context(ErrorKind::Consensus).into()
    }
}

impl From<ConsensusErrorKind> for ConsensusError {
    fn from(kind: ConsensusErrorKind) -> Self {
        ConsensusError {
            kind: Context::new(kind),
        }
    }
}

impl From<ConsensusErrorKind> for Error {
    fn from(kind: ConsensusErrorKind) -> Self {
        Into::<ConsensusError>::into(kind).into()
    }
}

impl ConsensusErrorKind {
    pub fn cause<F: Fail>(self, cause: F) -> ConsensusError {
        ConsensusError {
            kind: cause.context(self),
        }
    }

    pub fn reason<S: Display + Debug + Sync + Send + 'static>(self, reason: S) -> ConsensusError {
        ConsensusError {
            kind: err_msg(reason).compat().context(self),
        }
    }
}

impl ConsensusError {
    pub fn kind(&self) -> &ConsensusErrorKind {
        &self.kind.get_context()
    }
}

impl Fail for ConsensusError {
    fn cause(&self) -> Option<&dyn Fail> {
        self.kind.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.kind.backtrace()
    }
}
