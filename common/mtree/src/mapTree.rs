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

#[cfg(not(any(feature = "use_hashbrown")))]
use std::collections::HashMap;
use std::path::PathBuf;

#[cfg(feature = "use_hashbrown")]
use hashbrown::HashMap;

use starling::merkle_bit::{BinaryMerkleTreeResult, MerkleBIT};
use starling::traits::{Array, Database, Decode, Encode};
use starling::tree::tree_branch::TreeBranch;
use starling::tree::tree_data::TreeData;
use starling::tree::tree_leaf::TreeLeaf;
use starling::tree::tree_node::TreeNode;
use starling::tree_hasher::TreeHasher;
// #[cfg(feature = "use_serde")]
use serde::de::DeserializeOwned;
// #[cfg(feature = "use_serde")]
use serde::Serialize;
use super::TreeDB;

pub struct MapTree<ArrayType = [u8; 32], ValueType = Vec<u8>>
where
    ArrayType: Array + Serialize + DeserializeOwned,
    ValueType: Encode + Decode,
{
    tree: MerkleBIT<
        TreeDB<ArrayType>,
        TreeBranch<ArrayType>,
        TreeLeaf<ArrayType>,
        TreeData,
        TreeNode<ArrayType>,
        TreeHasher,
        ValueType,
        ArrayType,
    >,
}

impl<ArrayType, ValueType> MapTree<ArrayType, ValueType>
where
    ArrayType: Array + Serialize + DeserializeOwned,
    ValueType: Encode + Decode,
{
    #[inline]
    pub fn open(path: &PathBuf, depth: usize) -> BinaryMerkleTreeResult<Self> {
        let db = TreeDB::open(path)?;
        let tree = MerkleBIT::from_db(db, depth)?;
        Ok(Self { tree })
    }

    #[inline]
    pub fn from_db(db: TreeDB<ArrayType>, depth: usize) -> BinaryMerkleTreeResult<Self> {
        let tree = MerkleBIT::from_db(db, depth)?;
        Ok(Self { tree })
    }
    /// Get items from the `MapTree`.  Returns a map of `Option`s which may include the corresponding values.
    #[inline]
    pub fn get(
        &self,
        root_hash: &ArrayType,
        keys: &mut [ArrayType],
    ) -> BinaryMerkleTreeResult<HashMap<ArrayType, Option<ValueType>>> {
        self.tree.get(root_hash, keys)
    }

    /// Gets a single key from the tree.
    #[inline]
    pub fn get_one(
        &self,
        root: &ArrayType,
        key: &ArrayType,
    ) -> BinaryMerkleTreeResult<Option<ValueType>> {
        self.tree.get_one(&root, &key)
    }

    /// Insert items into the `MapTree`.  Keys must be sorted.  Returns a new root hash for the `MapTree`.
    #[inline]
    pub fn insert(
        &mut self,
        previous_root: Option<&ArrayType>,
        keys: &mut [ArrayType],
        values: &[ValueType],
    ) -> BinaryMerkleTreeResult<ArrayType> {
        self.tree.insert(previous_root, keys, values)
    }

    /// Inserts a single value into a tree.
    #[inline]
    pub fn insert_one(
        &mut self,
        previous_root: Option<&ArrayType>,
        key: &ArrayType,
        value: &ValueType,
    ) -> BinaryMerkleTreeResult<ArrayType> {
        self.tree.insert_one(previous_root, key, value)
    }

    /// Remove all items with less than 1 reference under the given root.
    #[inline]
    pub fn remove(&mut self, root_hash: &ArrayType) -> BinaryMerkleTreeResult<()> {
        self.tree.remove(root_hash)
    }

    /// Generates an inclusion proof.  The proof consists of a list of hashes beginning with the key/value
    /// pair and traveling up the tree until the level below the root is reached.
    #[inline]
    pub fn generate_inclusion_proof(
        &self,
        root: &ArrayType,
        key: ArrayType,
    ) -> BinaryMerkleTreeResult<Vec<(ArrayType, bool)>> {
        self.tree.generate_inclusion_proof(root, key)
    }

    #[inline]
    pub fn verify_inclusion_proof(
        &self,
        root: &ArrayType,
        key: ArrayType,
        value: &ValueType,
        proof: &Vec<(ArrayType, bool)>,
    ) -> BinaryMerkleTreeResult<()> {
        self.tree.verify_inclusion_proof(root, key, value, proof)
    }
}


#[cfg(test)]
pub mod tests {
    extern crate rand;
    use std::path::PathBuf;
    use std::error::Error;
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    use starling::constants::KEY_LEN;
    use starling::merkle_bit::BinaryMerkleTreeResult;
    use starling::traits::Exception;
    use super::MapTree;

    fn generate_path(seed: [u8; KEY_LEN]) -> PathBuf {
        let mut rng: StdRng = SeedableRng::from_seed(seed);
        let suffix = rng.gen_range(1000, 100000);
        let path_string = format!("Test_DB_{}", suffix);
        PathBuf::from(path_string)
    }
    fn tear_down(_path: &PathBuf) {
        use std::fs::remove_dir_all;
        remove_dir_all(&_path).unwrap();
    }

    #[test]
    // #[cfg(feature = "use_serialization")]
    fn test01_real_database() -> BinaryMerkleTreeResult<()> {
        println!("");
        println!("begin test01_real_database");
        let seed = [0x00u8; KEY_LEN];
        let path = generate_path(seed);
        println!("path:{:?}",path);
        let key = [0xAAu8; KEY_LEN];
        let retrieved_value;
        let removed_retrieved_value;
        let data = vec![0xFFu8];
        {
            let values = vec![data.clone()];
            let mut tree = MapTree::open(&path, 160)?;
            let root;

            match tree.insert(None, &mut [key], &values) {
                Ok(r) => root = r,
                Err(e) => {
                    drop(tree);
                    tear_down(&path);
                    panic!("{:?}", e.description());
                }
            }
            println!("insert test,root:{:?}",root);

            match tree.get(&root, &mut [key]) {
                Ok(v) => retrieved_value = v,
                Err(e) => {
                    drop(tree);
                    tear_down(&path);
                    panic!("{:?}", e.description());
                }
            }
            println!("get test,root:{:?}",root);

            match tree.remove(&root) {
                Ok(_) => {}
                Err(e) => {
                    drop(tree);
                    tear_down(&path);
                    panic!("{:?}", e.description());
                }
            }
            println!("remove test,root:{:?}",root);

            match tree.get(&root, &mut [key]) {
                Ok(v) => removed_retrieved_value = v,
                Err(e) => {
                    drop(tree);
                    tear_down(&path);
                    panic!("{:?}", e.description());
                }
            }
            println!("get test after remove,root:{:?},v:{:?}",root,removed_retrieved_value);
        }
        tear_down(&path);
        assert_eq!(retrieved_value[&key], Some(data));
        assert_eq!(removed_retrieved_value[&key], None);
        println!("end test01_real_database");
        Ok(())
    }
    #[test]
    fn test02_data_version() -> BinaryMerkleTreeResult<()> {
        println!("");
        println!("begin test02_data_version");
        let path = PathBuf::from("testdb_04".to_string());
        let keys = [[0x01u8; KEY_LEN],[0x02u8; KEY_LEN],[0x03u8; KEY_LEN],
                                    [0x04u8; KEY_LEN],[0x05u8; KEY_LEN],[0x06u8; KEY_LEN],
                                    [0x07u8; KEY_LEN],[0x08u8; KEY_LEN],[0x09u8; KEY_LEN],[0x0Au8; KEY_LEN]];
        let values = vec![0x01u8,0x02u8,0x03u8,0x04u8,0x05u8,0x06u8,
        0x07u8,0x08u8,0x09u8,0x0Au8];
        let mut root;
        let mut root0 = [0u8;32];
        let mut tree = MapTree::<[u8; 32],Vec<u8>>::open(&path, 160)?;
        for i in 0..10 {
            if i == 0 {
                let vv = vec![values[i].clone()];
                root = tree.insert(None, &mut [keys[i]], &[vv])?;
                root0 = root;
            } else {
                let vv = vec![values[i].clone()];
                root = tree.insert(Some(&root0), &mut [keys[i]], &[vv])?;
                root0 = root;
                println!("insert:{},key:{:X},value:{:?}",i,keys[i][0],values[i].clone());
                let val = tree.get(&root0, &mut [keys[i-1]])?;
                println!("get key:{},key:{:X},value {:?}",i-1,keys[i-1][0],val);
                assert_eq!(val[&keys[i-1]],Some(vec![values[i-1].clone()]));
            }
        }

        // assert_eq!(retrieved_value[&key], Some(data));
        // assert_eq!(retrieved_value2[&key], Some(data2));
        println!("end test02_data_version");
        Ok(())
    }
}
