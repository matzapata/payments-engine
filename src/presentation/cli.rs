use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use crate::application::process_transactions::{AppError, run as process_transactions};

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

fn exit_code_from_error(error: &AppError) -> ExitCode {
    if error.is_broken_pipe() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn execute(input_path: &PathBuf) -> Result<(), AppError> {
    let input = File::open(input_path).map_err(AppError::Io)?;
    // Acquire the stdout lock once for the whole run instead of re-locking on every
    // record write inside csv::Writer.
    let stdout = io::stdout();
    let output = stdout.lock();
    process_transactions(input, output)
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
    fn other_errors_map_to_failure_exit_code() {
        let error = AppError::Io(Error::other("other"));

        assert_eq!(exit_code_from_error(&error), ExitCode::from(1));
    }
}
