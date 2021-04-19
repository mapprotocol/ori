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

extern crate rand_os;
extern crate ed25519_dalek;

use rand_os::OsRng;
use ed25519_dalek::SecretKey;
use super::{privkey::PrivKey,pubkey::Pubkey};

pub struct Generator {}

impl Default for Generator {
    fn default() -> Self {
        Generator{}
    }
}

impl Generator {
    pub fn new(&self) -> (PrivKey,Pubkey) {
        let mut csprng: OsRng = OsRng::new().unwrap();
        let sk: SecretKey = SecretKey::generate(&mut csprng);
        let priv_key: PrivKey = PrivKey::from_secret_key(&sk);
        (priv_key,priv_key.to_pubkey().unwrap())
    }
}

pub fn create_key() -> (PrivKey, Pubkey) {
    Generator::default().new()
}

pub fn print_user_key() -> Pubkey {
    let (s,p) = create_key();
    println!("priv key: {:?}",s);
    println!("pub key: {:?}", p);
    p
}

#[test]
fn generatePair() {
    println!("start generatePair test....");
    let (priv_key,pub_key) = Generator::default().new();
    println!("priv_key:{:?},pub_key:{:?}",priv_key,pub_key);
    println!("end generatePair test....");
}
