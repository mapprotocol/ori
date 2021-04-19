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

use std::marker::PhantomData;

use serde::{Serialize, Deserialize};
use crate::types::Hash;

#[derive(Serialize, Deserialize)]
pub struct ListEntry<T> {
    pub pre: Option<Hash>,
    pub next: Option<Hash>,
    pub payload: T,
}

pub struct List<T> {
    pub head_key: Hash,
    // pub state: &StateDB,
    phantom: PhantomData<T>
}

impl<T> List<T> {
    pub fn new(head: Hash) -> Self {
        List {
            head_key: head,
            phantom: PhantomData,
        }
    }
}
