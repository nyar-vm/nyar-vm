//! `asgard` 产品二进制入口。

fn main() -> std::process::ExitCode {
    match rusty_valkyrie::asgard::cli::run_from_env() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error:?}");
            std::process::ExitCode::FAILURE
        }
    }
}
