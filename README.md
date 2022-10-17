# Thoughts (pre-implementation)

- For authorization purposes, does the client ID of a dispute and a transaction need to match?
- Can a client dispute a chargeback? This would require a deep traversal of the transaction history. Also not possible since disputes, chargebacks and resolves don't have a unique ID and instead use their `tx` value to reference another transaction by ID.
- Concurrency? Could this be handled in a multi-threaded web-app? With access to a shared database?
- What happens when a client disputes a withdrawal?

## Disputing a withdrawal

Based on how an ATM works, if I dispute a withdrawal (fraudulent use of card for instance) then I should be reimbursed the total value of the withdrawal upon resolution. Something like this:

```
deposit 100 : total = 100
withdraw 90 : total = 10
dispute ^   : total = 10,  held = 90, available = 10
resolve ^   : total = 100, held = 00, available = 100
```

Based on the way this has been described in documentation, the above transactions would rather look something like:

```
deposit 100 : total = 100
withdraw 90 : total = 10
dispute ^   : total = 10,  held = 90, available = -80
resolve ^   : total = 10,  held = 00, available = 10
```

I have gone with my intuition here and instead based the logic around how I believe an ATM withdrawal should be disputed and resolved.

# Dependencies

- serde
  - serializing and deserializing transactions from csv lines
- csv
  - parsing csv files into lines
- clap
  - parsing command line arguments

# Assumptions

- To simplify the implementation we assume that deposits, withdrawals, disputes, chargebacks and resolves MUST all happen from the same client. If a dispute, resolve or chargeback is created with a client ID different than that of the referenced transaction then the transaction will fail.
- We can always fail a transaction for a dispute, resolve or chargeback of an unseen transaction even if a valid transaction in the future appears. For example the following MUST always fail:
  - Client 1 makes a dispute (TX ref: #1) // fails
  - Client 1 makes a deposit (TX ref: #1)

# Performance

## Space complexity

- storing past transactions in memory allows for faster lookup at the cost of space complexity
- storing accounts in memory allows for faster writing at the cost of space complexity

## Time complexity

- sacrifices were made in space complexity in order to allow faster lookups (transactions) and writes (accounts)

# Limitations and improvements
- Because we are using the `f64` data type for `transaction.amount` (easier to parse out of the CSV with `serde` than implementing a custom parser for fixed precision from `x.xx` numbers) we can support up to `std::f64::MAX` values for each transaction. Care should be taken to ensure correct arithmetic operations here and given more time **a better implementation would use fixed precision numbers** (eg. `u64`) rather than floating point for improved accuracy. See: https://www.evanjones.ca/floating-point-money.html
- Holding the entire history of transactions in memory presents natural limitations to the amount of transactions the service can process. **A better implementation would use a database to store transaction history.** One which provides good lookup time by transaction ID is important.
- What would the system requirements be to handle every possible transaction (up to `std::u32::MAX`)?
  - Depends on # of accounts created
  - Assume each account has 4 transactions on average (# accounts = # transactions / 4)
  - Back of a napkin maths. Ballpark figures.
    - assume collection types (hashmap, etc.) introduce negligible overhead
    - assume no copy overhead from file
    - assume all transactions are withdrawal and deposit (disputes, resolves and chargebacks are not added to hashmap)
    - 27 bytes per account (struct packed)
    - 16 bytes per transaction (struct packed)
    - ~ 28 gigabytes to store all accounts (4294967295 * 27 / 4)
    - ~ 64 gigabytes to store all transactions (4294967295 * 16)
    - ~ 92 gigabytes to store all data (low end, + ~20%)
    - 128 gigabyte working set should cope
    - assuming ~4 billion transactions, ~1 billion accounts
  - This working set requirement can be almost entirely eliminated using a database but will slow down transaction processing. LRU cache (Redis?) could be a consideration to balance fast in-memory lookups with slower database transactions.
- To better describe invariance in the program it would be good to parse each transaction as it's own data type with associated fields. I couldn't figure out how to quickly do this with `serde` though I'm sure there's a way. Basically, `Transaction` should become `Dispute(associated_data)`. This would mean that we no longer need `Dispute.amount = None` and `Deposit.amount = Some(4)`. Rather no `Dispute` would ever contain an `amount` and all `Deposit` will always contain an `amount`. This is more accurate to the expected invariance and makes better use of the detailed Rust type system.


# Profiling on MacOS using Instruments.app

Whilst writing this I only had access to my Macbook Pro M1 and had to figure out the best way to profile the memory usage of the binary. I did some digging to figure out how to use `Instruments.app` but usually I'd use `valgrind` or something similar.

- See https://github.com/cmyr/cargo-instruments/issues/40
