#![deny(missing_debug_implementations, missing_copy_implementations)]
#![warn(missing_docs, rustdoc::missing_crate_level_docs)]
#![doc = include_str!("../readme.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oovm/shape-rs/dev/projects/images/Trapezohedron.svg")]


mod assembler;
mod errors;
mod helpers;
mod reader;
mod writer;
mod viewer;

#[derive(Copy, Clone, Debug)]
pub struct PeContext {}

// 使用 wasi-io-bindgen 生成 WIT 绑定
#[allow(missing_docs)]
wit_bindgen::generate!({
    world: "pe-assembly",
});

export!(PeContext);