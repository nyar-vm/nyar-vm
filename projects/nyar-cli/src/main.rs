use clap::Parser;
use std::process::ExitCode;

use nyar_cli::{CliError, NyarCli};

#[tokio::main]
fn main() -> Result<(), CliError> {
    let cli = NyarCli::parse();
    cli.run()
}
