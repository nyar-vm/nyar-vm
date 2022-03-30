use crate::{
    helpers::{Id, WasmOutput},
    modules::ModuleBuilder,
};
use nyar_error::NyarError;
use wast::{
    core::{Custom, InlineExport, Memory, MemoryKind, Module, ModuleField, ModuleKind, Producers},
    token::{NameAnnotation, Span},
};

impl ModuleBuilder {
    pub fn build_module_wasm(&self) -> Result<Vec<u8>, NyarError> {
        let mut module = self.build_module()?;
        match module.encode() {
            Ok(o) => Ok(o),
            Err(e) => Err(NyarError::custom(e)),
        }
    }
    pub fn build_module(&self) -> Result<Module, NyarError> {
        let mut terms = Vec::with_capacity(1024);
        for (_, _, k) in self.externals.into_iter() {
            terms.push(ModuleField::Import(k.as_wast()))
        }
        for (_, _, k) in self.types.into_iter() {
            terms.push(k.as_wast())
        }
        for (_, _, k) in self.functions.get_natives() {
            terms.push(ModuleField::Func(k.as_wast()))
        }
        for (_, _, k) in self.globals.into_iter() {
            terms.push(ModuleField::Global(k.as_wast()))
        }
        for (_, _, k) in self.data.into_iter() {
            terms.push(ModuleField::Data(k.as_wast()))
        }
        terms.push(ModuleField::Memory(self.wast_memory()));
        terms.push(self.wast_producer());
        Ok(Module {
            span: Span::from_offset(0),
            id: None,
            name: Some(NameAnnotation { name: self.get_module_name() }),
            kind: ModuleKind::Text(terms),
        })
    }
    fn wast_memory(&self) -> Memory {
        Memory {
            span: Span::from_offset(0),
            id: Id::type_id("static"),
            name: None,
            exports: InlineExport { names: vec!["static"] },
            kind: MemoryKind::Inline { is_32: true, data: vec![] },
        }
    }
    fn wast_producer(&self) -> ModuleField {
        let item = Custom::Producers(Producers {
            fields: vec![
                ("language", vec![("valkyrie", "2024"), ("player", "berserker")]),
                ("processed-by", vec![("nyar-wasm", env!("CARGO_PKG_VERSION"))]),
            ],
        });
        ModuleField::Custom(item)
    }
}
