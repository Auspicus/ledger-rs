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
