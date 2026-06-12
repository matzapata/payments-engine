use std::process::ExitCode;

use payments_engine::cli;

fn main() -> ExitCode {
    cli::run()
}
