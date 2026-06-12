use std::io::{Read, Write};

use thiserror::Error;

use crate::domain::ledger::Ledger;
use crate::infrastructure::csv::{CsvAccountWriter, CsvTransactionReader};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
}

pub fn run<R: Read, W: Write>(input: R, output: W) -> Result<(), AppError> {
    let mut ledger = Ledger::new();
    let mut reader = CsvTransactionReader::new(input)?;

    for transaction in reader.transactions() {
        ledger.apply(&transaction);
    }

    CsvAccountWriter::new(output).write_accounts(ledger.accounts().copied())?;
    Ok(())
}
