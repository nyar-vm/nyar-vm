use nyar_vm::NyarDriver;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Nyar Universal Runtime");
        println!("Usage: nyar <input_file>");
        return Ok(());
    }

    let driver = NyarDriver::new();
    let file_path = &args[1];

    if file_path.ends_with(".lua") {
        let frontend = rusty_lua::RustyLuaFrontend::new();
        let vfs = driver.default_vfs();
        driver.run_source(&frontend, &vfs, file_path)?;
    } else {
        println!("Nyar Universal Runtime (Internal Testing)");
        println!("Unsupported file format: {}", file_path);
    }

    Ok(())
}

fn _run_driver<F: nyar_types::NyarFrontend>(
    driver: &NyarDriver,
    frontend: &F,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        println!("Usage: {} <input_file>", args[0]);
        return Ok(());
    }
    let vfs = driver.default_vfs();
    let uri = &args[1];
    driver.run_source(frontend, &vfs, uri)?;
    Ok(())
}
