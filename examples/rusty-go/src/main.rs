use clap::Parser;
use mini_go::MiniGoFrontend;
use nyar_vm::NyarDriver;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input Mini Go file
    #[arg(required = true)]
    input: PathBuf,

    /// Output path for compiled executable (if compiling)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Compile to native executable instead of running
    #[arg(short, long)]
    compile: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let frontend = MiniGoFrontend::new();
    let driver = NyarDriver::new();

    if args.compile {
        let output = args.output.unwrap_or_else(|| {
            let mut p = args.input.clone();
            p.set_extension("exe");
            p
        });
        println!("正在将 Mini Go 文件 {:?} 编译为原生程序: {:?}", args.input, output);
        driver.compile_to_native(&frontend, &args.input, &output)?;
    } else {
        println!("正在运行 Mini Go 文件: {:?}", args.input);
        driver.run_source(&frontend, &args.input)?;
    }

    Ok(())
}
