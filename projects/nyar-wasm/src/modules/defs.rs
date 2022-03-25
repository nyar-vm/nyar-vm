use crate::{helpers::WasmOutput, modules::ModuleBuilder};
use nyar_error::NyarError;
use nyar_hir::{ExternalType, NyarType, TypeItem};
use wast::{
    component::{Component, ComponentKind},
    core::{
        Custom, FunctionType, HeapType, Import, ItemKind, ItemSig, Module, ModuleField, ModuleKind, Producers, RefType,
        TypeUse, ValType,
    },
    token::{NameAnnotation, Span},
    Wat,
};

impl<'a, 'i> WasmOutput<'a, ValType<'i>> for NyarType {
    fn as_wast(&'a self) -> ValType<'i> {
        match self {
            NyarType::I8 => ValType::I32,
            NyarType::I16 => ValType::I32,
            NyarType::I32 => ValType::I32,
            NyarType::I64 => ValType::I64,
            NyarType::F32 => ValType::F32,
            NyarType::F64 => ValType::F64,
            NyarType::Any => ValType::Ref(RefType { nullable: true, heap: HeapType::Func }),
            NyarType::Structure => ValType::Ref(RefType { nullable: true, heap: HeapType::Struct }),
            NyarType::Array => ValType::Ref(RefType { nullable: true, heap: HeapType::Array }),
        }
    }
}

impl<'a, 'i> WasmOutput<'a, ModuleField<'i>> for ExternalType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ModuleField<'i> {
        let item = Import {
            span: Span::from_offset(0),
            module: self.module.as_ref(),
            field: self.field.as_ref(),
            item: ItemSig {
                span: Span::from_offset(0),
                id: None,
                name: None,
                kind: ItemKind::Func(TypeUse { index: None, inline: Some(self.as_wast()) }),
            },
        };
        ModuleField::Import(item)
    }
}

impl<'a, 'i> WasmOutput<'a, FunctionType<'i>> for ExternalType {
    fn as_wast(&self) -> FunctionType<'i> {
        let input = self.input.iter().map(|ty| (None, None, ty.as_wast())).collect::<Vec<_>>();
        let result = self.output.iter().map(|ty| ty.as_wast()).collect::<Vec<_>>();
        FunctionType { params: Box::from(input), results: Box::from(result) }
    }
}

impl<'a, 'i> WasmOutput<'a, ModuleField<'i>> for TypeItem {
    fn as_wast(&self) -> ModuleField<'i> {
        match self {
            Self::Structure(v) => ModuleField::Type(v.as_wast()),
            Self::Array(v) => ModuleField::Type(v.as_wast()),
        }
    }
}

impl ModuleBuilder {
    fn wast_producer(&self) -> ModuleField {
        let item = Custom::Producers(Producers {
            fields: vec![
                ("language", vec![("valkyrie", "2024"), ("player", "berserker")]),
                ("processed-by", vec![("nyar-wasm", env!("CARGO_PKG_VERSION"))]),
            ],
        });
        ModuleField::Custom(item)
    }
    pub fn build_module(&self) -> Result<Wat, NyarError> {
        let mut terms: Vec<ModuleField> = Vec::with_capacity(1024);
        for (_, _, k) in self.functions.get_externals() {
            terms.push(k.as_wast())
        }
        for (_, _, k) in self.types.into_iter() {
            terms.push(k.as_wast())
        }
        terms.push(self.wast_producer());
        Ok(Wat::Module(Module {
            span: Span::from_offset(0),
            id: None,
            name: Some(NameAnnotation { name: "runtime" }),
            kind: ModuleKind::Text(terms),
        }))
    }

    pub fn build_component(&self) -> Result<Wat, NyarError> {
        Ok(Wat::Component(Component {
            span: Span::from_offset(0),
            id: None,
            name: Some(NameAnnotation { name: "runtime" }),
            kind: ComponentKind::Text(vec![]),
        }))
    }
}
