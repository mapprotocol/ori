use core::{iter};
use std::{
    net::Ipv4Addr,
    path::{PathBuf},
};
use std::fs::File;
use std::io::prelude::*;

use libp2p::{identity::Keypair};
use libp2p::{multiaddr, multiaddr::Multiaddr};
use slog::{info};

const NODE_KEY_FILENAME: &str = "nodekey";

#[derive(Clone, Debug)]
/// Network configuration for artemis
pub struct Config {
    /// Data directory where node's keyfile is stored
    pub network_dir: PathBuf,

    /// IP address to listen on.
    pub listen_address: Multiaddr,

    /// The TCP port that p2p listens on.
    pub port: u16,

    /// The cli dial addr.
    pub dial_addrs: Vec<Multiaddr>,
}

/// Generates a default Config.
impl Config {
    pub fn new() -> Self {
        Config::default()
    }

    pub fn update_network_cfg(&mut self, data_dir: PathBuf, dial_addrs: Vec<Multiaddr>, p2p_port: u16) -> Result<(), String> {
        // If a `datadir` has been specified, set the network dir to be inside it.
        self.network_dir = data_dir.join("network");
        self.dial_addrs = dial_addrs;
        self.listen_address = iter::once(multiaddr::Protocol::Ip4(Ipv4Addr::new(0, 0, 0, 0)))
            .chain(iter::once(multiaddr::Protocol::Tcp(p2p_port))).collect();
        Ok(())
    }
}

impl Default for Config {
    /// Generate a default network configuration.
    fn default() -> Self {
        let mut network_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        network_dir.push(".map");
        let listen_address = iter::once(multiaddr::Protocol::Ip4(Ipv4Addr::new(0, 0, 0, 0)))
            .chain(iter::once(multiaddr::Protocol::Tcp(40313)))
            .collect();
        Config {
            network_dir,
            port: 40313,
            dial_addrs: vec![],
            listen_address,
        }
    }
}

/// Loads a private key from disk. If this fails, a new key is
/// generated and is then saved to disk.
///
/// Currently only secp256k1 keys are allowed
pub fn load_private_key(config: &Config, log: slog::Logger) -> Keypair {
    // check for key from disk
    let key_file = config.network_dir.join(NODE_KEY_FILENAME);
    if let Ok(mut network_key_file) = File::open(key_file.clone()) {
        let mut key_bytes: Vec<u8> = Vec::with_capacity(36);
        match network_key_file.read_to_end(&mut key_bytes) {
            Ok(_) => {
                // only accept secp256k1 keys for now
                if let Ok(secret_key) =
                libp2p::core::identity::secp256k1::SecretKey::from_bytes(&mut key_bytes)
                {
                    let kp: libp2p::core::identity::secp256k1::Keypair = secret_key.into();
                    return Keypair::Secp256k1(kp);
                } else {
                    info!(log, "Node key file is not a valid secp256k1 key");
                }
            }
            Err(_) => info!(log, "Could not read node key file"),
        }
    }

    // if a key could not be loaded from disk, generate a new one and save it
    let node_private_key = Keypair::generate_secp256k1();
    if let Keypair::Secp256k1(key) = node_private_key.clone() {
        let _ = std::fs::create_dir_all(&config.network_dir);
        match File::create(key_file.clone())
            .and_then(|mut f| f.write_all(&key.secret().to_bytes()))
        {
            Ok(_) => {
                info!(log, "New node key generated and written to disk");
            }
            Err(e) => {
                info!(log, "Could not write node key to file: {:?}. Error: {}", key_file, e);
            }
        }
    }
    node_private_key
}
