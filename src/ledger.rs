use std::collections::HashMap;

use crate::{account::Account, transaction::Transaction};
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
