use std::fmt::{self, Display};
use std::str::FromStr;
use bigdecimal::BigDecimal;
use gc::{Gc, GcCell};
use num::traits::FromPrimitive;
use num::BigInt;

use crate::value::function::Function;
use crate::value::object::ObjectData;

use super::number::Number as NyarNumber;

/// 受 Garbage Collector 管理的值
pub type Value = Gc<ValueData>;

/// Nyar 7 大原生核心类型:
/// - Boolean/String/Integer/Decimal/Function/Object
/// - 以及 Type 类型本身
#[derive(Trace, Finalize, Debug, Clone)]
pub enum ValueData {
    /// `Boolean` - 真值类型 `true` / `false`
    Boolean(bool),
    /// `String` - 字符串类型
    String(String),
    /// `Number` - 高精度整数类型
    Number(NyarNumber),
    /// `Object` - 对象模型
    Object(GcCell<ObjectData>),
    /// `Function` - 函数类型
    Function(Box<GcCell<Function>>),
    /// `Type` - 特殊类型/高阶类型
    Type(GcCell<ObjectData>), //TODO: TypeData
}
