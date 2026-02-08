use clap::Parser;
use oak_core::source::Source;
use oak_vfs::{DiskVfs, Vfs, WritableVfs};
use rusty_typescript::project::ProjectLoader;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "tsc",
    version = "0.1.0",
    author = "Nyar Project",
    about = "Rusty TypeScript AOT Compiler to WASM"
)]
struct Args {
    /// The input TypeScript file
    #[arg(index = 1)]
    input: String,

    /// Output file (.wasm)
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Target architecture (e.g. wasm32-wasi)
    #[arg(short, long, default_value = "wasm32-wasi")]
    target: String,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let vfs = DiskVfs::new();

    if args.verbose {
        println!("Compiling {} to WASM...", args.input);
    }

    let mut loader = ProjectLoader::new(vfs.clone());
    
    // 使用 ProjectLoader 加载项目，它是 IO 无关的
    let modules = loader.load_project(&args.input)
        .map_err(|e| format!("Failed to load project: {}", e))?;

    // 目前 tsc.rs 只处理单个文件的编译到 WASM，这里我们取第一个模块进行演示
    // 实际生产中可能需要更复杂的逻辑来处理多模块 AOT
    if let Some(module) = modules.first() {
        let frontend = loader.frontend();
        // 这里假设我们从模块中提取源码重新编译，或者直接从 AST/Tree 编译
        // 为了保持原有的 compile_to_wasm 逻辑，我们需要获取源码
        let source = vfs.get_source(&args.input)
            .ok_or_else(|| format!("Source not found: {}", args.input))?;
        let content = source.get_text_from(0);
        
        let artifacts = frontend.compile_to_wasm(&content)?;

        for (name, bytes) in artifacts {
            let output_uri = if name == "main.wasm" && args.output.is_some() {
                args.output.clone().unwrap()
            } else {
                let mut path = PathBuf::from(&args.input);
                let ext = name.split('.').last().unwrap_or("bin");
                path.set_extension(ext);
                path.to_string_lossy().to_string()
            };

            vfs.write_file(&output_uri, String::from_utf8_lossy(&bytes).to_string().into());
            println!("Successfully compiled to {}", output_uri);
        }
    }

    Ok(())
}
