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

use map_store::mapdb::MapDB;
use map_store::Config;
use map_store::Error;
use map_core::block::{Header, Block};
use map_core::types::Hash;
use bincode;

const HEADER_PREFIX: u8 = 'h' as u8;
const HEAD_PREFIX: u8 = 'H' as u8;
const BLOCK_PREFIX: u8 = 'b' as u8;
const HEADERHASH_PREFIX: u8 = 'n' as u8;
const HEAD_KEY: &str = "HEAD";


/// Blockchain storage backend implement
pub struct ChainDB {
    db: MapDB,
}

impl ChainDB {

    pub fn new(cfg: Config) -> Result<Self, Error> {
        let m = MapDB::open(cfg).unwrap();
        Ok(ChainDB{db: m})
    }

    // Save block header by hash (hash --> blockHeader)
    pub fn write_header(&mut self, h: &Header) -> Result<(), Error> {
        let encoded: Vec<u8> = bincode::serialize(h).unwrap();
        let key = Self::header_key(&(h.hash().0));
        self.write_header_hash(h.height, &h.hash())?;
        self.db.put(&key, &encoded)
    }

    // Read block header by hash (hash --> blockHeader)
    pub fn get_header(&self, h: &Hash) -> Option<Header> {
        let key = Self::header_key(&(h.0));
        let serialized = match self.db.get(&key.as_slice()) {
            Some(s) => s,
            None => return None,
        };

        let header: Header = bincode::deserialize(&serialized.as_slice()).unwrap();
        Some(header)
    }

    // Delete a block header by hash (hash --> blockHeader)
    pub fn delete_header(&mut self, h: &Hash) -> Result<(), Error> {
        let key = Self::header_key(&(h.0));
        self.db.remove(&key[..])
    }

    pub fn get_header_by_number(&self, num: u64) -> Option<Header> {
        let header_hash = match self.get_header_hash(num) {
            Some(h) => h,
            None => return None,
        };

        self.get_header(&header_hash)
    }

    pub fn head_header(&self) -> Option<Header> {
        let header_hash = match self.head_hash() {
            Some(h) => h,
            None => return None,
        };
        let key = Self::header_key(&(header_hash.0));
        let serialized = match self.db.get(&key.as_slice()) {
            Some(s) => s,
            None => return None,
        };

        let header: Header = bincode::deserialize(&serialized.as_slice()).unwrap();
        Some(header)
    }

    pub fn head_hash(&self) -> Option<Hash> {
        let h = match self.db.get(&Self::head_key()[..]) {
            Some(h) => h,
            None => return None,
        };
        let mut hash: Hash = Default::default();
        hash.0.copy_from_slice(h.as_slice());
        Some(hash)
    }

    pub fn write_head_hash(&mut self, hash: Hash) -> Result<(), Error>{
        let key = Self::head_key();
        self.db.put(&key, hash.to_slice())
    }

    // read block header hash to certain height (num --> hash)
    pub fn get_header_hash(&self, num: u64) -> Option<Hash> {
        let key = Self::header_hash_key(num);
        self.db.get(&key).map(|h| {
            let mut hash: Hash = Default::default();
            hash.0.copy_from_slice(h.as_slice());
            hash
        })
    }

    // write header hash to num (num --> hash)
    pub fn write_header_hash(&mut self, num: u64, hash: &Hash) -> Result<(), Error> {
        let key = Self::header_hash_key(num);
        self.db.put(&key, hash.to_slice())
    }

    // remove the block assigned to certain height (num --> hash)
    pub fn delete_header_height(&mut self, num: u64) -> Result<(), Error> {
        let key = Self::header_hash_key(num);
        self.db.remove(&key)
    }

    pub fn head_block(&self) -> Option<Block> {
        let hash = match self.head_hash() {
            Some(h) => h,
            None => return None,
        };

        self.get_block(&hash)
    }

    pub fn get_block(&self, h: &Hash) -> Option<Block> {
        let key = Self::block_key(h);
        let serialized = match self.db.get(&key[..]) {
            Some(s) => s,
            None => return None,
        };

        let b: Block = bincode::deserialize(&serialized[..]).unwrap();
        Some(b)
    }

    pub fn get_block_by_number(&self, num: u64) -> Option<Block> {
        let header_hash = match self.get_header_hash(num) {
            Some(h) => h,
            None => return None,
        };

        self.get_block(&header_hash)
    }

    // Seek the common ancestor of two branch
    pub fn find_ancestor(&self, mut a: Header, mut b: Header) -> Option<Hash> {
        if a.height != b.height {
            return None
        }

        while a.hash() != b.hash() {
            a = match self.get_header(&a.parent_hash) {
                Some(h) => h,
                None => return None,
            };
            b = match self.get_header(&b.parent_hash) {
                Some(h) => h,
                None => return None,
            };
        }
        // Ancestor found here
        Some(a.hash())
    }

    pub fn setup_height(&mut self, h: &Header) {
        let mut pre = h.height;
        let mut pre_hash = h.hash();

        while pre > 0 {
            // Update num --> hash index if not set
            let header = self.get_header(&pre_hash).unwrap();
            if self.get_header_hash(pre).unwrap() == header.hash() {
                break;
            }
            self.write_header_hash(pre, &header.hash()).unwrap();
            pre = pre - 1;
            pre_hash = header.parent_hash;
        }
    }

    pub fn write_block(&mut self, block: &Block) -> Result<(), Error> {
        self.write_header(&block.header)?;
        let key = Self::block_key(&block.header.hash());
        let encoded: Vec<u8> = bincode::serialize(block).unwrap();
        self.db.put(&key, &encoded)
    }

    // Delete a block with header by hash
    pub fn delete_block(&mut self, h: &Hash) -> Result<(), Error> {
        // Delete block body
        let key = Self::block_key(h);
        self.db.remove(&key[..])?;
        // Delete it's header
        self.delete_header(h)
    }

    fn head_key() -> Vec<u8> {
        let mut pre = Vec::new();
        pre.push(HEAD_PREFIX);
        pre.extend_from_slice(HEAD_KEY.as_bytes());
        pre
    }

    fn header_key(_hash: &[u8]) -> Vec<u8> {
        let mut pre = Vec::new();
        pre.push(HEADER_PREFIX);
        pre.extend_from_slice(_hash);
        pre
    }

    fn header_hash_key(num: u64) -> Vec<u8> {
        let mut pre = Vec::new();
        pre.push(HEADERHASH_PREFIX);
        let bytes = num.to_be_bytes();
        pre.extend_from_slice(&bytes);
        pre
    }

    fn block_key(hash: &Hash) -> Vec<u8> {
        let mut pre = Vec::new();
        pre.push(BLOCK_PREFIX);
        pre.extend_from_slice(hash.to_slice());
        pre
    }
}
