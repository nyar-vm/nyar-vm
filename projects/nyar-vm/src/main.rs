use nyar_vm::NyarDriver;
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let program_name = PathBuf::from(&args[0])
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let driver = NyarDriver::new();

    match program_name.as_str() {
        "csc" | "csharp" => {
            let frontend = rusty_csharp::CSharpFrontend::new();
            run_driver(&driver, &frontend, &args)?;
        }
        "javac" | "java" => {
            let frontend = rusty_java::JavaFrontend::new();
            run_driver(&driver, &frontend, &args)?;
        }
        "kotlinc" | "kotlin" => {
            let frontend = rusty_kotlin::KotlinFrontend::new();
            run_driver(&driver, &frontend, &args)?;
        }
        "python" => {
            let frontend = rusty_python::PythonFrontend::new();
            run_driver(&driver, &frontend, &args)?;
        }
        "lua" => {
            let frontend = rusty_lua::LuaFrontend::new();
            run_driver(&driver, &frontend, &args)?;
        }
        "rustc" => {
            let frontend = rusty_rust::MiniRustFrontend::new();
            run_driver(&driver, &frontend, &args)?;
        }
        "tsc" | "tsx" => {
            let frontend = rusty_typescript::MiniTypescriptFrontend::new();
            run_driver(&driver, &frontend, &args)?;
        }
        "cc" | "gcc" | "clang" => {
            let frontend = mini_c::MiniCFrontend::new();
            run_driver(&driver, &frontend, &args)?;
        }
        "go" => {
            let frontend = mini_go::MiniGoFrontend::new();
            run_driver(&driver, &frontend, &args)?;
        }
        _ => {
            if args.len() < 2 {
                println!("Usage: nyar <frontend> <input_file>");
                println!("Supported frontends: csharp, java, kotlin, python, lua, rust, typescript, c, go");
                return Ok(());
            }
            // Default behavior if not called as a symlink
            println!("Nyar Universal Runtime");
            println!("Please use one of the supported tool names or specify a frontend.");
        }
    }

    Ok(())
}

fn run_driver<F: nyar_frontend::NyarFrontend>(
    driver: &NyarDriver,
    frontend: &F,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        println!("Usage: {} <input_file>", args[0]);
        return Ok(());
    }
    let input_path = PathBuf::from(&args[1]);
    driver.run_source(frontend, &input_path)?;
    Ok(())
}
