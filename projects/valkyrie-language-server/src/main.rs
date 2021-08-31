use clap::Parser;

use nyar_error::NyarResult;
use valkyrie_language_server::App;

#[tokio::main]
async fn main() -> NyarResult<()> {
    App::parse().run().await
}
