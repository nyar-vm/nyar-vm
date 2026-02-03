use clap::Parser;
use rusty_go::RustyGoFrontend;
use nyar_vm::NyarDriver;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input Rusty Go file
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

    let frontend = RustyGoFrontend::new();
    let driver = NyarDriver::new();

    if args.compile {
        let output = args.output.unwrap_or_else(|| {
            let mut p = args.input.clone();
            p.set_extension("exe");
            p
        });
        println!(
            "正在将 Rusty Go 文件 {:?} 编译为原生程序: {:?}",
            args.input, output
        );
        let vfs = driver.default_vfs();
        let source_uri = args.input.to_string_lossy();
        let output_uri = output.to_string_lossy();
        driver.compile_to_native(&frontend, &vfs, &source_uri, &output_uri)?;
    } else {
        println!("正在运行 Rusty Go 文件: {:?}", args.input);
        let vfs = driver.default_vfs();
        let uri = args.input.to_string_lossy();
        driver.run_source(&frontend, &vfs, &uri)?;
    }

    Ok(())
}
