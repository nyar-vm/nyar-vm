use clap::Parser;
use std::process::ExitCode;

use nyar_tools::NyarCli;

fn main() -> ExitCode {
    let cli = NyarCli::parse();
    match cli.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::from(1)
        }
    }
}
