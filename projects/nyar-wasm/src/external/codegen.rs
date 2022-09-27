use super::*;
use wast::{
    component::{
        CanonLower, ComponentExportType, ComponentExternName, ComponentFunctionResult, ComponentFunctionType, ComponentTypeUse,
        CoreFunc, CoreFuncKind, InstanceTypeDecl, ItemRef, ItemSig, ItemSigKind,
    },
    token::Index,
};

impl<'a, 'i> IntoWasm<'a, Import<'i>> for ImportFunction
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Import<'i> {
        Import {
            span: Span::from_offset(0),
            module: self.external.long_name(),
            field: self.local.as_ref(),
            item: wast::core::ItemSig {
                span: Span::from_offset(0),
                id: WasmName::id(self.function_id()),
                name: None,
                kind: ItemKind::Func(TypeUse { index: None, inline: Some(self.as_wast()) }),
            },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, CoreFunc<'i>> for ImportFunction
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CoreFunc<'i> {
        CoreFunc {
            span: Span::from_offset(0),
            id: WasmName::id(self.function_id()),
            name: None,
            kind: CoreFuncKind::Lower(CanonLower {
                func: ItemRef {
                    kind: Default::default(),
                    idx: WasmName::index(self.external.long_name()),
                    export_names: vec![self.local.as_ref()],
                },
                opts: vec![],
            }),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, InstanceTypeDecl<'i>> for ImportFunction
where
    'a: 'i,
{
    fn as_wast(&'a self) -> InstanceTypeDecl<'i> {
        InstanceTypeDecl::Export(ComponentExportType {
            span: Span::from_offset(0),
            name: ComponentExternName(self.local.as_ref()),
            item: ItemSig {
                span: Span::from_offset(0),
                id: None,
                name: None,
                kind: ItemSigKind::Func(ComponentTypeUse::Inline(self.as_wast())),
            },
        })
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::FunctionType<'i>> for ImportFunction
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FunctionType<'i> {
        let input = self.inputs.iter().map(|ty| ty.as_wast()).collect::<Vec<_>>();
        let result = [self.output.as_wast()];
        wast::core::FunctionType { params: Box::from(input), results: Box::from(result) }
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentFunctionType<'i>> for ImportFunction
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentFunctionType<'i> {
        let input = self.inputs.iter().map(|ty| ty.as_wast()).collect::<Vec<_>>();
        let result = [ComponentFunctionResult { name: None, ty: self.output.as_wast() }];
        ComponentFunctionType { params: Box::from(input), results: Box::from(result) }
    }
}
