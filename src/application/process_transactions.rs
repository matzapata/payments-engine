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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_processes_in_memory_csv() {
        let input = b"type,client,tx,amount\n\
                      deposit,1,1,10.0000\n\
                      withdrawal,1,2,3.5000\n\
                      dispute,2,10,\n";
        let mut output = Vec::new();

        run(input.as_slice(), &mut output).expect("run succeeds");

        let expected = "client,available,held,total,locked\n\
                        1,6.5000,0.0000,6.5000,false\n";
        assert_eq!(std::str::from_utf8(&output).unwrap(), expected);
    }
}
