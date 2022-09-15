mod ledger;
mod tests;

fn main() {
    let args = &std::env::args().collect::<Vec<String>>()[1];
    let file = std::fs::File::open(args).expect("Failed to read input file.");

    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All) // example file contains space padding
        .from_reader(file);

    let mut ledger = crate::ledger::Ledger::new(
        std::collections::HashMap::new(),
        std::collections::HashMap::new(),
    );

    for transaction in rdr.deserialize::<crate::ledger::Transaction>() {
        transaction
            .expect("Failed to parse transaction.")
            .append_to(&mut ledger)
            .expect("Failed to apply transaction.");
    }

    let mut wtr = csv::WriterBuilder::new().from_writer(std::io::stdout());

    for (_, account) in ledger.accounts {
        wtr.serialize(account)
            .expect("Failed to serialize account.");
    }

    wtr.flush().expect("Failed to write to stdout.");
}
