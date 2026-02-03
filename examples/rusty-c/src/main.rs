//! Rusty C 解释器

use rusty_c::RustyCFrontend;
use nyar_vm::NyarDriver;
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("DEBUG: rusty-c started");
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!(
            "Usage: {} <input_file> [-c/--compile <output_exe>]",
            args[0]
        );
        return Ok(());
    }

    let mut input_file = None;
    let mut output_file = None;
    let mut compile_only = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-c" | "--compile" => {
                compile_only = true;
            }
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    output_file = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            _ => {
                if input_file.is_none() {
                    input_file = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }

    let input_path = match input_file {
        Some(f) => PathBuf::from(f),
        None => {
            println!("Error: No input file specified");
            return Ok(());
        }
    };

    let frontend = RustyCFrontend::new();
    let driver = NyarDriver::new();

    if compile_only {
        let output_path = match output_file {
            Some(f) => PathBuf::from(f),
            None => {
                let mut p = input_path.clone();
                p.set_extension("exe");
                p
            }
        };
        println!(
            "正在编译 Rusty C 文件: {:?} -> {:?}",
            input_path, output_path
        );
        driver.compile_to_native(&frontend, &input_path, &output_path)?;
    } else {
        println!("正在运行 Rusty C 文件: {:?}", input_path);
        driver.run_source(&frontend, &input_path)?;
    }

    Ok(())
}
