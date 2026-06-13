//! End-to-end test via `run()` — same orchestration path as `cargo run`, without the CLI.
//!
//! `fixtures/sample_transactions.csv` exercises:
//! - multi-client independent balances
//! - dispute lifecycle (dispute → resolve)
//! - chargeback lock (post-lock rows ignored)
//! - invalid CSV rows (bad type/client/decimals) and domain no-ops

use payments_engine::application::process_transactions::run;
use payments_engine::infrastructure::accounts::CsvAccountWriter;
use payments_engine::infrastructure::transactions::CsvTransactionReader;

#[test]
fn sample_transactions_match_expected_accounts() {
    let input = include_str!("fixtures/sample_transactions.csv");
    let expected = include_str!("fixtures/expected_accounts.csv");

    let mut source = CsvTransactionReader::new(input.as_bytes()).expect("valid csv");
    let mut output = Vec::new();
    let mut sink = CsvAccountWriter::new(&mut output);
    run(&mut source, &mut sink).expect("run succeeds");

    assert_eq!(std::str::from_utf8(&output).unwrap(), expected);
}
