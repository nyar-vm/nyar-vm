//! 跨语言源码格式化与 AST pretty printer。
//!
//! 本 crate 是语言中立的平台层，只拥有注册表、扩展名表与平台文本模型：
//! VON / AWSL 的源码格式化委托对应 Oak formatter，WAT / WIT / MSIL 的文本模型与写出在此实现；
//! Valkyrie 等具体前端在启动时向 [`formatter::register_source_provider`] /
//! [`printer::register_printer_provider`] 注册 provider（方向是前端 → 平台）。
//! 语言语义不属于本 crate。

#![deny(missing_docs)]

pub mod awsl;
pub mod formatter;
pub mod msil;
pub mod printer;
pub mod von;
pub mod wat;
pub mod wit;
