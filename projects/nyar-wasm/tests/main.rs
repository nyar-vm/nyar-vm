use nyar_error::NyarError;

use std::{path::Path, process::Command};

mod adt_examples;
mod cf_examples;
mod io_examples;

pub use crate::{adt_examples::*, cf_examples::*, io_examples::*};

#[test]
fn ready() {
    println!("it works!")
}

#[test]
fn test() -> Result<(), NyarError> {
    let debug = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/valkyrie");
    if !debug.exists() {
        std::fs::create_dir_all(&debug).unwrap();
    }
    // let _ = new_structure().build_module(debug.join("mod.wasm")).unwrap();
    // let _ = new_array().build_module(debug.join("array.wasm")).unwrap();
    let _ = import_random().build_component(debug.join("random_pi.wasm")).unwrap();
    // let _ = control_flow().build_module(debug.join("control.wasm")).unwrap();

    let o = Command::new("valor").arg("build").output();
    println!("{o:?}");
    Ok(())
}
