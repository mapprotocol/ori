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

extern crate starling;
extern crate serde;
extern crate map_store;

use std::error::Error;
use std::path::PathBuf;
use starling::traits::{Array, Database, Decode, Encode, Exception};
use starling::tree::tree_node::TreeNode;
use std::marker::PhantomData;
use map_store::{mapdb::MapDB,Config};

pub struct MError(map_store::Error);
pub struct MWriteBatch(map_store::WriteBatch);
impl Default for MWriteBatch {
    fn default() -> Self {
        MWriteBatch(map_store::WriteBatch::default())
    }
}
unsafe impl Send for MWriteBatch {}
unsafe impl Sync for MWriteBatch {}

pub mod mapTree;

impl From<MError> for Exception {
    #[inline]
    fn from(error: MError) -> Self {
        Self::new(error.0.description())
    }
}
pub struct TreeDB<ArrayType>
where
    ArrayType: Array,
{
    db: MapDB,
    pending_inserts: Option<MWriteBatch>,
    array: PhantomData<ArrayType>,
}

impl<ArrayType> TreeDB<ArrayType>
where
    ArrayType: Array,
{
    #[inline]
    pub fn new(db: MapDB) -> Self {
        Self {
            db,
            pending_inserts: Some(MWriteBatch::default()),
            array: PhantomData,
        }
    }
}

impl<ArrayType> Database<ArrayType> for TreeDB<ArrayType>
where
    ArrayType: Array,
    TreeNode<ArrayType>: Encode + Decode,
{
    type NodeType = TreeNode<ArrayType>;
    type EntryType = (usize, usize);

    #[inline]
    fn open(path: &PathBuf) -> Result<Self, Exception> {
        let mut p = path.clone();
        let cfg = Config::new(p);
        let res = MapDB::open(cfg);
        match res {
            Ok(db) => Ok(Self::new(db)),
            Err(e) => Err(MError(e).into()),
        }
    }

    #[inline]
    fn get_node(&self, key: ArrayType) -> Result<Option<Self::NodeType>, Exception> {
        if let Some(buffer) = self.db.get(key.as_ref()) {
            Ok(Some(Self::NodeType::decode(buffer.as_ref())?))
        } else {
            Ok(None)
        }
    }

    #[inline]
    fn insert(&mut self, key: ArrayType, value: Self::NodeType) -> Result<(), Exception> {
        let serialized = value.encode()?;
        if let Some(wb) = &mut self.pending_inserts {
            let res = wb.0.put(key, serialized);
            match res {
                Ok(()) => Ok(()),
                Err(e) => Err(MError(e).into()),
            }
        } else {
            let mut wb = MWriteBatch::default();
            let res = wb.0.put(key, serialized);
            match res {
                Ok(()) => {
                    self.pending_inserts = Some(wb);
                    Ok(())
                },
                Err(e) => Err(MError(e).into()),
            }
        }
    }

    #[inline]
    fn remove(&mut self, key: &ArrayType) -> Result<(), Exception> {
        let res = self.db.remove(key.as_ref());
        match res {
            Ok(()) => Ok(()),
            Err(e) => Err(MError(e).into()),
        }
    }

    #[inline]
    fn batch_write(&mut self) -> Result<(), Exception> {
        if let Some(wb) = self.pending_inserts.replace(MWriteBatch::default()) {
            let res = self.db.write_batch(wb.0);
            match res {
                Ok(()) => {
                    self.pending_inserts = None;
                    Ok(())
                },
                Err(e) => Err(MError(e).into()),
            }
        } else {
            self.pending_inserts = None;
            Ok(())
        }
    }
}

#[cfg(test)]
pub mod tests {
    type keyType = [u8;8];
    use super::{MWriteBatch,TreeDB};
    use std::path::PathBuf;
    use map_store::{mapdb::MapDB,Config};
    use starling::traits::{Array, Database, Decode, Encode, Exception};

    #[test]
    fn test01_replace_field() {
        let mut pending_inserts = Some(MWriteBatch::default());
        if let Some(wb) = pending_inserts.replace(MWriteBatch::default()) {
            println!("ok");
        } else {
            println!("wrong replace on pending_inserts");
        }
    }
    #[test]
    fn test02_wb_replace_field() {
        let path_string = format!("Test_DB_{}", 100);
        let path = PathBuf::from(path_string);
        let cfg = Config::new(path);
        let db = MapDB::open(cfg).unwrap();
        let mut tdb = TreeDB::<keyType>::new(db);
        println!("create treedb ok...");
        println!("test write batch.....");
        let res = tdb.batch_write();
        match res {
            Ok(()) => println!("write batch finish"),
            Err(e) => println!("write batch error:{:?}",e),
        }
        println!("end of test.....");
    }
}
