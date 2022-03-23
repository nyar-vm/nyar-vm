use crate::modules::ModuleBuilder;
use nyar_error::NyarError;
use nyar_hir::{ExternalType, FieldType, NyarType, StructureType};
use std::mem::transmute;
use wast::{
    component::{Component, ComponentKind},
    core::{
        Custom, FunctionType, HeapType, Import, ItemKind, ItemSig, Module, ModuleField, ModuleKind, Producers, RefType,
        StorageType, StructField, StructType, Type, TypeDef, TypeUse, ValType,
    },
    token::{Index, NameAnnotation, Span},
    Wat,
};

pub struct Id<'a> {
    name: &'a str,
    gen: u32,
    span: Span,
}

impl<'a> Id<'a> {
    pub fn new(name: &'a str, offset: usize) -> wast::token::Id<'a> {
        unsafe {
            let s = Id { name, gen: 0, span: Span::from_offset(offset) };
            transmute::<Id, wast::token::Id>(s)
        }
    }
    pub fn type_id(name: &'a str, offset: usize) -> Option<wast::token::Id<'a>> {
        Some(Self::new(name, offset))
    }

    pub fn type_index(name: &'a str, offset: usize) -> Option<Index> {
        Some(Index::Id(Self::new(name, offset)))
    }
}

pub trait WasmOutput<'a, Item> {
    fn as_wast(&'a self) -> Item;
}

impl<'a, 'i> WasmOutput<'a, ModuleField<'i>> for StructureType {
    fn as_wast(&'a self) -> ModuleField<'i> {
        let offset = self.namepath.span.get_start();

        let item = Type {
            span: Span::from_offset(offset),
            id: Id::type_id("point", offset),
            name: Some(NameAnnotation { name: "Point" }),
            def: TypeDef::Struct(StructType {
                fields: vec![
                    StructField { id: None, mutable: false, ty: StorageType::I8 },
                    StructField { id: None, mutable: false, ty: StorageType::Val(ValType::Ref(RefType::r#struct())) },
                ],
            }),
            parent: None,
            final_type: Some(true),
        };
        ModuleField::Type(item)
    }
}

impl<'a, 'i> WasmOutput<'a, StructType<'i>> for StructureType {
    fn as_wast(&'a self) -> StructType<'i> {
        StructType { fields: self.fields().map(|(_, _, field)| field.as_wast()).collect() }
    }
}

impl<'a, 'i> WasmOutput<'a, StructField<'i>> for FieldType {
    fn as_wast(&'a self) -> StructField<'i> {
        StructField { id: None, mutable: self.mutable, ty: StorageType::I8 }
    }
}

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

impl ModuleBuilder {
    fn build_structure(&self) -> ModuleField {
        let item = Type {
            span: Span::from_offset(0),
            id: Id::type_id("point", 0),
            name: Some(NameAnnotation { name: "Point" }),
            def: TypeDef::Struct(StructType {
                fields: vec![
                    StructField { id: None, mutable: false, ty: StorageType::I8 },
                    StructField { id: None, mutable: false, ty: StorageType::Val(ValType::Ref(RefType::r#struct())) },
                ],
            }),
            parent: None,
            final_type: Some(true),
        };
        ModuleField::Type(item)
    }
    fn build_function_type(&self) -> ModuleField {
        let item = Type {
            span: Span::from_offset(0),
            id: Id::type_id("tty", 0),
            name: None,
            def: TypeDef::Func(FunctionType {
                params: Box::new([
                    (None, Some(NameAnnotation { name: "a" }), ValType::I32),
                    (None, Some(NameAnnotation { name: "b" }), ValType::I32),
                    (None, Some(NameAnnotation { name: "c" }), ValType::I32),
                    (None, Some(NameAnnotation { name: "d" }), ValType::I32),
                ]),
                results: Box::new([ValType::I32]),
            }),
            parent: None,
            final_type: None,
        };
        ModuleField::Type(item)
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
    pub fn build_module(&self) -> Result<Wat, NyarError> {
        let mut terms = Vec::with_capacity(1024);

        for (i, j, k) in self.functions.get_externals() {
            terms.push(k.as_wast())
        }
        terms.extend(vec![self.build_function_type(), self.build_structure(), self.wast_producer()]);

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
