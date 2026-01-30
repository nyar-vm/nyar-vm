//! Mini C 解释器

use mini_c::MiniCFrontend;
use nyar_vm::NyarDriver;
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!(
            "Usage: {} <input_file> [-c/--compile <output_exe>]",
            args[0]
        );
        return Ok(());
    }

    let input_path = PathBuf::from(&args[1]);
    let frontend = MiniCFrontend::new();
    let driver = NyarDriver::new();

    if let Some(pos) = args.iter().position(|a| a == "-c" || a == "--compile") {
        let output_path = if pos + 1 < args.len() {
            PathBuf::from(&args[pos + 1])
        } else {
            let mut p = input_path.clone();
            p.set_extension("exe");
            p
        };
        println!(
            "正在编译 Mini C 文件: {:?} -> {:?}",
            input_path, output_path
        );
        driver.compile_to_native(&frontend, &input_path, &output_path)?;
    } else {
        println!("正在运行 Mini C 文件: {:?}", input_path);
        driver.run_source(&frontend, &input_path)?;
    }

    Ok(())
}
