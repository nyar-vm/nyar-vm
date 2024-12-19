//! `legion` 产品二进制入口。

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(rusty_valkyrie::legion::cli::run_from_env() as u8)
}
