//! Mini Python 语言编译器
//!
//! 这是一个类似 Python 的语言前端演示程序，支持编译到 Gaia 指令或 Python 字节码 (.pyc)

use virtual_python::MiniPythonFrontend;
use nyar_vm::NyarDriver;
use std::{fs, process::exit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <input_file>", args[0]);
        exit(1);
    }

    let input_file = &args[1];
    let source_code = fs::read_to_string(input_file).unwrap_or_else(|e| {
        eprintln!("Error reading file: {}", e);
        exit(1);
    });

    let frontend = MiniPythonFrontend::new();
    if let Err(e) = NyarDriver::run_source(&frontend, &source_code) {
        eprintln!("Runtime error: {:?}", e);
        exit(1);
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

        let mut frontend = MiniPythonFrontend::new();
        let source = fs::read_to_string(temp_file.path()).unwrap();
        let result = frontend.parse(&source);

        assert!(result.is_ok());
        let program = result.unwrap();
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn test_compile_to_gaia() {
        let mut frontend = MiniPythonFrontend::new();
        let source = "x = 42\nprint(x)";

        let result = frontend.compile_to_gaia(source);
        assert!(result.is_ok());

        let gaia_program = result.unwrap();
        assert_eq!(gaia_program.name, "python_program");
        assert!(!gaia_program.functions.is_empty());
    }

    #[test]
    fn test_tokenize_simple_code() {
        let mut frontend = MiniPythonFrontend::new();
        let source = "x = 42";

        let result = frontend.tokenize(source);
        assert!(result.is_ok());

        let tokens = result.unwrap();
        assert!(!tokens.is_empty());
    }
}
