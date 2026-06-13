use crate::application::AppError;
use crate::application::process_transactions::run as process_transactions;
use crate::infrastructure::accounts::CsvAccountWriter;
use crate::infrastructure::transactions::CsvTransactionReader;
use clap::Parser;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "payments-engine",
    version,
    about = "Process transactions CSV and emit account snapshot"
)]
struct Args {
    /// Path to the transactions CSV file
    input: PathBuf,
}

pub fn run() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let _ = error.print();
            return ExitCode::from(2);
        }
    };

    match execute(&args.input) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let exit_code = exit_code_from_error(&error);
            if exit_code != ExitCode::SUCCESS {
                eprintln!("{error}");
            }
            exit_code
        }
    }
}

fn is_broken_pipe(error: &AppError) -> bool {
    match error {
        AppError::Io(error) => error.kind() == io::ErrorKind::BrokenPipe,
        AppError::Csv(error) => matches!(
            error.kind(),
            csv::ErrorKind::Io(io_error) if io_error.kind() == io::ErrorKind::BrokenPipe
        ),
    }
}

fn exit_code_from_error(error: &AppError) -> ExitCode {
    if is_broken_pipe(error) { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn execute(input_path: &PathBuf) -> Result<(), AppError> {
    let input = File::open(input_path).map_err(AppError::Io)?;
    // Acquire the stdout lock once for the whole run instead of re-locking on every
    // record write inside csv::Writer.
    let stdout = io::stdout();
    let output = stdout.lock();
    let mut source = CsvTransactionReader::new(input)?;
    let mut sink = CsvAccountWriter::new(output);
    process_transactions(&mut source, &mut sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[test]
    fn broken_pipe_maps_to_success_exit_code() {
        let error = AppError::Io(Error::new(ErrorKind::BrokenPipe, "broken pipe"));

        assert_eq!(exit_code_from_error(&error), ExitCode::SUCCESS);
    }

    #[test]
    fn csv_wrapped_broken_pipe_maps_to_success_exit_code() {
        let io_error = Error::new(ErrorKind::BrokenPipe, "broken pipe");
        let error = AppError::Csv(csv::Error::from(io_error));

        assert_eq!(exit_code_from_error(&error), ExitCode::SUCCESS);
    }

    #[test]
    fn other_errors_map_to_failure_exit_code() {
        let error = AppError::Io(Error::other("other"));

        assert_eq!(exit_code_from_error(&error), ExitCode::from(1));
    }
}
