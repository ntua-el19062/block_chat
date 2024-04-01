use super::{Transaction, TransactionPayload};
use crate::account::AccountsCatalog;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidateStructureError {
    #[error("The sender address must be `Some`, but was `None`")]
    MissingSenderAddr,
    #[error("The recipient address must be `Some`, but was `None`")]
    MissingRecipientAddr,
    #[error("The signature must be `Some`, but was `None`")]
    MissingSignature,
    #[error("The sender and recipient addresses are identical")]
    IdenticalSenderRecipientAddrs,
    #[error("The recipient address must be `None`, but was `Some`")]
    UnexpectedRecipientAddr,
    #[error("The calculated hash does not match the provided one")]
    InvalidHash,
    #[error("The signature could not be verified")]
    InvalidSignature,
}

#[derive(Error, Debug)]
pub enum ValidateSemanticsError {
    #[error("The sender does not exist in the accounts catalog")]
    NonExistentSender,
    #[error("The recipient does not exist in the accounts catalog")]
    NonExistentRecipient,
    #[error("The sender's nonce value ({value}) is repeated")]
    RepeatedNonce { value: u64 },
    #[error(
        "The sender does not have enough coins to complete the transaction \
        (sender has {actual}, while {required} are required"
    )]
    InsufficientFunds { required: u32, actual: u32 },
}

pub struct TransactionValidator;

impl TransactionValidator {
    /// Validates whether a transaction is structurally correct.
    pub fn validate_structure(tsx: &Transaction) -> Result<(), ValidateStructureError> {
        use TransactionPayload::*;
        use ValidateStructureError::*;

        if tsx.sndr_addr().is_none() {
            return Err(MissingSenderAddr);
        }

        if matches!(tsx.payload(), Transfer(_) | Message(_)) && tsx.recp_addr().is_none() {
            return Err(MissingRecipientAddr);
        }

        if tsx.sig().is_none() {
            return Err(MissingSignature);
        }

        if matches!(tsx.payload(), Stake(_)) && tsx.recp_addr().is_some() {
            return Err(UnexpectedRecipientAddr);
        }

        if tsx.sndr_addr() == tsx.recp_addr() {
            return Err(IdenticalSenderRecipientAddrs);
        }

        if *tsx.hash() != tsx.calculate_hash() {
            return Err(InvalidHash);
        }

        if !tsx
            .sndr_addr()
            .unwrap()
            .verify(tsx.hash(), tsx.sig().unwrap())
        {
            return Err(InvalidSignature);
        }

        Ok(())
    }

    /// Validates whether a transaction is semantically correct in the given context.
    ///
    /// **Warning**: This function expects a structurally sound transaction.
    pub fn validate_semantics(
        tsx: &Transaction,
        ctx: &AccountsCatalog,
    ) -> Result<(), ValidateSemanticsError> {
        #[cfg(debug_assertions)]
        if let Err(e) = Self::validate_structure(tsx) {
            panic!("Debug assertion failed: {}", e);
        }

        use TransactionPayload::*;
        use ValidateSemanticsError::*;

        let sndr = match ctx.get_by_publ_key(tsx.sndr_addr().unwrap()) {
            Some(sndr) => sndr,
            None => return Err(NonExistentSender),
        };

        if matches!(tsx.payload(), Transfer(_) | Message(_))
            && ctx.get_by_publ_key(tsx.recp_addr().unwrap()).is_none()
        {
            return Err(NonExistentRecipient);
        }

        if tsx.total_cost() > sndr.held_cents() {
            return Err(InsufficientFunds {
                required: tsx.total_cost(),
                actual: sndr.held_cents(),
            });
        }

        if sndr.nonce_pool().is_marked_used(tsx.nonce()) {
            return Err(RepeatedNonce { value: tsx.nonce() });
        }

        Ok(())
    }
}
