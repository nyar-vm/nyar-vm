use mini_go::MiniGoFrontend;
use nyar_vm::NyarDriver;
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <input_file>", args[0]);
        return Ok(());
    }

    let input_path = PathBuf::from(&args[1]);
    let frontend = MiniGoFrontend::new();
    let driver = NyarDriver::new();

    println!("正在运行 Mini Go 文件: {:?}", input_path);
    driver.run_source(&frontend, &input_path)?;

    Ok(())
}
