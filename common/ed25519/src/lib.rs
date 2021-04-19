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

//! MAP ED25519.

use std::fmt;

pub mod generator;
pub mod privkey;
pub mod pubkey;
pub mod signature;


#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub struct H256(pub [u8; 32]);

impl fmt::Debug for H256{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.0.iter() {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

impl fmt::Display for H256 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.0[..4].iter() {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

pub type Message = H256;
