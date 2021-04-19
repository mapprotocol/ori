use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use jsonrpc_core::Result;
use jsonrpc_derive::rpc;
use bincode;
use tokio::sync::mpsc;

use pool::tx_pool::TxPoolManager;
use network::manager::{self, NetworkMessage};
use ed25519::{privkey::PrivKey};
use map_core::transaction::{Transaction, balance_msg};
use map_core::types::Address;

/// AccountManager rpc interface.
#[rpc(server)]
pub trait AccountManager {
    /// Send transaction.
    /// curl -d '{"id": 2, "jsonrpc": "2.0", "method":"map_sendTransaction","params": ["0xd2480451ef35ff2fdd7c69cad058719b9dc4d631","0x0000000000000000000000000000000000000011",100000]}' -H 'content-type:application/json' 'http://localhost:9545'
    #[rpc(name = "map_sendTransaction")]
    fn send_transaction(&self, from: String, to: String, value: u128) -> Result<String>;
}

/// AccountManager rpc implementation.
pub struct AccountManagerImpl {
    tx_pool: Arc<RwLock<TxPoolManager>>,
    accounts: HashMap<Address, PrivKey>,
    network_send: mpsc::UnboundedSender<NetworkMessage>,
}

impl AccountManagerImpl {
    /// Creates new AccountManagerImpl.
    pub fn new(tx_pool: Arc<RwLock<TxPoolManager>>, key: String, network_send: mpsc::UnboundedSender<NetworkMessage>) -> Self {
        let mut accounts = HashMap::new();

        if key != "" {
            let priv_key = PrivKey::from_hex(key.as_str()).expect("private ok");
            let pubkey = priv_key.to_pubkey().expect("pub key ok");
            let address = Address::from(pubkey);
            accounts.insert(address, priv_key);
        }

        AccountManagerImpl {
            tx_pool,
            accounts,
            network_send: network_send,
        }
    }
}

impl AccountManager for AccountManagerImpl {
    fn send_transaction(&self, from: String, to: String, value: u128) -> Result<String> {
        if !is_hex(from.as_str()).is_ok() {
            return Ok(format!("from address is not hex {}", from));
        }
        if !is_hex(to.as_str()).is_ok() {
            return Ok(format!("to address is not hex {}", to));
        }

        let from = match Address::from_hex(&from) {
            Ok(v) => v,
            Err(e) => return Ok(format!("convert address err  {} {}", &from, e))
        };

        let to = match Address::from_hex(&to) {
            Ok(v) => v,
            Err(e) => return Ok(format!("convert address err  {} {}", &to, e))
        };

        let priv_key = match self.accounts.get(&from) {
            Some(v) => v,
            None => return Ok(format!("account no exist {}", from)),
        };

        let nonce = self.tx_pool.read().expect("acquiring tx pool read lock").get_nonce(&from);
        let input: Vec<u8> = bincode::serialize(&balance_msg::MsgTransfer{
            receiver: to,
            value: value}).unwrap();

        let mut tx = Transaction::new(from, nonce + 1, 1000, 1000, b"balance.transfer".to_vec(), input);

        tx.sign(&priv_key.to_bytes()).expect("sign ok");
        if self.tx_pool.write().expect("acquiring tx_pool write_lock").add_tx(tx.clone()) {
            manager::publish_transaction(&mut self.network_send.clone(), tx.clone())
        }
        Ok(format!("{}", tx.hash()))
    }
}

fn is_hex(hex: &str) -> core::result::Result<(), String> {
    let tmp = hex.as_bytes();
    if tmp.len() < 2 {
        Err("Must be a 0x-prefix hex string".to_string())
    } else if tmp.len() & 1 != 0 {
        Err("Hex strings must be of even length".to_string())
    } else if tmp[..2] == b"0x"[..] {
        for byte in &tmp[2..] {
            match byte {
                b'A'..=b'F' | b'a'..=b'f' | b'0'..=b'9' => continue,
                invalid_char => {
                    return Err(format!("Hex has invalid char: {}", invalid_char));
                }
            }
        }
        Ok(())
    } else {
        Err("Must 0x-prefix hex string".to_string())
    }
}

#[cfg(test)]
mod account {
    use super::*;
    use ed25519::{privkey::PrivKey, pubkey::Pubkey};
    use map_core::genesis::{ed_genesis_priv_key, ed_genesis_pub_key};

    #[test]
    fn test_is_hex() {
        {
            let pkey = PrivKey::from_bytes(&ed_genesis_priv_key);
            let pk = Pubkey::from_bytes(&ed_genesis_pub_key);
            let address = Address::from(pk);
            println!("{}", pkey.to_string());
            println!("decode {}", PrivKey::from_hex("0xf9cb7ea173840aeba4fc8146743464cdae3e5527414872155fe331bd2a3454a2").unwrap().to_string());
            assert_eq!("d2480451ef35ff2fdd7c69cad058719b9dc4d631", address.to_string().as_str());
            assert!(is_hex("0xd2480451ef35ff2fdd7c69cad058719b9dc4d631").is_ok())
        }
    }
}
