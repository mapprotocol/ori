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

extern crate rocksdb;
pub mod mapdb;
pub type Error = rocksdb::Error;
pub type WriteBatch = rocksdb::WriteBatch;

use std::path::PathBuf;
use std::env;
use std::io;
use std::sync::RwLock;
use std::collections::HashMap;

pub trait KVDB: Sync + Send {
    fn get(&self, key: &[u8]) -> io::Result<Option<Vec<u8>>>;

    fn put(&mut self, key: &[u8], value: &[u8]) -> io::Result<()>;

    fn remove(&mut self, key: &[u8]) -> io::Result<()>;
}

#[derive(Default)]
pub struct MemoryKV {
    db: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

impl MemoryKV {
    pub fn new() -> Self {
        MemoryKV {
            db: RwLock::new(HashMap::new()),
        }
    }
}

impl KVDB for MemoryKV {
    fn put(&mut self, key: &[u8], value: &[u8]) -> io::Result<()> {
        let mut db = self.db.write().unwrap();
        db.insert(key.into(), value.into());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> io::Result<Option<Vec<u8>>> {
        let db = self.db.read().unwrap();
        let ret = match db.get(key) {
            Some(v) => Some(v.clone()),
            None => None,
        };
        Ok(ret)
    }

    fn remove(&mut self, key: &[u8]) -> io::Result<()> {
        let mut db = self.db.write().unwrap();
        db.remove(key);
        Ok(())
    }
}

#[derive(Clone,Debug)]
pub struct Config {
    pub path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let mut cur = env::current_dir().unwrap();
        cur.push("mapdata");
        Config{
            path:   cur,
        }
    }
}

impl Config {
    pub fn new(mut dir: PathBuf) -> Self {
        dir.push("mapdata");
        Config {
            path: dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryKV, KVDB};

    #[test]
    fn test_memdb() {
        let mut db = MemoryKV::new();

        db.put(b"key1", b"a").unwrap();
        assert_eq!(db.get(b"key1").unwrap().unwrap(), b"a");

        db.put(b"key1", b"b").unwrap();
        assert_eq!(db.get(b"key1").unwrap().unwrap(), b"b");

        db.remove(b"key1").unwrap();
        assert_eq!(db.get(b"key1").unwrap(), None);
    }
}
