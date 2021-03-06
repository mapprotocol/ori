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
use std::borrow::Borrow;
use std::ops::Range;
use plain_hasher::PlainHasher;
use rlp::{DecoderError, RlpStream, Rlp, Prototype, Encodable};
use hash_db;
use trie_db;
use trie_db::{TrieLayout, NodeCodec, ChildReference, Partial,
    node::{NibbleSlicePlan, NodePlan, NodeHandlePlan}};
use hash as core_hash;
use crate::types::Hash;

const HASHED_NULL_NODE_BYTES :[u8;32] = [0x3a, 0xc4, 0xbf, 0x3c, 0xc9, 0x2d, 0x05, 0x46, 0x3d, 0x1e, 0x9c, 0x40, 0x24, 0xca, 0xab, 0x4c, 0x78, 0x0f, 0x9d, 0x99, 0xd2, 0x4d, 0xd2, 0xf4, 0x55, 0x70, 0x86, 0x94, 0xb5, 0x0a, 0x00, 0xc9];
/// hash of null node with rlp encoded([0x80])
pub const NULL_ROOT : Hash = Hash(HASHED_NULL_NODE_BYTES);
/// Empty node with rlp of null item
pub const EMPTY_TRIE: &[u8] = &[0x80];

impl Encodable for Hash {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.encoder().encode_value(&self.0);
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Blake2Hasher;

impl hash_db::Hasher for Blake2Hasher {
    type Out = Hash;
    type StdHasher = PlainHasher;
    const LENGTH: usize = 32;

    fn hash(x: &[u8]) -> Self::Out {
        let out = core_hash::blake2b_256(x);
        out[..].into()
    }
}

/// implementation of a `NodeCodec`.
#[derive(Default, Clone)]
pub struct BinNodeCodec<H> {
    mark: PhantomData<H>
}

/// layout using modified partricia trie with extention node
#[derive(Clone, Default)]
pub struct ExtensionLayout;

impl TrieLayout for ExtensionLayout {
    const USE_EXTENSION: bool = true;
    type Hash = Blake2Hasher;
    type Codec = BinNodeCodec<Blake2Hasher>;
}

/// Encode a partial value with a partial tuple as input.
fn encode_partial_iter<'a>(partial: Partial<'a>, is_leaf: bool) -> impl Iterator<Item = u8> + 'a {
    encode_partial_inner_iter((partial.0).1, partial.1.iter().map(|v| *v), (partial.0).0 > 0, is_leaf)
}

/// Encode a partial value with an iterator as input.
fn encode_partial_from_iterator_iter<'a>(
    mut partial: impl Iterator<Item = u8> + 'a,
    odd: bool,
    is_leaf: bool,
) -> impl Iterator<Item = u8> + 'a {
    let first = if odd { partial.next().unwrap_or(0) } else { 0 };
    encode_partial_inner_iter(first, partial, odd, is_leaf)
}

/// Encode a partial value with an iterator as input.
fn encode_partial_inner_iter<'a>(
    first_byte: u8,
    partial_remaining: impl Iterator<Item = u8> + 'a,
    odd: bool,
    is_leaf: bool,
) -> impl Iterator<Item = u8> + 'a {
    let encoded_type = if is_leaf {0x20} else {0};
    let first = if odd {
        0x10 + encoded_type + first_byte
    } else {
        encoded_type
    };
    std::iter::once(first).chain(partial_remaining)
}

fn decode_value_range(rlp: Rlp, mut offset: usize) -> Result<Range<usize>, DecoderError> {
    let payload = rlp.payload_info()?;
    offset += payload.header_len;
    Ok(offset..(offset + payload.value_len))
}

fn decode_child_handle_plan<H: hash_db::Hasher>(child_rlp: Rlp, mut offset: usize)
    -> Result<NodeHandlePlan, DecoderError>
{
    Ok(if child_rlp.is_data() && child_rlp.size() == H::LENGTH {
        let payload = child_rlp.payload_info()?;
        offset += payload.header_len;
        NodeHandlePlan::Hash(offset..(offset + payload.value_len))
    } else {
        NodeHandlePlan::Inline(offset..(offset + child_rlp.as_raw().len()))
    })
}

impl NodeCodec for BinNodeCodec<Blake2Hasher> {
    type Error = DecoderError;
    type HashOut = <Blake2Hasher as hash_db::Hasher>::Out;

    fn hashed_null_node() -> <Blake2Hasher as hash_db::Hasher>::Out {
        // HASHED_NULL_NODE
        let out = core_hash::blake2b_256(<Self as NodeCodec>::empty_node());
        out[..].into()
    }

    fn decode_plan(data: &[u8]) -> ::std::result::Result<NodePlan, Self::Error> {
        let r = Rlp::new(data);
        match r.prototype()? {
            // either leaf or extension - decode first item with NibbleSlice::???
            // and use is_leaf return to figure out which.
            // if leaf, second item is a value (is_data())
            // if extension, second item is a node (either SHA3 to be looked up and
            // fed back into this function or inline RLP which can be fed back into this function).
            Prototype::List(2) => {
                let (partial_rlp, mut partial_offset) = r.at_with_offset(0)?;
                let partial_payload = partial_rlp.payload_info()?;
                partial_offset += partial_payload.header_len;

                let (partial, is_leaf) = if partial_rlp.is_empty() {
                    (NibbleSlicePlan::new(partial_offset..partial_offset, 0), false)
                } else {
                    let partial_header = partial_rlp.data()?[0];
                    // check leaf bit from header.
                    let is_leaf = partial_header & 32 == 32;
                    // Check the header bit to see if we're dealing with an odd partial (only a nibble of header info)
                    // or an even partial (skip a full byte).
                    let (start, byte_offset) = if partial_header & 16 == 16 { (0, 1) } else { (1, 0) };
                    let range = (partial_offset + start)..(partial_offset + partial_payload.value_len);
                    (NibbleSlicePlan::new(range, byte_offset), is_leaf)
                };

                let (value_rlp, value_offset) = r.at_with_offset(1)?;
                Ok(if is_leaf {
                    let value = decode_value_range(value_rlp, value_offset)?;
                    NodePlan::Leaf { partial, value }
                } else {
                    let child = decode_child_handle_plan::<Blake2Hasher>(value_rlp, value_offset)?;
                    NodePlan::Extension { partial, child }
                })
            },
            // branch - first 16 are nodes, 17th is a value (or empty).
            Prototype::List(17) => {
                let mut children = [
                    None, None, None, None, None, None, None, None,
                    None, None, None, None, None, None, None, None,
                ];
                for (i, child) in children.iter_mut().enumerate() {
                    let (child_rlp, child_offset) = r.at_with_offset(i)?;
                    if !child_rlp.is_empty() {
                        *child = Some(
                            decode_child_handle_plan::<Blake2Hasher>(child_rlp, child_offset)?
                        );
                    }
                }
                let (value_rlp, value_offset) = r.at_with_offset(16)?;
                let value = if value_rlp.is_empty() {
                    None
                } else {
                    Some(decode_value_range(value_rlp, value_offset)?)
                };
                Ok(NodePlan::Branch { value, children })
            },
            // an empty branch index.
            Prototype::Data(0) => Ok(NodePlan::Empty),
            // something went wrong.
            _ => Err(DecoderError::Custom("Rlp is not valid."))
        }
    }

    fn is_empty_node(data: &[u8]) -> bool {
        Rlp::new(data).is_empty()
    }

    fn empty_node() -> &'static[u8] {
        EMPTY_TRIE
    }

    fn leaf_node(partial: Partial, value: &[u8]) -> Vec<u8> {
        let mut stream = RlpStream::new_list(2);
        stream.append_iter(encode_partial_iter(partial, true));
        stream.append(&value);
        stream.drain()
    }

    fn extension_node(
        partial: impl Iterator<Item = u8>,
        number_nibble: usize,
        child_ref: ChildReference<<Blake2Hasher as hash_db::Hasher>::Out>,
    ) -> Vec<u8> {
        let mut stream = RlpStream::new_list(2);
        stream.append_iter(encode_partial_from_iterator_iter(partial, number_nibble % 2 > 0, false));
        match child_ref {
            ChildReference::Hash(hash) => stream.append(&hash),
            ChildReference::Inline(inline_data, length) => {
                let bytes = &AsRef::<[u8]>::as_ref(&inline_data)[..length];
                stream.append_raw(bytes, 1)
            },
        };
        stream.drain()
    }

    fn branch_node(
        children: impl Iterator<Item = impl Borrow<Option<ChildReference<<Blake2Hasher as hash_db::Hasher>::Out>>>>,
        maybe_value: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut stream = RlpStream::new_list(17);
        for child_ref in children {
            match child_ref.borrow() {
                Some(c) => match c {
                    ChildReference::Hash(h) => {
                        stream.append(h)
                    },
                    ChildReference::Inline(inline_data, length) => {
                        let bytes = &AsRef::<[u8]>::as_ref(inline_data)[..*length];
                        stream.append_raw(bytes, 1)
                    },
                },
                None => stream.append_empty_data()
            };
        }
        if let Some(value) = maybe_value {
            stream.append(&&*value);
        } else {
            stream.append_empty_data();
        }
        stream.drain()
    }

    fn branch_node_nibbled(
        _partial:   impl Iterator<Item = u8>,
        _number_nibble: usize,
        _children: impl Iterator<Item = impl Borrow<Option<ChildReference<Self::HashOut>>>>,
        _maybe_value: Option<&[u8]>) -> Vec<u8> {
        unreachable!("This codec is only used with a trie Layout that uses extension node.")
    }
}

pub type TrieDBMut<'db> = trie_db::TrieDBMut<'db, ExtensionLayout>;

pub type TrieDB<'db> = trie_db::TrieDB<'db, ExtensionLayout>;

pub type TrieFactory = trie_db::TrieFactory<ExtensionLayout>;

pub type MemoryDB = memory_db::MemoryDB<
    Blake2Hasher,
    memory_db::HashKey<Blake2Hasher>,
    trie_db::DBValue,
>;

#[cfg(test)]
mod tests {
    use trie_db::{DBValue, Trie, TrieMut};
    use memory_db::{MemoryDB, HashKey};
    use crate::types::Hash;
    use hash as core_hash;
    use super::{TrieDBMut, TrieDB, Blake2Hasher, NULL_ROOT, EMPTY_TRIE};

    #[test]
    fn test_trie_mut() {
        // empty item of rlp
        let null_root = Hash(core_hash::blake2b_256(EMPTY_TRIE));
        assert_eq!(null_root, NULL_ROOT);
        let long_node = vec![1u8;33];

        let mut memdb = MemoryDB::<Blake2Hasher, HashKey<_>, DBValue>::new(EMPTY_TRIE);
        let mut root: Hash = Default::default();
        {
            let mut t = TrieDBMut::new(&mut memdb, &mut root);
            assert!(t.is_empty());
            assert_eq!(*t.root(), NULL_ROOT);
            t.insert(b"foo", b"b").unwrap();
            t.insert(b"fog", b"a").unwrap();
        }

        {
            let t = TrieDB::new(&memdb, &root).unwrap();
            assert!(!t.is_empty());
            assert!(t.contains(b"foo").unwrap());
            assert_eq!(t.get(b"foo").unwrap().unwrap(), b"b".to_vec());
            assert_eq!(t.get(b"fog").unwrap().unwrap(), b"a".to_vec());
        }

        {
            let mut t = TrieDBMut::new(&mut memdb, &mut root);
            t.insert(b"fot", &long_node).unwrap();
            assert_eq!(t.get(b"fot").unwrap().unwrap(), long_node);
        }
    }
}
