use super::Peer;
use crate::crypto::PublicKey;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, slice::Iter};

/*
    The PeersCatalog struct is responsible for managing the peers in the network.
    It exposes an API to access peers by ID or public key,
    but under the hood the peers are kept in a vector sorted by ID to make the access faster.
    To facilitate accessing peers by public key, a hash map is used to map public keys to peer IDs.

    The leak() method can be used to make a PeersCatalog static, which is very useful
    when working with rust threads, which require static lifetimes.
*/

#[derive(Debug)]
pub enum PeersCatalogError {
    DuplicateEntry(Box<(PublicKey, SocketAddr)>),
    NotFound,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PeersCatalog {
    peers: Vec<Peer>,
    index_map: HashMap<PublicKey, usize>,
}

impl PeersCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_capacity(cap: usize) -> Self {
        Self {
            peers: Vec::with_capacity(cap),
            index_map: HashMap::with_capacity(cap),
        }
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    pub fn leak<'a>(self) -> &'a mut Self {
        Box::leak(Box::new(self))
    }

    pub fn iter(&self) -> PeersCatalogIterator {
        PeersCatalogIterator::new(self)
    }

    pub fn insert(
        &mut self,
        (publ_key, sock_addr): (PublicKey, SocketAddr),
    ) -> Result<u32, PeersCatalogError> {
        if self.index_map.contains_key(&publ_key) {
            return Err(PeersCatalogError::DuplicateEntry(Box::new((
                publ_key, sock_addr,
            ))));
        }

        let id = self.peers.len() as u32;
        self.index_map.insert(publ_key.clone(), id as usize);
        self.peers.push(Peer {
            id,
            publ_key,
            sock_addr,
        });

        Ok(id)
    }

    pub fn get_by_id(&self, id: u32) -> Option<&Peer> {
        self.peers.get(id as usize)
    }

    pub fn get_by_publ_key(&self, publ_key: &PublicKey) -> Option<&Peer> {
        self.index_map
            .get(publ_key)
            .and_then(|idx| self.peers.get(*idx))
    }
}

pub struct PeersCatalogIterator<'a> {
    iter: Iter<'a, Peer>,
}

impl<'a> PeersCatalogIterator<'a> {
    fn new(catalog: &'a PeersCatalog) -> Self {
        Self {
            iter: catalog.peers.iter(),
        }
    }

    pub fn peers_by_id_asc(self) -> impl 'a + Iterator<Item = &'a Peer> {
        self.iter
    }

    pub fn peers_by_id_desc(self) -> impl 'a + Iterator<Item = &'a Peer> {
        self.iter.rev()
    }

    pub fn addrs(self) -> impl 'a + Iterator<Item = SocketAddr> {
        self.iter.map(|peer| peer.sock_addr)
    }

    pub fn publ_keys(self) -> impl 'a + Iterator<Item = &'a PublicKey> {
        self.iter.map(|peer| &peer.publ_key)
    }
}

impl<'a> Iterator for PeersCatalogIterator<'a> {
    type Item = &'a Peer;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
