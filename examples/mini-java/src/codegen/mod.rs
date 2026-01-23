//! Java 到 Nyar 字节码的翻译器

use nyar_vm::bytecode::format::NyarModule;
use oak_java::ast::JavaRoot;
use anyhow::Result;

pub struct NyarTranslator;

impl NyarTranslator {
    pub fn new() -> Self {
        Self
    }

    pub fn translate(&self, _ast: &JavaRoot) -> Result<NyarModule> {
        // 翻译逻辑
        Ok(NyarModule::default())
    }
}
