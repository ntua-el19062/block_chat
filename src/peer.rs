mod peers_catalog;

pub use peers_catalog::{PeersCatalog, PeersCatalogError};

use crate::crypto::PublicKey;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

// A Peer is just a struct that holds an ID, a public key, and a socket address.

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Peer {
    id: u32,
    publ_key: PublicKey,
    sock_addr: SocketAddr,
}

impl Peer {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn publ_key(&self) -> &PublicKey {
        &self.publ_key
    }

    pub fn sock_addr(&self) -> SocketAddr {
        self.sock_addr
    }
}
