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

extern crate ed25519_dalek;
extern crate errors;
extern crate sha2;

use std::fmt;

use ed25519_dalek::{ExpandedSecretKey, PublicKey, SecretKey};
use ed25519_dalek::SECRET_KEY_LENGTH;
use hex;
pub use hex::FromHexError;
use sha2::Sha512;

use errors::{Error, InternalErrorKind};

use super::{H256, pubkey::Pubkey, signature::SignatureInfo};

#[derive(Debug, Eq, PartialEq, Clone,Copy)]
pub struct PrivKey {
    inner: H256,
}

impl PrivKey {

    pub fn to_bytes(&self) -> [u8; SECRET_KEY_LENGTH] {
        self.inner.0
    }
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut pkey: [u8; 32] = [0u8; 32];
        pkey.copy_from_slice(&bytes[..32]);
        PrivKey{inner: H256(pkey)}
    }
    pub fn to_secrit_key(&self) -> Result<SecretKey, Error> {
        let data = self.inner.0;
        SecretKey::from_bytes(&data[..])
        .map_err(|e|InternalErrorKind::Other(e.to_string()).into())
    }

    pub fn from_secret_key(key: &SecretKey) -> Self {
        PrivKey{inner: H256(key.to_bytes())}
    }
    pub fn to_pubkey(&self) -> Result<Pubkey,Error> {
        let sk: SecretKey = self.to_secrit_key()?;
        let public_key: PublicKey = PublicKey::from_secret::<Sha512>(&sk);
        Ok(Pubkey::from_pubkey(&public_key))
    }
    pub fn sign(&self,message: &[u8]) -> Result<SignatureInfo,Error> {
        let sk: SecretKey = self.to_secrit_key()?;
        let expanded_secret: ExpandedSecretKey = ExpandedSecretKey::from_secret_key::<Sha512>(&sk);
        let pk: PublicKey = self.to_pubkey()?.to_pubkey()?;
        let sign_data = expanded_secret.sign::<Sha512>(&message,&pk);
        let mut p = [0u8;32];
        p[..].copy_from_slice(&pk.to_bytes()[..]);
        Ok(SignatureInfo::from_signature(&sign_data,p))
    }

    pub fn from_hex(text: &str) -> Result<Self, FromHexError> {
        let mut from = text;
        if text.starts_with("0x") || text.starts_with("0X") {
            from = &text[2..];
        }
        let b = hex::decode(from)?;
        let mut pkey: [u8; 32] = [0u8; 32];
        if b.len() != 32 {
            return Err(FromHexError::InvalidStringLength);
        }
        pkey.copy_from_slice(&b);
        Ok(PrivKey{inner: H256(pkey)})
    }
}

impl fmt::Display for PrivKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x")?;
        for i in self.to_bytes().iter() {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}
