use nyar_vm::NyarDriver;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let program_name = if let Some(last_slash) = args[0].rfind(|c| c == '/' || c == '\\') {
        &args[0][last_slash + 1..]
    } else {
        &args[0]
    };

    let _driver = NyarDriver::new();

    match program_name {
        // Frontends are currently disabled as they are not in the workspace
        _ => {
            if args.len() < 2 {
                println!("Nyar Universal Runtime");
                println!("Usage: nyar <input_file>");
                return Ok(());
            }
            println!("Nyar Universal Runtime (Internal Testing)");
            // In a real scenario, we would pick a frontend based on file extension
            // For now, this is just a placeholder.
        }
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
