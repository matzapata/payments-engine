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
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn execute(input_path: &PathBuf) -> Result<(), AppError> {
    let input = File::open(input_path).map_err(AppError::Io)?;
    let output = io::stdout();
    process_transactions(input, output)
}
