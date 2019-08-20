use crate::NyarValue;
use nyar_error::Result;
use std::{
    any::{type_name, Any, TypeId},
    collections::BTreeMap,
    sync::Arc,
};
use FunctionTable::{Dispatch, Overload, Principal};

pub trait NyarFunction {
    fn get_name(&self) -> &str;
    fn get_principal(types: &[NyarType]) -> Option<Arc<dyn FunctionObject>>;
}

impl NyarFunction for CustomFunction {
    fn get_name(&self) -> &str {
        self.name.as_str()
    }
}

pub struct CustomFunction {
    name: String,
}

pub enum FunctionTable {
    Principal(Arc<dyn FunctionObject>),
    Overload(BTreeMap<usize, Arc<dyn FunctionObject>>),
    Dispatch(BTreeMap<usize, Arc<dyn FunctionObject>>),
}

impl FunctionTable {
    /// 获取主函数
    pub fn get_principal(&self, types: &[NyarType]) -> Option<Arc<dyn FunctionObject>> {
        match self {
            Principal(f) => {
                if f.match_typing(&types) {
                    return Some(f.clone());
                }
            }
            Overload(table) | Dispatch(table) => {
                for (_, f) in table {
                    if f.match_typing(&types) {
                        return Some(f.clone());
                    }
                }
            }
        }
        None
    }
    pub fn as_overload(&mut self) {
        match self {
            Principal(s) => *self = Overload(Self::table(s.clone(), 0)),
            Overload(_) => {}
            Dispatch(s) => *self = Overload(s.clone()),
        };
    }
    pub fn as_dispatch(&mut self) {
        match self {
            Principal(s) => *self = Dispatch(Self::table(s.clone(), 0)),
            Overload(s) => *self = Dispatch(s.clone()),
            Dispatch(_) => {}
        };
    }
    pub fn table(f: Arc<dyn FunctionObject>, priority: usize) -> BTreeMap<usize, Arc<dyn FunctionObject>> {
        let mut out = BTreeMap::new();
        out.insert(priority, f);
        return out;
    }
}

pub enum NyarType {
    Boolean,
}

pub struct FunctionInput {}

pub struct FunctionContext {}

pub trait FunctionObject {
    fn match_typing(&self, types: &[NyarType]) -> bool;
    fn apply(&self, input: FunctionInput, ctx: FunctionContext) -> Result<NyarValue>;
    /// 参数不足不报错
    fn is_curry(&self) -> bool {
        false
    }
    fn is_lambda(&self) -> bool {
        false
    }
    fn is_native(&self) -> bool {
        false
    }
}

/// 不可重载
pub struct NativeFunction {
    name: String,
    typing: Vec<NyarType>,
    typing_ret: NyarType,
    arg_types: AsRef<[TypeId]>,
    function: fn(FunctionInput) -> Result<NyarValue>,
}

///
fn is_true(this: bool) -> bool {
    matches!(this, true)
}
