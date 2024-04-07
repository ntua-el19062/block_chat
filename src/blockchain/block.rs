mod block_validator;

pub use block_validator::{BlockValidator, ValidateSemanticsError, ValidateStructureError};

use super::transaction::Transaction;
use crate::crypto::PublicKey;
use hex::ToHex;
use rsa::sha2::{Digest as _, Sha256};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Debug, Formatter},
    time::SystemTime,
};

pub const BLOCK_CAPACITY: usize = 5;

#[derive(Clone, Deserialize, Serialize)]
pub struct Block {
    pub(super) index: u32,
    timestamp: u128,
    #[serde(rename = "transactions")]
    tsxs: Vec<Transaction>,
    #[serde(rename = "validator")]
    val: Option<PublicKey>,
    #[serde(rename = "previous_hash")]
    prev_hash: [u8; 32],
    hash: [u8; 32],
}

impl Block {
    pub fn new(tsxs: [Transaction; BLOCK_CAPACITY], val: PublicKey, prev_hash: [u8; 32]) -> Self {
        let mut blk = Self {
            index: 0,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis(),
            tsxs: tsxs.to_vec(),
            val: Some(val),
            prev_hash,
            hash: [0; 32],
        };

        blk.hash = blk.calculate_hash();

        blk
    }

    pub fn new_genesis(gen_tsxs: Vec<Transaction>) -> Self {
        let mut blk = Self {
            index: 0,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis(),
            tsxs: gen_tsxs,
            val: None,
            prev_hash: [0; 32],
            hash: [0; 32],
        };

        blk.hash = blk.calculate_hash();

        blk
    }

    pub fn calculate_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();

        hasher.update(self.timestamp().to_be_bytes());

        for tsx in self.tsxs() {
            hasher.update(tsx.hash());
        }

        if let Some(v) = self.val() {
            hasher.update(v.to_der());
        }

        hasher.update(self.prev_hash());

        hasher.finalize().into()
    }

    // getters

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn timestamp(&self) -> u128 {
        self.timestamp
    }

    pub fn tsxs(&self) -> &[Transaction] {
        &self.tsxs
    }

    pub fn val(&self) -> Option<&PublicKey> {
        self.val.as_ref()
    }

    pub fn prev_hash(&self) -> &[u8; 32] {
        &self.prev_hash
    }

    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }
}

impl Debug for Block {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Block")
            .field("index", &self.index)
            .field("timestamp", &self.timestamp)
            .field("tsxs", &self.tsxs)
            .field("val", &self.val)
            .field(
                "prev_hash",
                &format_args!("{}", &self.prev_hash.encode_hex::<String>()),
            )
            .field(
                "hash",
                &format_args!("{}", &self.hash.encode_hex::<String>()),
            )
            .finish()
    }
}
