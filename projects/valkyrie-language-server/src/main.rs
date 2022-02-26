use jsonrpc_core::{IoHandler, Params, Result};
// use clap::Parser;
//
// use nyar_error::NyarResult;
// use valkyrie_language_server::App;
//
// #[tokio::main]
// async fn main() -> NyarResult<()> {
//     App::parse().run().await
// }
use jsonrpc_ws_server::*;
use serde_json::Value;

fn main() {
    let mut io = IoHandler::new();
    io.add_method("say_hello", say_hello);

    let server = ServerBuilder::new(io).start(&"0.0.0.0:3030".parse().unwrap()).expect("Server must start with no issues");

    server.wait().unwrap()
}

async fn say_hello(_params: Params) -> Result<Value> {
    Ok(Value::String("hello".into()))
}
