use super::*;
use wast::{
    component::{
        CanonLift, CanonLower, ComponentExternName, ComponentFunctionParam, ComponentFunctionResult, ComponentFunctionType,
        ComponentTypeUse, CoreFunc, CoreFuncKind, CoreItemRef, Func, FuncKind, InlineExport, Start,
    },
    core::{Expression, ModuleField, TypeUse},
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
    /// 定义函数接口
    fn as_wast(&'a self) -> Func<'i> {
        Func { span: Span::from_offset(0), id: self.symbol.as_id(), name: None, exports: self.as_wast(), kind: self.as_wast() }
    }
}
impl<'a, 'i> IntoWasm<'a, CoreFunc<'i>> for FunctionType
where
    'a: 'i,
{
    /// 定义函数实现
    fn as_wast(&'a self) -> CoreFunc<'i> {
        CoreFunc { span: Span::from_offset(0), id: self.symbol.as_id(), name: None, kind: self.as_wast() }
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
        CoreFuncKind::Lower(CanonLower::default())
    }
}
impl<'a, 'i> IntoWasm<'a, CanonLift<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> CanonLift<'i> {
        CanonLift::default()
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
impl<'a, 'i> IntoWasm<'a, ComponentFunctionParam<'i>> for ParameterType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ComponentFunctionParam<'i> {
        ComponentFunctionParam { name: self.name.as_ref(), ty: self.type_hint.as_wast() }
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

impl<'a, 'i> IntoWasm<'a, InlineExport<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> InlineExport<'i> {
        match &self.export {
            Some(s) => InlineExport { names: vec![ComponentExternName(s.as_ref())] },
            None => InlineExport { names: vec![] },
        }
    }
}

impl<'a, 'i> IntoWasm<'a, ModuleField<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> ModuleField<'i> {
        ModuleField::Func(wast::core::Func {
            span: Span::from_offset(0),
            id: self.symbol.as_id(),
            name: None,
            exports: Default::default(),
            kind: self.as_wast(),
            ty: TypeUse { index: None, inline: Some(self.as_wast()) },
        })
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::FunctionType<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FunctionType<'i> {
        wast::core::FunctionType { params: Box::new([]), results: Box::new([]) }
    }
}

impl<'a, 'i> IntoWasm<'a, wast::core::FuncKind<'i>> for FunctionType
where
    'a: 'i,
{
    fn as_wast(&'a self) -> wast::core::FuncKind<'i> {
        wast::core::FuncKind::Inline { locals: Box::new([]), expression: Expression { instrs: Box::new([]) } }
    }
}
