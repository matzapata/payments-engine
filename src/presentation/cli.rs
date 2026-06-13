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
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
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
