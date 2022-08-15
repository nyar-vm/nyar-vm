use crate::{
    helpers::{Id, IntoWasm},
    modules::ModuleBuilder,
};
use nyar_error::NyarError;
use wast::{
    core::{Custom, DataVal, InlineExport, Limits, Memory, MemoryKind, MemoryType, Module, ModuleField, ModuleKind, Producers},
    token::{Index, NameAnnotation, Span},
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
        for (_, _, k) in self.functions.into_iter() {
            terms.push(ModuleField::Func(k.as_wast()))
        }
        for (_, _, k) in self.globals.into_iter() {
            terms.push(ModuleField::Global(k.as_wast()))
        }
        for (_, _, k) in self.data.into_iter() {
            terms.push(ModuleField::Data(k.as_wast()))
        }

        terms.push(ModuleField::Memory(self.build_memory()));
        self.build_start(&mut terms);
        self.build_producer(&mut terms);
        Ok(Module {
            span: Span::from_offset(0),
            id: None,
            name: Some(NameAnnotation { name: self.get_module_name() }),
            kind: ModuleKind::Text(terms),
        })
    }
    fn build_memory(&self) -> Memory {
        Memory {
            span: Span::from_offset(0),
            id: Id::type_id("memory"),
            name: None,
            exports: InlineExport { names: vec!["memory"] },
            kind: MemoryKind::Normal(MemoryType::B32 { limits: Limits { min: 1, max: None }, shared: false }),
        }
    }
    fn build_start<'a, 'b>(&'a self, m: &mut Vec<ModuleField<'b>>)
    where
        'a: 'b,
    {
        if !self.entry.is_empty() {
            m.push(ModuleField::Start(Index::Id(Id::new(&self.entry))))
        }
    }
    fn build_producer(&self, m: &mut Vec<ModuleField>) {
        let item = Custom::Producers(Producers {
            fields: vec![
                ("language", vec![("valkyrie", "2024"), ("player", "berserker")]),
                ("processed-by", vec![("nyar-wasm", env!("CARGO_PKG_VERSION"))]),
            ],
        });
        m.push(ModuleField::Custom(item))
    }
}
