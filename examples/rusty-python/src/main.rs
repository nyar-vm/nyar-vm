//! Rusty Python 语言编译器
//!
//! 这是一个类似 Python 的语言 frontend 演示程序，支持编译到 Gaia 指令或 Python 字节码 (.pyc)

use chomsky_extract::{Backend, BackendArtifact};
use nyar_types::NyarFrontend;
use rusty_python::codegen::GaiaTranslator;
use rusty_python::pyc_codegen::PycTranslator;
use rusty_python::RustyPythonFrontend;
use std::{fs, path::Path, process::exit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_file> [--target <target>] [--output <output>]", args[0]);
        eprintln!("Targets: gaia (default), pyc");
        exit(1);
    }

    let input_file = Path::new(&args[1]);
    let mut target = "gaia";
    let mut output_path = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                if i + 1 < args.len() {
                    target = &args[i + 1];
                    i += 2;
                } else {
                    eprintln!("Missing value for --target");
                    exit(1);
                }
            }
            "--output" => {
                if i + 1 < args.len() {
                    output_path = Some(&args[i + 1]);
                    i += 2;
                } else {
                    eprintln!("Missing value for --output");
                    exit(1);
                }
            }
            // Backward compatibility
            "--pyc" => {
                target = "pyc";
                if i + 1 < args.len() {
                    output_path = Some(&args[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    let frontend = RustyPythonFrontend::new();
    let source = fs::read_to_string(input_file).expect("Failed to read input file");

    let ast = frontend.parse(&source).expect("Failed to parse source");
    let tree = frontend.lower(&ast).expect("Failed to lower to IR");

    let backend: Box<dyn Backend> = match target {
        "gaia" => Box::new(GaiaTranslator::new()),
        "pyc" => Box::new(PycTranslator::new(
            input_file.file_name().unwrap().to_str().unwrap(),
            "main",
        )),
        _ => {
            eprintln!("Unknown target: {}", target);
            exit(1);
        }
    };

    println!("Target: {}", backend.name());

    match backend.generate(&tree) {
        Ok(artifact) => match artifact {
            BackendArtifact::Source(s) => {
                if let Some(out) = output_path {
                    fs::write(out, s).expect("Failed to write output");
                    println!("Generated source to {}", out);
                } else {
                    println!("Successfully compiled to Gaia module");
                    // Run with NyarDriver if it's Gaia
                    if target == "gaia" {
                        let driver = nyar_vm::NyarDriver::new();
                        if let Err(e) = driver.run_source(&frontend, input_file) {
                            eprintln!("Runtime error: {:?}", e);
                            exit(1);
                        }
                    } else {
                        println!("{}", s);
                    }
                }
            }
            BackendArtifact::Binary(b) => {
                let out = output_path.map(|s| s.to_string()).unwrap_or_else(|| {
                    let mut p = input_file.to_path_buf();
                    p.set_extension(target);
                    p.to_str().unwrap().to_string()
                });
                fs::write(&out, b).expect("Failed to write binary output");
                println!("Generated binary to {}", out);
            }
            _ => {
                eprintln!("Unsupported artifact type");
                exit(1);
            }
        },
        Err(e) => {
            eprintln!("Compilation error: {:?}", e);
            exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write};
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_simple_python_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "x = 42").unwrap();
        writeln!(temp_file, "print(x)").unwrap();

        let mut frontend = RustyPythonFrontend::new();
        let source = fs::read_to_string(temp_file.path()).unwrap();
        let result = frontend.parse(&source);

        assert!(result.is_ok());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn test_compile_to_gaia() {
        let mut frontend = RustyPythonFrontend::new();
        let source = "x = 42\nprint(x)";

        let result = frontend.compile_to_gaia(source);
        assert!(result.is_ok());

        let gaia_program = result.unwrap();
        assert_eq!(gaia_program.name, "python_program");
        assert!(!gaia_program.functions.is_empty());
    }

    #[test]
    fn test_tokenize_simple_code() {
        let mut frontend = RustyPythonFrontend::new();
        let source = "x = 42";

        let result = frontend.tokenize(source);
        assert!(result.is_ok());

        let tokens = result.unwrap();
        assert!(!tokens.is_empty());
    }
}
