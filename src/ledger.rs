use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone, Copy)]
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

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Transaction {
    /// Type of transaction. See `TransactionType` for more information.
    #[serde(rename = "type")]
    pub tx_type: TransactionType,

    /// Client ID.
    #[serde(rename = "client")]
    pub client_id: u16,

    /// Transaction ID.
    #[serde(rename = "tx")]
    pub tx_id: u32,

    /// Using an `f64` here is not advised but done for simplicity.
    /// Transaction amounts should be stored with fixed precision to
    /// ensure correct and precise arithmetic operations.
    pub amount: Option<f64>,

    #[serde(skip)]
    pub disputed: bool,
}

impl Transaction {
    pub fn append_to(&mut self, ledger: &mut Ledger) -> Result<(), Box<dyn std::error::Error>> {
        if !ledger.accounts.contains_key(&self.client_id) {
            ledger
                .accounts
                .insert(self.client_id, Account::new(self.client_id));
        }

        let account = ledger.accounts
              .get_mut(&self.client_id)
              .expect("This should never fail since we insert a new Account into the HashMap if one does not exist.");

        if account.is_locked {
            panic!("cannot perform transactions on locked accounts");
        }

        match self.tx_type {
            TransactionType::Deposit => {
                if self.amount.is_none() {
                    panic!("malformed tx");
                }

                let amount = self
                    .amount
                    .expect("Deposit should always contain an amount.");

                account.available_funds += amount;

                ledger.transactions.insert(self.tx_id, *self);
            }
            TransactionType::Withdrawal => {
                if self.amount.is_none() {
                    panic!("malformed tx");
                }

                let amount = self
                    .amount
                    .expect("Withdrawal should always contain an amount.");
                if account.available_funds < amount {
                    panic!("insufficient funds");
                }

                account.available_funds -= amount;

                ledger.transactions.insert(self.tx_id, *self);
            }
            TransactionType::Dispute => {
                if !ledger.transactions.contains_key(&self.tx_id) {
                    panic!("erroneous dispute");
                }

                let mut referenced_tx = ledger
                    .transactions
                    .get_mut(&self.tx_id)
                    .expect("Transaction at this key should always exist at this point.");

                let amount = referenced_tx
                    .amount
                    .expect("Disputed transactions should always contain an amount.");

                referenced_tx.disputed = true;
                account.available_funds -= amount;
                account.held_funds += amount;
            }
            TransactionType::Resolve => {
                if !ledger.transactions.contains_key(&self.tx_id) {
                    panic!("erroneous resolve");
                }

                let referenced_tx = ledger
                    .transactions
                    .get_mut(&self.tx_id)
                    .expect("Transaction at this key should always exist at this point.");

                if !referenced_tx.disputed {
                    panic!("erroneous resolve");
                }

                let amount = referenced_tx
                    .amount
                    .expect("Resolved transactions should always contain an amount.");

                referenced_tx.disputed = false;
                account.available_funds += amount;
                account.held_funds -= amount;
            }
            TransactionType::Chargeback => {
                if !ledger.transactions.contains_key(&self.tx_id) {
                    panic!("erroneous chargeback");
                }

                let referenced_tx = ledger
                    .transactions
                    .get(&self.tx_id)
                    .expect("Transaction at this key should always exist at this point.");

                if !referenced_tx.disputed {
                    panic!("erroneous chargeback");
                }

                let amount = referenced_tx
                    .amount
                    .expect("Chargebacked transactions should always contain an amount.");

                account.is_locked = true;
                account.held_funds -= amount;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct Account {
    /// Client ID.
    #[serde(rename = "client")]
    pub client_id: u16,

    #[serde(rename = "available")]
    pub available_funds: f64,

    #[serde(rename = "held")]
    pub held_funds: f64,

    #[serde(rename = "locked")]
    pub is_locked: bool,
}

impl Account {
    pub fn new(id: u16) -> Self {
        Account {
            client_id: id,
            held_funds: 0.0,
            available_funds: 0.0,
            is_locked: false,
        }
    }
}

#[derive(Debug)]
pub struct Ledger {
    pub transactions: HashMap<u32, Transaction>,
    pub accounts: HashMap<u16, Account>,
}

impl Ledger {
    pub fn new(transactions: HashMap<u32, Transaction>, accounts: HashMap<u16, Account>) -> Self {
        Ledger {
            transactions,
            accounts,
        }
    }
}
