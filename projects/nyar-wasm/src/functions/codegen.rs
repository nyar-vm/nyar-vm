use super::*;
use crate::helpers::WasmName;
use wast::{
    component::{CanonOpt, ComponentField, ComponentValType, CoreItemRef, ItemRef},
    core::Local,
};

impl<'a, 'i> IntoWasm<'a, Start<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Start<'i> {
        Start { func: self.symbol.as_index(), args: vec![], results: vec![] }
    }
}

impl FunctionType {
    /// Make a component function if it needs to export
    pub fn make_component<'a, 'i>(&'i self, instance_name: &'a str) -> Option<ComponentField<'i>>
    where
        'a: 'i,
    {
        let export = self.export.as_ref()?;
        Some(ComponentField::Func(Func {
            span: Span::from_offset(0),
            id: self.symbol.as_id(),
            name: None,
            exports: export.as_wast(),
            kind: FuncKind::Lift {
                ty: ComponentTypeUse::Inline(self.signature.as_wast()),
                info: CanonLift {
                    func: CoreItemRef {
                        kind: Default::default(),
                        idx: WasmName::index(instance_name),
                        export_name: Some(export.short_name()),
                    },
                    opts: vec![
                        CanonOpt::Memory(CoreItemRef {
                            kind: Default::default(),
                            idx: WasmName::index(instance_name),
                            export_name: Some("memory"),
                        }),
                        CanonOpt::StringUtf8,
                    ],
                },
            },
        }))
    }
}

impl<'a, 'i> IntoWasm<'a, CoreFunc<'i>> for FunctionType
where
    'a: 'i,
{
    /// 怎么把组件函数降低为核心函数
    fn as_wast(&'a self) -> CoreFunc<'i> {
        CoreFunc { span: Span::from_offset(0), id: self.symbol.as_id(), name: None, kind: self.as_wast() }
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::Func<'i>> for FunctionType
where
    'a: 'i,
{
    /// 核心函数的实现
    fn as_wast(&'a self) -> wast::core::Func<'i> {
        wast::core::Func {
            span: Span::from_offset(0),
            id: self.symbol.as_id(),
            name: None,
            exports: self.export.as_wast(),
            kind: self.as_wast(),
            ty: TypeUse { index: None, inline: Some(self.signature.as_wast()) },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, CoreFuncKind<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CoreFuncKind<'i> {
        CoreFuncKind::Lower(CanonLower {
            func: ItemRef { kind: wast::kw::func(Span::from_offset(0)), idx: self.symbol.as_index(), export_names: Vec::new() },
            opts: Vec::new(),
        })
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentFunctionType<'i>> for FunctionSignature
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentFunctionType<'i> {
        let params: Vec<_> = self.inputs.iter().map(|v| v.as_wast()).collect();
        let results = [ComponentFunctionResult { name: None, ty: ComponentValType::Inline(self.output.as_wast()) }];
        ComponentFunctionType { params: Box::from(params), results: Box::from(results) }
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::FunctionType<'i>> for FunctionSignature
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FunctionType<'i> {
        let params: Vec<_> = self.inputs.iter().map(|v| v.as_wast()).collect();
        let result: Vec<_> = self.output.as_wast();
        wast::core::FunctionType { params: Box::from(params), results: Box::from(result) }
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentFunctionParam<'i>> for WasmParameter
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentFunctionParam<'i> {
        ComponentFunctionParam { name: self.name.as_ref(), ty: self.type_hint.as_wast() }
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentFunctionResult<'i>> for WasmParameter
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentFunctionResult<'i> {
        ComponentFunctionResult { name: Some(self.name.as_ref()), ty: self.type_hint.as_wast() }
    }
}

impl<'a, 'i> IntoWasm<'a, (Option<Id<'a>>, Option<NameAnnotation<'a>>, ValType<'a>)> for WasmParameter
where
    'a: 'i,
{
    fn as_wast(&'a self) -> (Option<Id<'a>>, Option<NameAnnotation<'a>>, ValType<'a>) {
        (self.name.as_id(), None, self.type_hint.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, ValType<'a>> for WasmParameter
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ValType<'a> {
        self.type_hint.as_wast()
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::FuncKind<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FuncKind<'i> {
        let locals: Vec<_> = self.local.values().map(|p| p.as_wast()).collect();
        wast::core::FuncKind::Inline { locals: Box::from(locals), expression: self.body.as_wast() }
    }
}

impl<'a, 'i> IntoWasm<'a, Local<'i>> for WasmParameter
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Local<'i> {
        Local { id: self.name.as_id(), name: None, ty: self.type_hint.as_wast() }
    }
}

impl<'a, 'i> IntoWasm<'a, Expression<'i>> for FunctionBody
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Expression<'i> {
        let mut bytecode = Vec::with_capacity(self.codes.len());
        for code in &self.codes {
            code.emit(&mut bytecode)
        }
        Expression { instrs: Box::from(bytecode), branch_hints: vec![] }
    }
}
