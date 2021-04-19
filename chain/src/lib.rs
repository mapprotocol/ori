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
extern crate log;
extern crate futures;
extern crate errors;
#[macro_use]
extern crate enum_display_derive;


pub mod store;
pub mod blockchain;
use std::fmt::{self, Display,Debug};
use errors::{Error,ErrorKind};
use failure::{Backtrace,err_msg, Context, Fail};

//////////////////////////////////////////////////////////////////
#[derive(Debug)]
pub struct BlockChainError {
    kind: Context<BlockChainErrorKind>,
}

#[derive(Debug, Clone, Eq, PartialEq, Display)]
pub enum BlockChainErrorKind {
    UnknownAncestor,
    KnownBlock,
    MismatchHash,
    InvalidBlockProof,
    InvalidBlockTime,
    InvalidBlockHeight,
    InvalidState,
    InvalidAuthority,
}

#[derive(Debug, PartialEq)]
pub enum BlockProcessState {
    Processed,
    ParentUnknown,
    FutureBlock,
    BlockIsAlreadyKnown,
}


impl fmt::Display for BlockChainError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(cause) = self.cause() {
            write!(f, "{}({})", self.kind(), cause)
        } else {
            write!(f, "{}", self.kind())
        }
    }
}

impl From<BlockChainError> for Error {
    fn from(error: BlockChainError) -> Self {
        error.context(ErrorKind::Consensus).into()
    }
}

impl From<BlockChainErrorKind> for BlockChainError {
    fn from(kind: BlockChainErrorKind) -> Self {
        BlockChainError {
            kind: Context::new(kind),
        }
    }
}

impl From<BlockChainErrorKind> for Error {
    fn from(kind: BlockChainErrorKind) -> Self {
        Into::<BlockChainError>::into(kind).into()
    }
}

impl BlockChainErrorKind {
    pub fn cause<F: Fail>(self, cause: F) -> BlockChainError {
        BlockChainError {
            kind: cause.context(self),
        }
    }
    pub fn reason<S: Display + Debug + Sync + Send + 'static>(self, reason: S) -> BlockChainError {
        BlockChainError {
            kind: err_msg(reason).compat().context(self),
        }
    }
}

impl BlockChainError {
    pub fn kind(&self) -> &BlockChainErrorKind {
        &self.kind.get_context()
    }
}

impl Fail for BlockChainError {
    fn cause(&self) -> Option<&dyn Fail> {
        self.kind.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.kind.backtrace()
    }
}
