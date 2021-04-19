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

//! MAP CLI.
extern crate ctrlc;

use std::path::PathBuf;
use clap::{App, Arg, SubCommand};
use logger::LogConfig;
use service::{Service, NodeConfig};
use std::sync::Arc;
use parking_lot::{Condvar, Mutex};
use std::sync::mpsc;
use ed25519::{privkey::PrivKey, generator};
use network::{Multiaddr};
use map_core::types::Address;

pub fn run() {
    let matches = App::new("map")
        .version("0.0.1")
        .about("MAP Protocol - Chain-to-Chain Interoperation Protocol")
        .arg(Arg::with_name("data_dir")
            .long("datadir")
            .short("d")
            .value_name("PATH")
            .takes_value(true)
            .help("Run as if map was started in <PATH> instead of the current working directory."))
        .arg(Arg::with_name("log")
            .long("log")
            .short("l")
            .value_name("LOG_FILTER")
            .takes_value(true)
            .help("Sets logging filter with <LOG_FILTER>."))
        .arg(Arg::with_name("rpc_addr")
            .long("rpc_addr")
            .takes_value(true)
            .help("Customize RPC listening address"))
        .arg(
            Arg::with_name("rpc_port")
            .long("rpc_port")
            .takes_value(true)
            .default_value("9545")
            .help("Customize RPC listening port"),
        )
        .arg(Arg::with_name("single")
            .long("single")
            .short("s")
            .help("Run with single node"))
        .arg(Arg::with_name("key")
            .long("key")
            .takes_value(true)
            .help("Specify private key"))
        .arg(Arg::with_name("dial_addrs")
            .long("dial_addrs")
            .takes_value(true)
            .help("One or more  multiaddrs to manually connect to a p2p peer")
        )
        .arg(
            Arg::with_name("p2p_port")
                .long("p2p_port")
                .takes_value(true)
                .default_value("40313")
                .help("Customize p2p listening port"),
        )
        .arg(Arg::with_name("seal_block")
            .long("seal")
            .help("Auto generate block"))
        .subcommand(SubCommand::with_name("clean")
            .about("Remove the whole chain data"))
        .subcommand(SubCommand::with_name("keygen")
            .about("Generate key pair"))
        .subcommand(SubCommand::with_name("create_account")
            .about("Generate key pair"))
        .get_matches();

    if let Some(_) = matches.subcommand_matches("keygen") {
        let (priv_key, pub_key) = generator::Generator::default().new();
        println!("priv_key: {:}, pub_key: {:}", priv_key, pub_key);
        return;
    }

    if let Some(_) = matches.subcommand_matches("create_account") {
        let (priv_key, pub_key) = generator::Generator::default().new();
        let addr: Address = pub_key.into();
        println!("priv_key: {:}, address: {:}", priv_key, addr);
        return;
    }

    let mut config = NodeConfig::default();

    if let Some(data_dir) = matches.value_of("data_dir") {
        config.data_dir = PathBuf::from(data_dir);
    }

    if let Some(log_filter) = matches.value_of("log") {
        let log_config = LogConfig {
            filter: log_filter.to_string(),
        };
        config.log = log_filter.to_string();
        logger::init(log_config);
    } else {
        logger::init(LogConfig::default());
    }

    if let Some(rpc_addr) = matches.value_of("rpc_addr") {
        config.rpc_addr = rpc_addr.to_string();
    }
    if let Some(rpc_port) = matches.value_of("rpc_port") {
        let port = rpc_port.parse::<u16>()
            .map_err(|_| format!("Invalid rpc_port port: {}", rpc_port)).unwrap();
        config.rpc_port = port;
    }

    if let Some(p2p_port) = matches.value_of("p2p_port") {
        let port = p2p_port.parse::<u16>()
            .map_err(|_| format!("Invalid p2p_port port: {}", p2p_port)).unwrap();
        config.p2p_port = port;
    }

    if matches.is_present("key") {
        if let Some(key) = matches.value_of("key") {
            if PrivKey::from_hex(key).is_ok() {
                config.key = key.to_string();
            } else {
                println!("Please specify correct key");
                return;
            }
        }
    }
    if matches.is_present("poa_privkey") {
        if let Some(key) = matches.value_of("poa_privkey") {
            if PrivKey::from_hex(key).is_ok() {
                config.poa_privkey = key.to_string();
            } else {
                println!("Please specify correct  poa_privkey");
                // return;
            }
        }
    }
    if matches.is_present("dial_addrs") {
        if let Some(addresses_str) = matches.value_of("dial_addrs") {
            config.dial_addrs = addresses_str
                .split(',')
                .map(|multiaddr| {
                    multiaddr
                        .parse()
                        .map_err(|_| format!("Invalid Multiaddr: {}", multiaddr))
                })
                .collect::<Result<Vec<Multiaddr>, _>>().unwrap();
        }
    }

    if matches.is_present("seal_block") {
        config.seal_block = true;
    }

    if matches.is_present("single") {
        config.dev_mode = true;
        println!("Run map with single node");
    }

    if let Some(_) = matches.subcommand_matches("clean") {
        println!("Remove the whole chain data");
        return;
    }

    let exit = Arc::new((Mutex::new(()), Condvar::new()));
    let node = Service::new_service(config.clone());
    let tx = node.start(config.clone());

    wait_exit(exit,tx);
    println!("Got it! Exiting...");
    // th_handle.join().unwrap();
}

pub fn wait_exit(exit: Arc<(Mutex<()>, Condvar)>, tx : mpsc::Sender<i32>) {
    let e = Arc::<(Mutex<()>, Condvar)>::clone(&exit);
    let _ = ctrlc::set_handler(move || {
        tx.send(1).expect("stop block chain");
        e.1.notify_all();
    });

    // Wait for signal
    let mut l = exit.0.lock();
    exit.1.wait(&mut l);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
