use super::{Block, BLOCK_CAPACITY};
use crate::{
    account::AccountsCatalog,
    blockchain::{
        transaction::{self, TransactionValidator},
        Blockchain,
    },
};
use std::cmp::Ordering;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidateStructureError {
    #[error(
        "The block has {0} transactions less than the required {}",
        BLOCK_CAPACITY
    )]
    PartiallyFilledBlock(usize),
    #[error(
        "The block has {0} transactions more than the required {}",
        BLOCK_CAPACITY
    )]
    OverfilledBlock(usize),
    #[error("The validator should be `Some` but is `None`")]
    MissingValidator,
    #[error("The timestamp contains a future date")]
    InvalidTimestamp,
    #[error("The transaction at index {index} is invalid: {source}")]
    InvalidTransaction {
        index: usize,
        source: transaction::ValidateStructureError,
    },
    #[error("The calculated hash does not match the provided one")]
    InvalidHash,
}

#[derive(Error, Debug)]
pub enum ValidateSemanticsError {
    #[error("The validator does not exist in the accounts catalog")]
    NonExistentValidator,
    #[error(
        "The validator ID does not match the predicted one (expected {expected}, found {actual})"
    )]
    MismatchedValidator { expected: u32, actual: u32 },
    #[error("The transaction at index {index} is invalid: {source}")]
    InvalidTransaction {
        index: usize,
        source: transaction::ValidateSemanticsError,
    },
    #[error("The previous hash does not match the blockchain's last block's hash")]
    InvalidPreviousHash,
}

pub struct BlockValidator;

impl BlockValidator {
    /// Validates whether a block is structurally correct.
    pub fn validate_structure(blk: &Block) -> Result<(), ValidateStructureError> {
        use ValidateStructureError::*;

        let diff = blk.tsxs().len().abs_diff(BLOCK_CAPACITY);
        match blk.tsxs().len().cmp(&BLOCK_CAPACITY) {
            Ordering::Less => return Err(PartiallyFilledBlock(diff)),
            Ordering::Greater => return Err(OverfilledBlock(diff)),
            Ordering::Equal => (),
        }

        blk.tsxs().iter().enumerate().try_for_each(|(index, tsx)| {
            TransactionValidator::validate_structure(tsx)
                .map_err(|source| ValidateStructureError::InvalidTransaction { index, source })
        })?;

        if *blk.hash() != blk.calculate_hash() {
            return Err(InvalidHash);
        }

        Ok(())
    }

    /// Validates whether a block is semantically correct in the given context.
    ///
    /// **Warning**: This function expects a structurally correct block.
    pub fn validate_semantics(
        blk: &Block,
        pred_val_id: u32,
        ctx: (&AccountsCatalog, &Blockchain),
    ) -> Result<(), ValidateSemanticsError> {
        #[cfg(debug_assertions)]
        if let Err(e) = Self::validate_structure(blk) {
            panic!("Debug assertion failed: {}", e);
        }

        use ValidateSemanticsError::*;

        if let Some(account) = ctx.0.get_by_publ_key(blk.val().unwrap()) {
            if account.id() != pred_val_id {
                return Err(MismatchedValidator {
                    expected: pred_val_id,
                    actual: account.id(),
                });
            }
        } else {
            return Err(NonExistentValidator);
        }

        blk.tsxs().iter().enumerate().try_for_each(|(index, tsx)| {
            TransactionValidator::validate_semantics(tsx, ctx.0)
                .map_err(|source| ValidateSemanticsError::InvalidTransaction { index, source })
        })?;

        if *blk.prev_hash() != *ctx.1.last_block().hash() {
            return Err(InvalidPreviousHash);
        }

        Ok(())
    }
}
