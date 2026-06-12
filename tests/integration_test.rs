//! End-to-end test via `run()` — same orchestration path as `cargo run`, without the CLI.
//!
//! `fixtures/sample_transactions.csv` exercises:
//! - multi-client independent balances
//! - dispute lifecycle (dispute → resolve)
//! - chargeback lock (post-lock rows ignored)
//! - invalid CSV rows (bad type/client/decimals) and domain no-ops

use payments_engine::application::process_transactions::run;

#[test]
fn sample_transactions_match_expected_accounts() {
    let input = include_str!("fixtures/sample_transactions.csv");
    let expected = include_str!("fixtures/expected_accounts.csv");

    let mut output = Vec::new();
    run(input.as_bytes(), &mut output).expect("run succeeds");

    assert_eq!(std::str::from_utf8(&output).unwrap(), expected);
}
