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

#![cfg_attr(feature = "bench", feature(test))]
#[cfg(feature = "bench")]
extern crate test;

#[macro_use]
extern crate clap;

// use std::{str::FromStr, io::{stdin, Read}};
#[allow(unused_imports)]
use hex_literal::hex;
// use clap::load_yaml;
use ed25519::generator::Generator;
use ed25519::pubkey::Pubkey;
use hash;

fn display_address_by_pubkey(pk: Pubkey) {
	let raw = pk.to_bytes();
	let mut addr:[u8; 20] = [0u8; 20];
	addr.copy_from_slice(&(hash::blake2b_256(&raw)[12..]));
    println!("address:{}",hex::encode(&addr));
}
trait Crypto {
	type Generator: Default;

	fn display_new_key_infos() {
		let (s,p) = Generator::default().new();
		println!("Secret key:{},Public key:{}",s,p);
		display_address_by_pubkey(p);
	}
}

struct Ed25519;

impl Crypto for Ed25519 {
	type Generator = ed25519::generator::Generator;
}

fn execute<C: Crypto>(matches: clap::ArgMatches)
{
	// let password = matches.value_of("password");
	match matches.subcommand() {
		("generate", Some(matches)) => {
			C::display_new_key_infos();
		}
		("sign", Some(matches)) => {
			println!("TODO, may be soon");
		}
		("sign-transaction", Some(matches)) => {
			println!("TODO, may be soon");
		}
		("verify", Some(matches)) => {
			println!("TODO, may be soon");
		}
		_ => print_usage(&matches),
	}
}

fn main() {
	let yaml = load_yaml!("cli.yml");
	let matches = clap::App::from_yaml(yaml)
		.version(env!("CARGO_PKG_VERSION"))
		.get_matches();

	if matches.is_present("ed25519") {
		execute::<Ed25519>(matches)
	} else {
		println!("wrong params");
	}
}

fn print_usage(matches: &clap::ArgMatches) {
	println!("{}", matches.usage());
}

#[cfg(test)]
mod tests {
	use super::{Hash, Decode};
	#[test]
	fn should_work() {
		// let s = "0123456789012345678901234567890123456789012345678901234567890123";

		// let d1: Hash = hex::decode(s).ok().and_then(|x| Decode::decode(&mut &x[..])).unwrap();

		// let d2: Hash = {
		// 	let mut gh: [u8; 32] = Default::default();
		// 	gh.copy_from_slice(hex::decode(s).unwrap().as_ref());
		// 	Hash::from(gh)
		// };

		// assert_eq!(d1, d2);
	}
}
