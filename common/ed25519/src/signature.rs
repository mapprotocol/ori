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
extern crate serde;
extern crate errors;

use errors::{Error,InternalErrorKind};
use serde::{Serialize, Deserialize};
use ed25519_dalek::{Signature};
// use ed25519_dalek::{PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH, KEYPAIR_LENGTH, SIGNATURE_LENGTH};
// use std::fmt;
// use faster_hex::hex_string;
// use std::str::FromStr;

#[derive(Serialize, Deserialize)]
#[derive(Debug, Default,Copy, Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct SignatureInfo([u8; 32], [u8;32],[u8;32]);

impl SignatureInfo {
    pub fn r(&self) -> &[u8] {
        &self.0[..]
    }
    pub fn s(&self) -> &[u8] {
        &self.1[..]
    }
    pub fn p(&self) -> &[u8] {
        &self.2[..]
    }
    pub fn make(r:[u8;32],s:[u8;32],p:[u8;32]) -> Self {
        SignatureInfo(r,s,p)
    }
    pub fn to_signature(&self) -> Result<Signature, Error> {
        let mut sig = [0u8; 64];
        sig[0..32].copy_from_slice(&self.0[..]);
        sig[32..64].copy_from_slice(&self.1[..]);
        Signature::from_bytes(&sig)
        .map_err(|e|InternalErrorKind::Other(e.to_string()).into())
    }
    pub fn from_signature(sign: &Signature,p:[u8;32]) -> Self {
        let data = sign.to_bytes();
        let mut r = [0u8;32];
        let mut s = [0u8;32];
        r[..].copy_from_slice(&data[0..32]);
        s[..].copy_from_slice(&data[32..64]);
        SignatureInfo(r,s,p)
    }
    pub fn from_slice(data: &[u8]) -> Result<Self, Error> {
        let mut sign_data = [0u8;64];
        let mut p = [0u8;32];
        sign_data[..].copy_from_slice(&data[0..64]);
        p[..].copy_from_slice(&data[64..96]);
        let sig  = Signature::from_bytes(&sign_data)
        .map_err(|e|InternalErrorKind::Other(e.to_string()))?;
        Ok(SignatureInfo::from_signature(&sig,p))
    }
}


// impl Default for SignatureInfo {
//     fn default() -> Self {
//         SignatureInfo([0u8;SIGNATURE_LENGTH])
//     }
// }
// impl fmt::Debug for SignatureInfo {
//     fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
//         f.debug_struct("Signature")
//             .field("r", &hex_string(&self.0[0..32]).expect("hex string"))
//             .field("s", &hex_string(&self.0[32..64]).expect("hex string"))
//             .field("v", &hex_string(&self.0[64..65]).expect("hex string"))
//             .finish()
//     }
// }

