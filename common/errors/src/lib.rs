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

use failure::{Backtrace,err_msg, Context, Fail};
use std::fmt::{self, Display,Debug};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Display)]
pub enum ErrorKind {
    Header,
    BlockChain,
    Internal,
    Consensus,
}

#[derive(Debug)]
pub struct Error {
    kind: Context<ErrorKind>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(cause) = self.cause() {
            if f.alternate() {
                write!(f, "{}: {}", self.kind(), cause)
            } else {
                write!(f, "{}({})", self.kind(), cause)
            }
        } else {
            write!(f, "{}", self.kind())
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Self {
        Self { kind: inner }
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&dyn Fail> {
        self.kind.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.kind.backtrace()
    }
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        self.kind.get_context()
    }

    pub fn downcast_ref<T: Fail>(&self) -> Option<&T> {
        self.cause().and_then(|cause| cause.downcast_ref::<T>())
    }
}

//////////////////////////////////////////////////////////////////
#[derive(Debug)]
pub struct InternalError {
    kind: Context<InternalErrorKind>,
}

#[derive(Debug, Clone, Eq, PartialEq, Display)]
pub enum InternalErrorKind {
    InvalidSignData,
    BalanceNotEnough,
    InvalidTxNonce,
    NoneSign,
    Execute,
    Other(String),
}

impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(cause) = self.cause() {
            write!(f, "{}({})", self.kind(), cause)
        } else {
            write!(f, "{}", self.kind())
        }
    }
}

impl From<InternalError> for Error {
    fn from(error: InternalError) -> Self {
        error.context(ErrorKind::Internal).into()
    }
}

impl From<InternalErrorKind> for InternalError {
    fn from(kind: InternalErrorKind) -> Self {
        InternalError {
            kind: Context::new(kind),
        }
    }
}

impl From<InternalErrorKind> for Error {
    fn from(kind: InternalErrorKind) -> Self {
        Into::<InternalError>::into(kind).into()
    }
}

impl InternalErrorKind {
    pub fn cause<F: Fail>(self, cause: F) -> InternalError {
        InternalError {
            kind: cause.context(self),
        }
    }

    pub fn reason<S: Display + Debug + Sync + Send + 'static>(self, reason: S) -> InternalError {
        InternalError {
            kind: err_msg(reason).compat().context(self),
        }
    }
}

impl InternalError {
    pub fn kind(&self) -> &InternalErrorKind {
        &self.kind.get_context()
    }
}

impl Fail for InternalError {
    fn cause(&self) -> Option<&dyn Fail> {
        self.kind.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.kind.backtrace()
    }
}
