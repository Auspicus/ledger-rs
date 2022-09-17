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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::transaction::TransactionError;

    fn create_test_ledger(contents: &str) -> Result<crate::ledger::Ledger, TransactionError> {
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(contents.as_bytes());

        let mut ledger = crate::ledger::Ledger::new(HashMap::new(), HashMap::new());

        for transaction in rdr.deserialize::<crate::transaction::Transaction>() {
            transaction.unwrap().append_to(&mut ledger)?;
        }

        Ok(ledger)
    }

    #[test]
    fn account_balances_should_add_up() {
        let ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,1
deposit,1,3,2
",
        )
        .unwrap();

        assert!(ledger.accounts.values().eq(vec![&crate::ledger::Account {
            client_id: 1,
            available_funds: 3.0,
            held_funds: 0.0,
            total_funds: 3.0,
            is_locked: false,
        },]));
    }

    #[test]
    #[should_panic]
    fn erroneous_disputes_should_panic() {
        create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,1
deposit,2,2,2
deposit,1,3,2
dispute,1,5,
",
        )
        .unwrap();
    }

    #[test]
    fn valid_dispute_should_hold_funds() {
        let ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,1
deposit,1,3,2
dispute,1,1,
",
        )
        .unwrap();

        assert!(ledger.accounts.values().eq(vec![&crate::ledger::Account {
            client_id: 1,
            available_funds: 2.0,
            held_funds: 1.0,
            total_funds: 3.0,
            is_locked: false,
        }]));
    }

    #[test]
    fn valid_chargeback_should_lock_account() {
        let ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,1
deposit,1,3,2
dispute,1,1,
chargeback,1,1,
",
        )
        .unwrap();

        assert!(ledger.accounts.values().eq(vec![&crate::ledger::Account {
            client_id: 1,
            available_funds: 2.0,
            held_funds: 0.0,
            total_funds: 2.0,
            is_locked: true,
        }]));
    }

    #[test]
    fn prevents_unauthorized_transactions() {
        if let Err(err) = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,1
dispute,2,1,
",
        ) {
            assert_eq!(err, TransactionError::Unauthorized);
        } else {
            unreachable!();
        }
    }

    /// This test checks the case where a user spends and then
    /// attempts to dispute their original deposit. This is prevented
    /// because withdrawals leaving the account in a negative balance
    /// are not possible.
    ///
    /// deposits funds (tx#1)
    /// purchases assets (tx#2)
    /// withdraws funds (tx#3)
    /// disputes deposit
    /// resolve dispute
    #[test]
    fn prevents_malicious_actor() {
        if let Ok(ledger) = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,50
withdrawal,1,3,50
dispute,1,1,
resolve,1,1,
",
        ) {
            assert!(ledger.accounts.values().eq(vec![&crate::ledger::Account {
                client_id: 1,
                held_funds: 0.0,
                available_funds: 0.0,
                total_funds: 0.0,
                is_locked: false,
            }]));
        }
    }

    /// If an account is locked and then a dispute is made against a
    /// transaction it has made the transaction should not be marked
    /// as disputed.
    #[test]
    fn prevent_transactions_from_being_stuck_in_disputed() {
        if let Ok(ledger) = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,50
dispute,1,2,
chargeback,1,2,
",
        ) { 
            assert_eq!(ledger.transactions.get(&2).unwrap(), &crate::transaction::Transaction {
                tx_id: 2,
                tx_type: crate::transaction::TransactionType::Withdrawal,
                amount: Some(50.0),
                client_id: 1,
                disputed: false,
            });
        }
    }
}
