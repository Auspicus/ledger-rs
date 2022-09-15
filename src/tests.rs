#[cfg(test)]
mod tests {
    use crate::ledger::Transaction;
    use std::collections::HashMap;

    #[test]
    fn account_balances_should_add_up() {
        let contents = "\
type,client,tx,amount
deposit,1,1,1
deposit,1,3,2
";
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(contents.as_bytes());

        let mut ledger = crate::ledger::Ledger::new(HashMap::new(), HashMap::new());

        for transaction in rdr.deserialize::<Transaction>() {
            transaction.unwrap().append_to(&mut ledger).unwrap();
        }

        assert!(ledger.accounts.values().eq(vec![&crate::ledger::Account {
            client_id: 1,
            available_funds: 3.0,
            held_funds: 0.0,
            is_locked: false,
        },]));
    }

    #[test]
    #[should_panic]
    fn erroneous_disputes_should_panic() {
        let contents = "\
type,client,tx,amount
deposit,1,1,1
deposit,2,2,2
deposit,1,3,2
dispute,1,5,
";
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(contents.as_bytes());

        let mut ledger = crate::ledger::Ledger::new(HashMap::new(), HashMap::new());

        for transaction in rdr.deserialize::<Transaction>() {
            transaction.unwrap().append_to(&mut ledger).unwrap();
        }
    }

    #[test]
    fn valid_dispute_should_hold_funds() {
        let contents = "\
type,client,tx,amount
deposit,1,1,1
deposit,1,3,2
dispute,1,1,
";
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(contents.as_bytes());

        let mut ledger = crate::ledger::Ledger::new(HashMap::new(), HashMap::new());

        for transaction in rdr.deserialize::<Transaction>() {
            transaction.unwrap().append_to(&mut ledger).unwrap();
        }

        assert!(ledger.accounts.values().eq(vec![&crate::ledger::Account {
            client_id: 1,
            available_funds: 2.0,
            held_funds: 1.0,
            is_locked: false,
        }]));
    }

    #[test]
    fn valid_chargeback_should_lock_account() {
        let contents = "\
type,client,tx,amount
deposit,1,1,1
deposit,1,3,2
dispute,1,1,
chargeback,1,1,
";
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(contents.as_bytes());

        let mut ledger = crate::ledger::Ledger::new(HashMap::new(), HashMap::new());

        for transaction in rdr.deserialize::<Transaction>() {
            transaction.unwrap().append_to(&mut ledger).unwrap();
        }

        assert!(ledger.accounts.values().eq(vec![&crate::ledger::Account {
            client_id: 1,
            available_funds: 2.0,
            held_funds: 0.0,
            is_locked: true,
        }]));
    }
}
