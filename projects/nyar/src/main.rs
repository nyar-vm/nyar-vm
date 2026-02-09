use nyar_types::{NyarError, NyarFrontend};
use nyar_vm::NyarDriver;
use oak_vfs::DiskVfs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "nyar")]
#[command(about = "Nyar Virtual Machine CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Input file to run or compile
    #[arg(value_name = "FILE")]
    input: Option<PathBuf>,

    /// Output file for compilation
    #[arg(short, long, value_name = "OUT")]
    output: Option<PathBuf>,

    /// Compile the input file instead of running it
    #[arg(short, long)]
    compile: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a script or program
    Run {
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// Compile a script to native binary
    Compile {
        #[arg(value_name = "FILE")]
        file: PathBuf,
        #[arg(short, long, value_name = "OUT")]
        output: Option<PathBuf>,
    },
    /// Start a REPL for a specific language
    Repl {
        #[arg(short, long)]
        lang: String,
    },
}

fn main() -> Result<(), NyarError> {
    let cli = Cli::parse();

    let driver = NyarDriver::new();
    let vfs = DiskVfs::new();

    if let Some(command) = cli.command {
        match command {
            Commands::Run { file } => run_file(&driver, &vfs, &file)?,
            Commands::Compile { file, output } => compile_file(&driver, &vfs, &file, output)?,
            Commands::Repl { lang } => start_repl(&driver, &vfs, &lang)?,
        }
    } else if let Some(input) = cli.input {
        if cli.compile {
            compile_file(&driver, &vfs, &input, cli.output)?;
        } else {
            run_file(&driver, &vfs, &input)?;
        }
    } else {
        println!("Nyar VM CLI. Use --help for usage.");
    }

    Ok(())
}

fn run_file(driver: &NyarDriver, vfs: &DiskVfs, path: &Path) -> Result<(), NyarError> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let input_uri = path.to_str().ok_or_else(|| NyarError::RuntimeError("Invalid input path".to_string()))?;

    match ext {
        "c" => driver.run_source(&rusty_c::RustyCFrontend::new(), vfs, input_uri)?,
        "py" => driver.run_source(&rusty_python::RustyPythonFrontend::new(), vfs, input_uri)?,
        "java" => driver.run_source(&rusty_java::MiniJavaFrontend::default(), vfs, input_uri)?,
        "ts" | "js" => driver.run_source(&rusty_typescript::RustyTypescriptFrontend::new(), vfs, input_uri)?,
        "go" => driver.run_source(&rusty_go::RustyGoFrontend::new(), vfs, input_uri)?,
        "cs" => driver.run_source(&rusty_csharp::RustyCSharpFrontend::new(), vfs, input_uri)?,
        "tcl" => driver.run_source(&rusty_tcl::RustyTclFrontend::new(), vfs, input_uri)?,
        "swift" => driver.run_source(&rusty_swift::RustySwiftFrontend::new(), vfs, input_uri)?,
        "jl" => driver.run_source(&rusty_julia::RustyJuliaFrontend::new(), vfs, input_uri)?,
        "lua" => driver.run_source(&rusty_lua::RustyLuaFrontend::new(), vfs, input_uri)?,
        "rs" => driver.run_source(&rusty_rust::MiniRustFrontend::new(), vfs, input_uri)?,
        "zig" => driver.run_source(&rusty_zig::RustyZigFrontend::new(), vfs, input_uri)?,
        "php" => driver.run_source(&rusty_php::RustyPhpFrontend::new(), vfs, input_uri)?,
        "rb" => driver.run_source(&rusty_ruby::RustyRubyFrontend::new(), vfs, input_uri)?,
        "dart" => driver.run_source(&rusty_dart::RustyDartFrontend::new(), vfs, input_uri)?,
        "nim" => driver.run_source(&rusty_nim::RustyNimFrontend::new(), vfs, input_uri)?,
        "mojo" => driver.run_source(&rusty_mojo::RustyMojoFrontend::new(), vfs, input_uri)?,
        "kt" => driver.run_source(&rusty_kotlin::RustyKotlinFrontend::new(), vfs, input_uri)?,
        _ => return Err(NyarError::RuntimeError(format!("Unsupported file extension: .{}", ext))),
    }

    Ok(())
}

fn compile_file(
    driver: &NyarDriver,
    vfs: &DiskVfs,
    path: &Path,
    output: Option<PathBuf>,
) -> Result<(), NyarError> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let input_uri = path.to_str().ok_or_else(|| NyarError::RuntimeError("Invalid input path".to_string()))?;
    
    let output_path = output.unwrap_or_else(|| {
        let mut p = path.to_path_buf();
        p.set_extension("exe");
        p
    });
    let output_uri = output_path.to_str().ok_or_else(|| NyarError::RuntimeError("Invalid output path".to_string()))?;

    match ext {
        "c" => driver.compile_to_native(&rusty_c::RustyCFrontend::new(), vfs, input_uri, output_uri)?,
        "py" => driver.compile_to_native(&rusty_python::RustyPythonFrontend::new(), vfs, input_uri, output_uri)?,
        "java" => driver.compile_to_native(&rusty_java::MiniJavaFrontend::default(), vfs, input_uri, output_uri)?,
        "ts" | "js" => driver.compile_to_native(&rusty_typescript::RustyTypescriptFrontend::new(), vfs, input_uri, output_uri)?,
        "go" => driver.compile_to_native(&rusty_go::RustyGoFrontend::new(), vfs, input_uri, output_uri)?,
        "cs" => driver.compile_to_native(&rusty_csharp::RustyCSharpFrontend::new(), vfs, input_uri, output_uri)?,
        "tcl" => driver.compile_to_native(&rusty_tcl::RustyTclFrontend::new(), vfs, input_uri, output_uri)?,
        "swift" => driver.compile_to_native(&rusty_swift::RustySwiftFrontend::new(), vfs, input_uri, output_uri)?,
        "jl" => driver.compile_to_native(&rusty_julia::RustyJuliaFrontend::new(), vfs, input_uri, output_uri)?,
        "lua" => driver.compile_to_native(&rusty_lua::RustyLuaFrontend::new(), vfs, input_uri, output_uri)?,
        "rs" => driver.compile_to_native(&rusty_rust::MiniRustFrontend::new(), vfs, input_uri, output_uri)?,
        "zig" => driver.compile_to_native(&rusty_zig::RustyZigFrontend::new(), vfs, input_uri, output_uri)?,
        "php" => driver.compile_to_native(&rusty_php::RustyPhpFrontend::new(), vfs, input_uri, output_uri)?,
        "rb" => driver.compile_to_native(&rusty_ruby::RustyRubyFrontend::new(), vfs, input_uri, output_uri)?,
        "dart" => driver.compile_to_native(&rusty_dart::RustyDartFrontend::new(), vfs, input_uri, output_uri)?,
        "nim" => driver.compile_to_native(&rusty_nim::RustyNimFrontend::new(), vfs, input_uri, output_uri)?,
        "mojo" => driver.compile_to_native(&rusty_mojo::RustyMojoFrontend::new(), vfs, input_uri, output_uri)?,
        "kt" => driver.compile_to_native(&rusty_kotlin::RustyKotlinFrontend::new(), vfs, input_uri, output_uri)?,
        _ => return Err(NyarError::RuntimeError(format!("Unsupported file extension: .{}", ext))),
    }

    Ok(())
}

fn start_repl(_driver: &NyarDriver, _vfs: &DiskVfs, lang: &str) -> Result<(), NyarError> {
    match lang.to_lowercase().as_str() {
        "c" => {
            // Logic from cling.rs could be moved here or called
            println!("Starting C REPL (not fully implemented in dispatcher yet)");
        }
        _ => println!("REPL for {} is not yet supported in this CLI", lang),
    }
    Ok(())
}
