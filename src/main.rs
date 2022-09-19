use clap::Parser;

mod account;
mod ledger;
mod transaction;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(index = 1)]
    csv_filename: String,
}

fn main() {
    let args = Args::parse();
    let filename = args.csv_filename;
    let file = std::fs::File::open(filename).expect("Failed to read input file.");

    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) // example file contains space padding
        .flexible(true)
        .from_reader(file);

    let mut ledger = crate::ledger::Ledger::new(
        std::collections::HashMap::new(),
        std::collections::HashMap::new(),
    );

    for transaction in rdr.deserialize::<crate::transaction::Transaction>() {
        // We don't care about the errors here.
        let _ = transaction
            .expect("Failed to parse transaction.")
            .append_to(&mut ledger);
    }

    let mut wtr = csv::WriterBuilder::new().from_writer(std::io::stdout());

    for (_, account) in ledger.accounts {
        wtr.serialize(account)
            .expect("Failed to serialize account.");
    }

    wtr.flush().expect("Failed to write to stdout.");
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::account::Account;
    use crate::ledger::Ledger;
    use crate::transaction::{TransactionError, Transaction, TransactionType};

    fn create_test_ledger(contents: &str) -> Result<Ledger, TransactionError> {
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .flexible(true)
            .from_reader(contents.as_bytes());

        let mut ledger = Ledger::new(HashMap::new(), HashMap::new());

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
deposit,2,5,9
withdrawal,2,6,5
",
        )
        .unwrap();

        assert_eq!(ledger.accounts.get(&1).unwrap(), &Account {
            client_id: 1,
            available_funds: 3.0,
            held_funds: 0.0,
            total_funds: 3.0,
            is_locked: false,
        });

        assert_eq!(ledger.accounts.get(&2).unwrap(), &Account {
            client_id: 2,
            available_funds: 4.0,
            held_funds: 0.0,
            total_funds: 4.0,
            is_locked: false,
        });
    }

    #[test]
    fn disputes_of_unknown_transactions_should_fail() {
        let err = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,1
deposit,2,2,2
deposit,1,3,2
dispute,1,5,
",
        )
        .unwrap_err();

        assert_eq!(err, TransactionError::TransactionNotFound);
    }

    #[test]
    fn valid_disputes_should_hold_funds() {
        let ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,1
deposit,1,3,2
dispute,1,1,
",
        )
        .unwrap();

        assert!(ledger.accounts.values().eq(vec![&Account {
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

        assert!(ledger.accounts.values().eq(vec![&Account {
            client_id: 1,
            available_funds: 2.0,
            held_funds: 0.0,
            total_funds: 2.0,
            is_locked: true,
        }]));
    }

    #[test]
    fn disputes_of_non_matching_client_id_should_fail() {
        let err = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,1
dispute,2,1,
",
        )
        .unwrap_err();

        assert_eq!(err, TransactionError::Unauthorized);
    }

    /// This test checks the case where a user spends and then
    /// attempts to dispute their original deposit. The account
    /// should be locked and further transactions prevented.
    ///
    /// deposits funds (tx#1)
    /// purchases assets (tx#2)
    /// withdraws funds (tx#3)
    /// disputes deposit
    /// resolve dispute
    #[test]
    fn prevent_malicious_actor() {
        let ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,50
withdrawal,1,3,50
dispute,1,1,
chargeback,1,1,
",
        )
        .unwrap();

        assert_eq!(
            ledger.accounts.get(&1).unwrap(),
            &Account {
                client_id: 1,
                held_funds: 0.0,
                available_funds: -100.0,
                total_funds: -100.0,
                is_locked: true,
            }
        );
    }

    /// If an account is locked and then a dispute is made against a
    /// transaction it has made the transaction should not be marked
    /// as disputed.
    #[test]
    fn disputes_of_locked_accounts_should_fail() {
        let err = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,50
dispute,1,2,
chargeback,1,2,
dispute,1,2,
",
        )
        .unwrap_err();

        assert_eq!(err, TransactionError::AccountLocked);
    }

    #[test]
    fn deposits_without_an_amount_should_fail() {
        let err = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,
",
        )
        .unwrap_err();

        assert_eq!(err, TransactionError::Malformed);
    }

    #[test]
    fn withdrawals_without_an_amount_should_fail() {
        let err = create_test_ledger(
            "\
type,client,tx,amount
withdrawal,1,1,
",
        )
        .unwrap_err();

        assert_eq!(err, TransactionError::Malformed);
    }

    #[test]
    fn process_rows_which_omit_final_comma() {
        let ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,90
dispute,1,1
resolve,1,1
dispute,1,1
chargeback,1,1
",
        )
        .unwrap();

        assert_eq!(
            ledger.accounts.get(&1).unwrap(),
            &Account {
                held_funds: 0.0,
                available_funds: -90.0,
                total_funds: -90.0,
                is_locked: true,
                client_id: 1,
            }
        );
    }

    #[test]
    fn withdrawing_more_than_available_should_fail() {
        let err = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,120
",
        )
        .unwrap_err();

        assert_eq!(err, TransactionError::InsufficientFunds);
    }

    /// This is counter-intuitive as the client doesn't have
    /// any available funds to cover their held funds. Total
    /// funds here does still reflect the true amount though.
    #[test]
    fn disputes_of_withdrawal_should_increase_held_funds_but_not_available_funds() {
        let ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,90
dispute,1,2
",
        )
        .unwrap();

        assert_eq!(ledger.accounts.get(&1).unwrap(), &Account {
            client_id: 1,
            available_funds: 10.0,
            held_funds: 90.0,
            total_funds: 100.0,
            is_locked: false,
        });
    }

    #[test]
    fn resolving_a_disputed_withdrawal_restores_balances() {
        let ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,90
dispute,1,2
resolve,1,2
",
        )
        .unwrap();

        assert_eq!(ledger.accounts.get(&1).unwrap(), &Account {
            client_id: 1,
            available_funds: 100.0,
            held_funds: 0.0,
            total_funds: 100.0,
            is_locked: false,
        });
    }


    #[test]
    fn chargeback_on_a_disputed_withdrawal_removes_held_funds() {
        let ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
withdrawal,1,2,90
dispute,1,2
chargeback,1,2
",
        )
        .unwrap();

        assert_eq!(ledger.accounts.get(&1).unwrap(), &Account {
            client_id: 1,
            available_funds: 10.0,
            held_funds: 0.0,
            total_funds: 10.0,
            is_locked: true,
        });
    }

    #[test]
    fn second_transaction_with_duplicate_id_should_fail() {
        let mut ledger = create_test_ledger(
            "\
type,client,tx,amount
deposit,1,1,100
",
        )
        .unwrap();

        let err = Transaction {
            tx_type: TransactionType::Withdrawal,
            tx_id: 1,
            client_id: 1,
            amount: Some(90.0),
            disputed: false,
        }.append_to(&mut ledger).unwrap_err();

        // Rejects adding new transaction.
        assert_eq!(err, TransactionError::DuplicateTransactionID);
        
        // Maintains original transaction.
        assert_eq!(ledger.transactions.get(&1).unwrap(), &Transaction {
            tx_type: TransactionType::Deposit,
            tx_id: 1,
            client_id: 1,
            amount: Some(100.0),
            disputed: false,
        });
    }
}
