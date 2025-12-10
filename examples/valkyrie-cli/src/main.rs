use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "valkyrie", version, about = "Valkyrie language CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Compile { file: String, #[arg(short, long)] out: Option<String> },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Compile { file, out } => {
            let src = match std::fs::read_to_string(&file) { Ok(s) => s, Err(e) => { eprintln!("io error: {}", e); return std::process::ExitCode::from(1); } };
            let m = match valkyrie_language::compile_text_to_module(&src) { Ok(m) => m, Err(e) => { eprintln!("{}", e); return std::process::ExitCode::from(1); } };
            let data = m.encode();
            let out_path = out.unwrap_or_else(|| {
                let p = std::path::Path::new(&file);
                let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                let parent = p.parent().unwrap_or_else(|| std::path::Path::new("."));
                parent.join(format!("{}.nyarc", stem)).to_string_lossy().into_owned()
            });
            if let Err(e) = std::fs::write(&out_path, &data) { eprintln!("write error: {}", e); return std::process::ExitCode::from(1); }
            println!("compiled -> {} ({} bytes)", out_path, data.len());
            std::process::ExitCode::SUCCESS
        }
    }
}

