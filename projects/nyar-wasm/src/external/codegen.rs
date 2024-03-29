use super::*;
use wast::{
    component::{
        CanonLower, CanonOpt, ComponentExportType, ComponentExternName, ComponentTypeUse, CoreFunc, CoreFuncKind,
        CoreInstanceExport, CoreItemRef, InstanceTypeDecl, ItemRef, ItemSig, ItemSigKind,
    },
    core::ExportKind,
};

impl<'a, 'i> IntoWasm<'a, wast::core::Import<'i>> for ExternalFunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::Import<'i> {
        wast::core::Import {
            span: Span::from_offset(0),
            module: self.external_package.long_name(),
            field: self.external_name.as_ref(),
            item: wast::core::ItemSig {
                span: Span::from_offset(0),
                id: WasmName::id(self.function_id()),
                name: None,
                kind: ItemKind::Func(TypeUse { index: None, inline: Some(self.signature.as_wast()) }),
            },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, CoreFunc<'i>> for ExternalFunctionType
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
                    idx: WasmName::index(self.external_package.long_name()),
                    export_names: vec![self.external_name.as_ref()],
                },
                opts: vec![
                    // CanonOpt::Memory(CoreItemRef {
                    //     kind: Default::default(),
                    //     idx: WasmName::index("wasi_random"),
                    //     export_name: Some("memory"),
                    // }),
                    CanonOpt::StringUtf8,
                ],
            }),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, CoreInstanceExport<'i>> for ExternalFunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CoreInstanceExport<'i> {
        CoreInstanceExport {
            span: Span::from_offset(0),
            name: self.external_name.as_ref(),
            item: CoreItemRef { kind: ExportKind::Func, idx: WasmName::index(self.function_id()), export_name: None },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, InstanceTypeDecl<'i>> for ExternalFunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> InstanceTypeDecl<'i> {
        InstanceTypeDecl::Export(ComponentExportType {
            span: Span::from_offset(0),
            name: ComponentExternName(self.external_name.as_ref()),
            item: self.as_wast(),
        })
    }
}

impl<'a, 'i> IntoWasm<'a, ItemSig<'i>> for ExternalFunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ItemSig<'i> {
        let kind = ItemSigKind::Func(ComponentTypeUse::Inline(self.signature.as_wast()));
        ItemSig { span: Span::from_offset(0), id: None, name: None, kind }
    }
}
