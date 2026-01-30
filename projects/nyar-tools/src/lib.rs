use clap::{Parser, Subcommand};
use nyar_error::CliError;

mod cmds;

#[derive(Parser)]
#[command(name = "nyar-vm", version, about = "NYAR VM CLI")]
pub struct NyarCli {
    #[command(subcommand)]
    cmds: NyarCommand,
}

#[derive(Subcommand)]
pub enum NyarCommand {
    Run {
        file: String,
    },
    Dump {
        file: String,
    },
    Repl,
    Bench,
    Compile {
        target: String,
        input: String,
        output: Option<String>,
    },
}

impl NyarCli {
    pub fn run(&self) -> Result<(), CliError> {
        match &self.cmds {
            NyarCommand::Run { file } => cmds::cmd_run::run(file),
            NyarCommand::Dump { file } => cmds::cmd_dump::dump(file),
            NyarCommand::Repl => cmds::cmd_repl::repl(),
            NyarCommand::Bench => cmds::cmd_bench::bench(),
            NyarCommand::Compile {
                target,
                input,
                output,
            } => cmds::cmd_compile::compile(target, input, output.clone()),
        }
    }
}
