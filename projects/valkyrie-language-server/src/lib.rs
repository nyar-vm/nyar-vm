#![feature(trivial_bounds)]
#![feature(never_type)]

use std::{net::SocketAddr, str::FromStr};

use axum::Server;
use clap::Parser;

use nyar_error::NyarResult;

pub mod local;
pub mod remote;
pub mod utils;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct App {
    #[arg(short, long)]
    local: Option<String>,
    #[arg(short, long)]
    remote: Option<String>,
    #[arg(short, long)]
    socket: Option<String>,
}

impl App {
    pub async fn run(&self) -> NyarResult<()> {
        let mut socket = SocketAddr::from(([127, 0, 0, 1], 9600));
        match &self.socket {
            Some(s) => match SocketAddr::from_str(s) {
                Ok(o) => socket = o,
                Err(_) => {
                    println!("Invalid socket address `{}`, fallback to `{}`", s, socket)
                }
            },
            None => {}
        }
        match &self.local {
            None => {
                todo!()
            }
            Some(s) => {
                let router = local::LanguageServer::start(s)?;
                Server::bind(&socket).serve(router.into_make_service()).await.unwrap();
            }
        }
        Ok(())
    }
}
