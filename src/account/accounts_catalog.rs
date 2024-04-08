use super::{Account, AccountError, NoncePool};
use crate::{
    blockchain::{
        block::Block,
        transaction::{Transaction, TransactionPayload},
    },
    crypto::PublicKey,
    peer::PeersCatalog,
};
use std::ops::Deref;

/*
    The AccountsCatalog struct is responsible for managing the accounts of the peers in the network.
    It takes a reference to a PeersCatalog and creates an account for each peer in the catalog.
    The reason it takes a reference is to be able to have many accounts catalogs with the
    same peers catalog thus reserving space.

    The process_transaction() method is used to update the accounts of a catalog
    based on a transaction. Similarly, the process_block() method is used to update
    the accounts of a catalog based on the transactions a block.
*/

#[derive(Debug)]
pub struct AccountsCatalogError {
    pub account_id: u32,
    pub error: AccountError,
}

#[derive(Debug, Clone)]
pub struct AccountsCatalog<'a> {
    accounts: Vec<Account>,
    peers: &'a PeersCatalog,
}

impl<'a> AccountsCatalog<'a> {
    pub fn new(peers: &'a PeersCatalog) -> Self {
        let accounts = peers
            .iter()
            .peers_by_id_asc()
            .map(|p| Account {
                id: p.id(),
                nonce_pool: NoncePool::new(),
                held_cents: 0,
                staked_cents: 0,
            })
            .collect();

        Self { peers, accounts }
    }

    pub fn get_by_id(&self, id: u32) -> Option<&Account> {
        self.accounts.get(id as usize)
    }

    pub fn get_by_id_mut(&mut self, id: u32) -> Option<&mut Account> {
        self.accounts.get_mut(id as usize)
    }

    pub fn get_by_publ_key(&self, publ_key: &PublicKey) -> Option<&Account> {
        self.peers
            .get_by_publ_key(publ_key)
            .and_then(|p| self.get_by_id(p.id()))
    }

    pub fn get_by_publ_key_mut(&mut self, publ_key: &PublicKey) -> Option<&mut Account> {
        self.peers
            .get_by_publ_key(publ_key)
            .and_then(|p| self.get_by_id_mut(p.id()))
    }

    // update the accounts of a catalog based on a transaction
    // leaves the catalog unchanged if an error occurs
    pub fn process_transaction(&mut self, tsx: &Transaction) -> Result<(), AccountsCatalogError> {
        // sender is None in genesis transactions
        if let Some(addr) = tsx.sndr_addr() {
            let sndr = self.get_by_publ_key_mut(addr).unwrap();
            sndr.sub_held(tsx.total_cost())
                .map_err(|e| AccountsCatalogError {
                    account_id: sndr.id,
                    error: e,
                })?;

            if matches!(tsx.payload(), TransactionPayload::Stake(_)) {
                sndr.add_staked(tsx.total_cost() - tsx.fees());
            }

            sndr.nonce_pool_mut().mark_used(tsx.nonce());
        }

        // recipient is None in stake transactions
        if let Some(addr) = tsx.recp_addr() {
            let recp = self.get_by_publ_key_mut(addr).unwrap();
            recp.add_held(tsx.total_cost() - tsx.fees());
        }

        Ok(())
    }

    // update the accounts of a catalog based on the transactions of a block
    // leaves the catalog unchanged if an error occurs
    pub fn process_block(&mut self, blk: &Block) -> Result<(), AccountsCatalogError> {
        let mut self_clone = self.clone();

        for tsx in blk.tsxs() {
            self_clone.process_transaction(tsx)?;

            // validator is None in genesis transactions
            if let Some(v) = blk.val() {
                self_clone
                    .get_by_publ_key_mut(v)
                    .unwrap()
                    .add_held(tsx.fees());
            }
        }

        *self = self_clone;

        Ok(())
    }
}

impl Deref for AccountsCatalog<'_> {
    type Target = Vec<Account>;

    fn deref(&self) -> &Self::Target {
        &self.accounts
    }
}
