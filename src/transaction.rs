use std::{collections::HashMap, error::Error, fmt::Display};

use serde::Deserialize;

use crate::{account::Account, ledger::Ledger};

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum TransactionType {
    /// A deposit is a credit to the client's asset account,
    /// meaning it should increase the available and
    /// total funds of the client account.
    ///
    /// A deposit looks like
    ///
    /// |type       |client |id     |amount |
    /// |-----------|-------|-------|-------|
    /// |deposit    |1      |1      |1.0    |
    Deposit,

    /// A withdraw is a debit to the client's asset account,
    /// meaning it should decrease the available and
    /// total funds of the client account
    ///
    /// A withdrawal looks like
    ///
    /// |type       |client |id     |amount |
    /// |-----------|-------|-------|-------|
    /// |withdrawal |2      |2      |1.0    |
    Withdrawal,

    /// A dispute represents a client's claim that
    /// a transaction was erroneous and should be reversed.
    /// The transaction shouldn't be reversed yet but the
    /// associated funds should be held. This means that the
    /// clients available funds should decrease by the amount
    /// disputed, their held funds should increase by the amount
    /// disputed, while their total funds should remain the same.
    ///
    /// A dispute looks like
    ///
    /// |type       |client |id     |amount |
    /// |-----------|-------|-------|-------|
    /// |dispute    |1      |1      |       |
    ///
    /// Notice that a dispute does not state the amount disputed.
    /// Instead a dispute references the transaction that is
    /// disputed by ID. If the tx specified by the dispute doesn't
    /// exist you can ignore it and assume this is an error on our
    /// partners side.
    Dispute,

    /// A resolve represents a resolution to a dispute, releasing
    /// the associated held funds. Funds that were previously
    /// disputed are no longer disputed. This means that the
    /// clients held funds should decrease by the amount no
    /// longer disputed, their available funds should increase
    /// by the amount no longer disputed, and their total funds
    /// should remain the same.
    ///
    /// A resolve looks like
    ///
    /// |type       |client |id     |amount |
    /// |-----------|-------|-------|-------|
    /// |resolve    |1      |1      |       |
    ///
    /// Like disputes, resolves do not specify an amount.
    /// Instead they refer to a transaction that was under dispute
    /// by ID. If the tx specified doesn't exist, or the tx isn't
    /// under dispute, you can ignore the resolve and assume this
    /// is an error on our partner's side.
    Resolve,

    /// A chargeback is the final state of a dispute and represents
    /// the client reversing a transaction. Funds that were held have
    /// now been withdrawn. This means that the clients held funds and
    /// total funds should decrease by the amount previously disputed.
    /// If a chargeback occurs the client's account should be immediately
    /// frozen.
    ///
    /// A chargeback looks like
    ///
    /// |type       |client |id     |amount |
    /// |-----------|-------|-------|-------|
    /// |chargeback |1      |1      |       |
    ///
    /// Like a dispute and a resolve a chargeback refers to the transaction
    /// by ID (tx) and does not specify an amount. Like a resolve, if the
    /// tx specified doesn't exist, or the tx isn't under dispute, you can
    /// ignore chargeback and assume this is an error on our partner's side.
    Chargeback,
}

#[non_exhaustive]
#[derive(Debug, PartialEq)]
pub enum TransactionError {
    /// Transaction contains invalid data.
    Malformed,

    /// Two transactions with the same ID have been processed.
    DuplicateTransactionID,

    /// Transaction could not be processed due to the client having insufficient funds.
    InsufficientFunds,

    /// Transaction references another transaction that could not be found.
    /// This may be due to network issues or improper ordering.
    TransactionNotFound,

    /// Transaction attempts to resolve or chargeback a transaction that was not disputed.
    NotDisputed,

    /// Transaction attempts to dispute a transaction which is already disputed.
    AlreadyDisputed,

    /// Transaction attempts to dispute a chargeback or resolve.
    /// Only withdrawals and deposits can be disputed.
    Indisputable,

    /// Transaction could not be made since it refers to an account that is locked.
    AccountLocked,

    /// Transaction attempts to reference a transaction created by
    /// a different client.
    Unauthorized,
}

impl Error for TransactionError {}
impl Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

// 16 bytes
#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
pub struct Transaction {
    /// Type of transaction. See `TransactionType` for more information.
    #[serde(rename = "type")]
    pub tx_type: TransactionType, // 1 byte

    /// Client ID.
    #[serde(rename = "client")]
    pub client_id: u16, // 2 bytes

    /// Transaction ID.
    #[serde(rename = "tx")]
    pub tx_id: u32, // 4 bytes

    /// Using an `f64` here is not advised but done for simplicity.
    /// Transaction amounts should be stored with fixed precision to
    /// ensure correct and precise arithmetic operations.
    pub amount: Option<f64>, // 8 bytes

    #[serde(skip)]
    pub disputed: bool, // 1 byte
}

impl Transaction {
    fn is_disputed(&mut self) -> Result<(), TransactionError> {
        if !self.disputed {
            Err(TransactionError::NotDisputed)
        } else {
            Ok(())
        }
    }

    fn is_not_disputed(&mut self) -> Result<(), TransactionError> {
        if self.disputed {
            Err(TransactionError::AlreadyDisputed)
        } else {
            Ok(())
        }
    }

    fn get_amount(&self) -> Result<f64, TransactionError> {
        self.amount.ok_or(TransactionError::Malformed)
    }

    fn get_account<'a>(
        &self,
        accounts: &'a mut HashMap<u16, Account>,
    ) -> Result<&'a mut Account, TransactionError> {
        let account = accounts
            .entry(self.client_id)
            .or_insert_with(|| Account::new(self.client_id));

        if account.is_locked {
            return Err(TransactionError::AccountLocked);
        }

        Ok(account)
    }

    fn get_referenced_tx<'a>(
        &self,
        transactions: &'a mut HashMap<u32, Transaction>,
    ) -> Result<&'a mut Transaction, TransactionError> {
        let referenced_tx = transactions
            .get_mut(&self.tx_id)
            .ok_or(TransactionError::TransactionNotFound)?;

        if self.client_id != referenced_tx.client_id {
            return Err(TransactionError::Unauthorized);
        }

        // This is unnecessary in the current implementation
        // because we only store deposit and withdrawal transactions
        // but if a database was implemented and all transactions
        // were to be stored then this check is required.
        match referenced_tx.tx_type {
            TransactionType::Deposit | TransactionType::Withdrawal => {}
            _ => {
                return Err(TransactionError::Indisputable);
            }
        }

        Ok(referenced_tx)
    }

    /// Appends a transaction to the ledger.
    /// Applies balance mutations to the accounts.
    /// Creates accounts where necessary.
    pub fn append_to(&self, ledger: &mut Ledger) -> Result<(), TransactionError> {
        match self.tx_type {
            TransactionType::Deposit | TransactionType::Withdrawal => {
                // Keep track of this transaction in case there are disputes.
                // Since we only track deposits and withdrawals we don't need
                // to check the type of transaction that is disputed since
                // we will only find those transaction types from a lookup.
                if let Some(old) = ledger.transactions.insert(self.tx_id, *self) {
                    // `try_insert` could be used here but
                    // isn't available in stable Rust.
                    // Put back the old record.
                    ledger.transactions.insert(self.tx_id, old);

                    // Don't process the duplicate transaction,
                    // instead bail with an error.
                    return Err(TransactionError::DuplicateTransactionID);
                }
            }
            _ => {}
        }

        match self.tx_type {
            TransactionType::Deposit => {
                let amount = self.get_amount()?;
                let account = self.get_account(&mut ledger.accounts)?;

                account.available_funds += amount;
                account.total_funds = account.available_funds + account.held_funds;
            }
            TransactionType::Withdrawal => {
                let amount = self.get_amount()?;
                let account = self.get_account(&mut ledger.accounts)?;

                if amount > account.available_funds {
                    return Err(TransactionError::InsufficientFunds);
                }

                account.available_funds -= amount;
                account.total_funds = account.available_funds + account.held_funds;
            }
            TransactionType::Dispute => {
                let account = self.get_account(&mut ledger.accounts)?;
                let referenced_tx = self.get_referenced_tx(&mut ledger.transactions)?;
                let amount = referenced_tx.get_amount()?;
                referenced_tx.is_not_disputed()?;

                referenced_tx.disputed = true;

                if referenced_tx.tx_type == TransactionType::Deposit {
                    account.available_funds -= amount;
                }

                account.held_funds += amount;
                account.total_funds = account.available_funds + account.held_funds;
            }
            TransactionType::Resolve => {
                let account = self.get_account(&mut ledger.accounts)?;
                let referenced_tx = self.get_referenced_tx(&mut ledger.transactions)?;
                let amount = referenced_tx.get_amount()?;
                referenced_tx.is_disputed()?;

                referenced_tx.disputed = false;
                account.available_funds += amount;
                account.held_funds -= amount;
                account.total_funds = account.available_funds + account.held_funds;
            }
            TransactionType::Chargeback => {
                let account = self.get_account(&mut ledger.accounts)?;
                let referenced_tx = self.get_referenced_tx(&mut ledger.transactions)?;
                let amount = referenced_tx.get_amount()?;
                referenced_tx.is_disputed()?;

                referenced_tx.disputed = false;
                account.is_locked = true;
                account.held_funds -= amount;
                account.total_funds = account.available_funds + account.held_funds;
            }
        }

        Ok(())
    }
}
