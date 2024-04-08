mod transaction_validator;

pub use transaction_validator::{
    TransactionValidator, ValidateSemanticsError, ValidateStructureError,
};

use crate::crypto::{PrivateKey, PublicKey};
use crate::protocol::{
    CENTS_PER_COIN, MESSAGE_FEE_PER_CHARACTER_CENTS, MINIMUM_TRANSFER_FEE_CENTS,
    TRANSFER_FEE_PERCENTAGE,
};
use hex::ToHex;
use non_empty_string::NonEmptyString;
use rsa::sha2::{Digest as _, Sha256};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Debug, Formatter},
    num::NonZeroU32,
};

#[derive(Clone, Deserialize, Serialize)]
pub enum TransactionPayload {
    Transfer(NonZeroU32),
    Message(NonEmptyString),
    Stake(NonZeroU32),
}

impl TransactionPayload {
    pub fn coins(&self) -> Option<u32> {
        match self {
            Self::Stake(coins) => Some(coins.get()),
            Self::Transfer(coins) => Some(coins.get()),
            Self::Message(_) => None,
        }
    }

    pub fn message(&self) -> Option<&str> {
        if let Self::Message(msg) = self {
            return Some(msg.as_str());
        }

        None
    }
}

impl Debug for TransactionPayload {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transfer(amnt) => f
                .debug_tuple("Transfer")
                .field(&(amnt.get() as f64 / CENTS_PER_COIN as f64))
                .finish(),
            Self::Message(msg) => f.debug_tuple("Message").field(msg).finish(),
            Self::Stake(amnt) => f
                .debug_tuple("Stake")
                .field(&(amnt.get() as f64 / CENTS_PER_COIN as f64))
                .finish(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Transaction {
    payload: TransactionPayload,
    #[serde(rename = "sender_address")]
    sndr_addr: Option<PublicKey>,
    #[serde(rename = "recipient_address")]
    recp_addr: Option<PublicKey>,
    nonce: u64,
    hash: [u8; 32],
    #[serde(rename = "signature")]
    sig: Option<Vec<u8>>,
}

impl Transaction {
    pub fn new_genesis(sndr_addr: PublicKey, amnt: NonZeroU32) -> Self {
        Self::new(
            TransactionPayload::Transfer(amnt),
            None,
            Some(sndr_addr),
            0,
            None,
        )
    }

    pub fn new_transfer(
        sndr_addr: PublicKey,
        recp_addr: PublicKey,
        amnt: NonZeroU32,
        nonce: u64,
        priv_key: &PrivateKey,
    ) -> Self {
        Self::new(
            TransactionPayload::Transfer(amnt),
            Some(sndr_addr),
            Some(recp_addr),
            nonce,
            Some(priv_key),
        )
    }

    pub fn new_message(
        sndr_addr: PublicKey,
        recp_addr: PublicKey,
        msg: NonEmptyString,
        nonce: u64,
        priv_key: &PrivateKey,
    ) -> Self {
        Self::new(
            TransactionPayload::Message(msg),
            Some(sndr_addr),
            Some(recp_addr),
            nonce,
            Some(priv_key),
        )
    }

    pub fn new_stake(
        sndr_addr: PublicKey,
        amnt: NonZeroU32,
        nonce: u64,
        priv_key: &PrivateKey,
    ) -> Self {
        Self::new(
            TransactionPayload::Stake(amnt),
            Some(sndr_addr),
            None,
            nonce,
            Some(priv_key),
        )
    }

    pub fn fees(&self) -> u32 {
        match self.payload() {
            TransactionPayload::Transfer(amnt) => Self::calculate_transfer_fees(*amnt),
            TransactionPayload::Message(msg) => Self::calculate_message_fees(msg),
            TransactionPayload::Stake(amnt) => Self::calculcate_stake_fees(*amnt),
        }
    }

    // fees + amount where applicable
    pub fn total_cost(&self) -> u32 {
        match self.payload() {
            TransactionPayload::Transfer(amnt) => Self::calculate_transfer_total_cost(*amnt),
            TransactionPayload::Message(msg) => Self::calculate_message_total_cost(msg),
            TransactionPayload::Stake(amnt) => Self::calculate_stake_total_cost(*amnt),
        }
    }

    pub fn calculate_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();

        if let Some(c) = self.payload().coins() {
            hasher.update(c.to_be_bytes())
        }

        if let Some(m) = self.payload().message() {
            hasher.update(m.as_bytes())
        }

        if let Some(a) = self.recp_addr() {
            hasher.update(a.to_der());
        }

        if let Some(a) = self.sndr_addr() {
            hasher.update(a.to_der());
        }

        hasher.update(self.nonce().to_be_bytes());

        hasher.finalize().into()
    }

    pub fn calculate_transfer_fees(amnt: NonZeroU32) -> u32 {
        let fee = amnt.get() * TRANSFER_FEE_PERCENTAGE / 100;

        if fee < MINIMUM_TRANSFER_FEE_CENTS {
            MINIMUM_TRANSFER_FEE_CENTS
        } else {
            fee
        }
    }

    pub fn calculate_message_fees(msg: &NonEmptyString) -> u32 {
        msg.len() as u32 * MESSAGE_FEE_PER_CHARACTER_CENTS
    }

    pub fn calculcate_stake_fees(_amnt: NonZeroU32) -> u32 {
        0
    }

    pub fn calculate_transfer_total_cost(amnt: NonZeroU32) -> u32 {
        amnt.get() + Self::calculate_transfer_fees(amnt)
    }

    pub fn calculate_message_total_cost(msg: &NonEmptyString) -> u32 {
        msg.len() as u32 + Self::calculate_message_fees(msg)
    }

    pub fn calculate_stake_total_cost(amnt: NonZeroU32) -> u32 {
        amnt.get()
    }

    fn new(
        payload: TransactionPayload,
        sndr_addr: Option<PublicKey>,
        recp_addr: Option<PublicKey>,
        nonce: u64,
        priv_key: Option<&PrivateKey>,
    ) -> Self {
        let mut tsx = Self {
            payload,
            sndr_addr,
            recp_addr,
            nonce,
            hash: [0; 32],
            sig: None,
        };

        tsx.hash = tsx.calculate_hash();
        if let Some(key) = priv_key {
            tsx.sig = Some(key.sign(tsx.hash()));
        }

        tsx
    }

    // getters

    pub fn payload(&self) -> &TransactionPayload {
        &self.payload
    }

    pub fn sndr_addr(&self) -> Option<&PublicKey> {
        self.sndr_addr.as_ref()
    }

    pub fn recp_addr(&self) -> Option<&PublicKey> {
        self.recp_addr.as_ref()
    }

    pub fn nonce(&self) -> u64 {
        self.nonce
    }

    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }

    pub fn sig(&self) -> Option<&[u8]> {
        self.sig.as_deref()
    }
}

impl Debug for Transaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transaction")
            .field("payload", &self.payload)
            .field("sndr_addr", &self.sndr_addr)
            .field("recp_addr", &self.recp_addr)
            .field("nonce", &self.nonce)
            .field("hash", &self.hash.encode_hex::<String>())
            .field(
                "sig",
                &self.sig.as_ref().map(|s| (&s[..8]).encode_hex::<String>()),
            )
            .finish()
    }
}
