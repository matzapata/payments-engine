use crate::domain::ledger::Ledger;
use crate::infrastructure::csv::{CsvAccountWriter, CsvTransactionReader};
use std::io::{ErrorKind, Read, Write};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
}

impl AppError {
    pub fn is_broken_pipe(&self) -> bool {
        match self {
            Self::Io(error) => error.kind() == ErrorKind::BrokenPipe,
            Self::Csv(error) => matches!(
                error.kind(),
                csv::ErrorKind::Io(io_error) if io_error.kind() == ErrorKind::BrokenPipe
            ),
        }
    }
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

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::new(ErrorKind::BrokenPipe, "broken pipe"))
        }
    }

    #[test]
    fn broken_pipe_on_output_write_is_recognized() {
        let input = b"type,client,tx,amount\ndeposit,1,1,1.0\n";
        let error = run(input.as_slice(), BrokenPipeWriter).expect_err("broken pipe fails write");

        assert!(error.is_broken_pipe());
    }

    #[test]
    fn non_broken_pipe_io_error_is_not_recognized() {
        let error = AppError::Io(std::io::Error::other("other"));

        assert!(!error.is_broken_pipe());
    }

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
