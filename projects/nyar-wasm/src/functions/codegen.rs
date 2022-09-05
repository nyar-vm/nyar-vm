use super::*;
use wast::{
    component::{CoreItemRef, ItemRef},
    core::Local,
    kw,
};

impl<'a, 'i> IntoWasm<'a, Start<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Start<'i> {
        Start { func: self.symbol.as_index(), args: vec![], results: vec![] }
    }
}

impl<'a, 'i> IntoWasm<'a, Func<'i>> for FunctionType
where
    'a: 'i,
{
    /// 怎么把核心函数提升到组件函数
    fn as_wast(&'a self) -> Func<'i> {
        Func {
            span: Span::from_offset(0),
            id: self.symbol.as_id(),
            name: None,
            exports: self.export.as_wast(),
            kind: self.as_wast(),
        }
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
            ty: TypeUse { index: None, inline: Some(self.as_wast()) },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, FuncKind<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> FuncKind<'i> {
        FuncKind::Lift { ty: ComponentTypeUse::Inline(self.as_wast()), info: self.as_wast() }
    }
}
impl<'a, 'i> IntoWasm<'a, CoreFuncKind<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CoreFuncKind<'i> {
        CoreFuncKind::Lower(CanonLower {
            func: ItemRef { kind: kw::func(Span::from_offset(0)), idx: self.symbol.as_index(), export_names: Vec::new() },
            opts: Vec::new(),
        })
    }
}
impl<'a, 'i> IntoWasm<'a, CanonLift<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CanonLift<'i> {
        CanonLift {
            func: CoreItemRef { kind: kw::func(Span::from_offset(0)), idx: self.symbol.as_index(), export_name: None },
            opts: Vec::new(),
        }
    }
}
impl<'a, 'i> IntoWasm<'a, ComponentFunctionType<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentFunctionType<'i> {
        let params: Vec<_> = self.input.values().map(|v| v.as_wast()).collect();
        let result: Vec<_> = self.output.iter().map(|v| v.as_wast()).collect();
        ComponentFunctionType { params: Box::from(params), results: Box::from(result) }
    }
}
impl<'a, 'i> IntoWasm<'a, wast::core::FunctionType<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FunctionType<'i> {
        let params: Vec<_> = self.input.values().map(|v| v.as_wast()).collect();
        let result: Vec<_> = self.output.iter().map(|v| v.as_wast()).collect();
        wast::core::FunctionType { params: Box::from(params), results: Box::from(result) }
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentFunctionParam<'i>> for ParameterType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentFunctionParam<'i> {
        ComponentFunctionParam { name: self.name.as_ref(), ty: self.type_hint.as_wast() }
    }
}

impl<'a, 'i> IntoWasm<'a, (Option<Id<'a>>, Option<NameAnnotation<'a>>, ValType<'a>)> for ParameterType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> (Option<Id<'a>>, Option<NameAnnotation<'a>>, ValType<'a>) {
        (self.name.as_id(), None, self.type_hint.as_wast())
    }
}

impl<'a, 'i> IntoWasm<'a, ComponentFunctionResult<'i>> for WasmType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentFunctionResult<'i> {
        ComponentFunctionResult { name: None, ty: self.as_wast() }
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
impl<'a, 'i> IntoWasm<'a, Local<'i>> for ParameterType
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
        Expression { instrs: Box::from(bytecode) }
    }
}
