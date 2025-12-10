
#[tokio::main]
async fn main() {
    let cli = NyarCli::parse();
    cli.run().await?;
    std::process::exit(code);
}
