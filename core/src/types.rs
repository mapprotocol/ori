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

use std::fmt;
use std::io::Cursor;
use serde::{Serialize, Deserialize,Deserializer, Serializer};
use hex;
pub use hex::FromHexError as HexError;
use ed25519::Message;
use ed25519::pubkey::Pubkey;
pub use ed25519::H256;
use hash;

/// Mainnet chain id

extern crate byteorder;
use byteorder::{BigEndian,LittleEndian,WriteBytesExt,ReadBytesExt};

pub const CHAIN_ID: u32 = 1;

#[derive(Default, Copy, Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    pub fn to_slice(&self) -> &[u8] {
        return &self.0
    }

    pub fn as_bytes(&self) -> &[u8] {
        return &self.0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
    pub fn to_msg(&self) -> Message {
        H256(self.0)
    }

    pub fn from_bytes(src: &[u8]) -> Self {
        let mut h = Self::default();
        if src.len() <= 32 {
            h.0[(32-src.len())..].copy_from_slice(src);
        } else {
            h.0.copy_from_slice(&src[(src.len()-32)..])
        }
        h
    }

    pub fn from_hex(text: &str) -> Result<Self, HexError> {
        let mut from = text;
        if text.starts_with("0x") || text.starts_with("0X") {
            from = &text[2..];
        }
        let b = hex::decode(from)?;

        Ok(Hash::from_bytes(&b))
    }
    pub fn make_hash(data: &[u8]) -> Self {
        Hash(hash::blake2b_256(data))
    }
    pub fn from_u64(val: u64) -> Self {
        let mut buf = vec![];
        buf.write_u64::<BigEndian>(val).unwrap();
        let s = buf.len();
        let mut bytes = [0x0; 20];
        bytes[(20 - s)..].copy_from_slice(&buf[..s]);
        Self::from_bytes(&bytes)
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.0.iter() {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x")?;
        for i in self.0[..4].iter() {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

impl fmt::LowerHex for Hash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x")?;
        for i in self.0[..4].iter() {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

impl AsRef<[u8]> for Hash {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        return &self.0
    }
}

impl AsMut<[u8]> for Hash {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl From<&[u8]> for Hash {
    fn from(src: &[u8]) -> Self {
        let mut h = Self::default();
        if src.len() <= 32 {
            h.0[(32-src.len())..].copy_from_slice(src);
        } else {
            h.0.copy_from_slice(&src[(src.len()-32)..])
        }
        h
    }
}

impl Serialize for Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(self.0)))
    }
}

impl<'a> Deserialize<'a> for Hash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'a>
    {
        let s = String::deserialize(deserializer)?;
        let hash = Hash::from_hex(s.as_str()).expect("Hash decode error");
        Ok(hash)
    }
}

#[derive(Serialize, Deserialize)]
#[derive(Default, Copy, Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Address(pub [u8; 20]);

impl Address {
    pub fn as_slice(&self) -> &[u8] {
        return &self.0
    }
    pub fn from_slice(bytes: &[u8]) -> Self {
        let mut addr = Self::default();
        addr.0.copy_from_slice(&bytes[..20]);
        addr
    }
    pub fn zero() -> Self {
        return Address::default();
    }
    pub fn from_hex(text: &str) -> Result<Self, HexError> {
        let mut addr = Self::default();
        let mut from = text;
        if text.starts_with("0x") || text.starts_with("0X") {
            from = &text[2..];
        }
        let b = hex::decode(from)?;
        addr.0.copy_from_slice(&b);
        Ok(addr)
    }
    pub fn from_low_u64_le(val: u64) -> Self {
        let mut buf = vec![];
        buf.write_u64::<LittleEndian>(val).unwrap();
        let s = buf.len();
        let mut bytes = [0x0; 20];
        bytes[(20 - s)..].copy_from_slice(&buf[..s]);
        Self::from_slice(&bytes)
    }
    pub fn from_low_u64_be(val: u64) -> Self {
        let mut buf = vec![];
        buf.write_u64::<BigEndian>(val).unwrap();
        let s = buf.len();
        let mut bytes = [0x0; 20];
        bytes[(20 - s)..].copy_from_slice(&buf[..s]);
        Self::from_slice(&bytes)
    }
    pub fn to_low_u64_be(&self) -> u64 {
        let mut buf = [0x0; 8];
        let data = self.as_slice();
        buf[..].copy_from_slice(&data[20-8..]);
        Cursor::new(buf.to_vec())
        .read_u64::<BigEndian>().unwrap()
    }
    pub fn to_low_u64_le(&self) -> u64 {
        let mut buf = [0x0; 8];
        let data = self.as_slice();
        buf[..].copy_from_slice(&data[20-8..]);
        Cursor::new(buf.to_vec()).read_u64::<LittleEndian>().unwrap()
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.0.iter() {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for i in self.0.iter() {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

impl From<Pubkey> for Address {
    fn from(pk: Pubkey) -> Self {
        let raw = pk.to_bytes();
        let mut addr = Address::default();
        addr.0.copy_from_slice(&(hash::blake2b_256(&raw)[12..]));
        addr
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_address() {
        {
            let hex_addr = "0000000000000000000000000000000000000000";
            let addr = Address::from_hex(hex_addr).unwrap();
            assert_eq!(addr, Address::default());
        }
        {
            let hex_addr = "0x0000000000000000000000000000000000000000";
            let addr = Address::from_hex(hex_addr).unwrap();
            assert_eq!(addr, Address::default());
        }
        {
            let hex_addr = "0X0000000000000000000000000000000000000000";
            let addr = Address::from_hex(hex_addr).unwrap();
            assert_eq!(addr, Address::default());
        }

    }
}
