use std::io::{Read, Write};

use thiserror::Error;

use crate::domain::ledger::Ledger;
use crate::infrastructure::csv::{read_transactions, write_accounts};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
}

pub fn run<R: Read, W: Write>(input: R, output: W) -> Result<(), AppError> {
    let mut ledger = Ledger::new();

    for transaction in read_transactions(input)? {
        ledger.apply(&transaction);
    }

    write_accounts(output, ledger.accounts().copied())?;
    Ok(())
}
