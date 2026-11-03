//! 宿主导入能力：加载期把符号解析为 [`HostOp`]，热路径只按枚举分派。

use crate::error::OdinRuntimeError;
use nyar_gc::{ObjectHeap, ObjectPayload};
use odin_bytecode::OdinImport;

use crate::{array_runtime, dom_runtime, output_runtime, text_runtime, value::Value};

/// 与 emitter 约定的宿主导入模块名。
pub const HOST_IMPORT_MODULE: &str = "nyar.host";

/// `nyar.host` 上已冻结的宿主操作（稠密枚举，非字符串合同）。
///
/// 结构分配与字段访问不在此枚举：由 `ObjectNew` / `FieldGet` / `FieldSet` 闭合。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOp {
    /// 打印一个 `String` 值。
    Print,
    /// 拼接两个 `String` 值。
    StringConcat,
    /// 输出一行控制台日志。
    ConsoleLog,
    /// 单参数 `println` 重载：i32 按十进制、utf8 原样输出，末尾换行，返回传入值。
    Println,
    /// 有符号 32 位转 64 位。
    I32ToI64,
    /// 32 位有符号整数转 64 位浮点（IEEE-754，`i32` 全域精确）。
    I32ToF64,
    /// 64 位有符号整数转 64 位浮点（超过 2^53 的奇数按 IEEE-754 就近舍入）。
    I64ToF64,
    /// 64 位浮点加法（严格 IEEE-754，不 trap：溢出得 ±inf）。
    F64Add,
    /// 64 位浮点减法（严格 IEEE-754，不 trap）。
    F64Sub,
    /// 64 位浮点乘法（严格 IEEE-754，不 trap）。
    F64Mul,
    /// 64 位浮点除法（严格 IEEE-754，不 trap：除零得 ±inf 或 NaN）。
    F64Div,
    /// 64 位浮点取负（翻转符号位，保留 NaN 载荷）。
    F64Neg,
    /// 64 位浮点转 32 位整数（饱和，NaN 转 0，与 wasm `trunc_sat` 同口径）。
    F64ToI32,
    /// 64 位浮点转 64 位整数（饱和，NaN 转 0，与 wasm `trunc_sat` 同口径）。
    F64ToI64,
    /// 64 位浮点相等（IEEE-754：NaN 比较为 false，`+0.0` 与 `-0.0` 相等）。
    F64Eq,
    /// 64 位浮点不等（IEEE-754：NaN 与任何值不等，含 NaN 自身）。
    F64Ne,
    /// 64 位浮点小于（IEEE-754：NaN 比较为 false）。
    F64Lt,
    /// 64 位浮点小于等于（IEEE-754：NaN 比较为 false）。
    F64Le,
    /// 64 位浮点大于（IEEE-754：NaN 比较为 false）。
    F64Gt,
    /// 64 位浮点大于等于（IEEE-754：NaN 比较为 false）。
    F64Ge,
    /// UTF-8 字节长度（不做任何码点计算）。
    Utf8LengthBytes,
    /// UTF-8 码点计数（astral 字符算 1，组合附加符按各自码点计数）。
    Utf8LengthChars,
    /// UTF-8 按码点下标取半开区间 `[start, end)`；越界结构化失败，不 clamp。
    Utf8Slice,
    /// UTF-8 原样拼接（保留两侧内容，不做规范化）。
    Utf8Concat,
    /// UTF-8 按码点序列相等判定（不做 Unicode 规范化）。
    Utf8Eq,
    /// UTF-8 按码点序列不等判定。
    Utf8Ne,
    /// UTF-8 转 UTF-16：astral 码点产出代理对。
    Utf8ToUtf16,
    /// UTF-16 转 UTF-8：孤立代理结构化失败，不替换成 U+FFFD。
    Utf16ToUtf8,
    /// UTF-16 码元计数（代理对算 2，不是码点数）。
    Utf16Length,
    /// UTF-8 转 C 字符串：末尾补终止 NUL，内嵌 NUL 拒绝。
    Utf8ToCStr,
    /// C 字符串转 UTF-8：按首个 NUL 终止取内容，缺 NUL 或非法 UTF-8 结构化失败。
    CStrToUtf8,
    /// UTF-16 转 C 字符串：先按代理对规则转 UTF-8，再补终止 NUL。
    Utf16ToCStr,
    /// C 字符串转 UTF-16：按首个 NUL 取内容后编码为码元（astral 产出代理对）。
    CStrToUtf16,
    /// C 字符串内容字节长度（终止 NUL 不计）。
    CStrLength,
    /// 有符号 32 位整除。
    I32Div,
    /// 无符号 32 位整除（`u32` 的 `/`；与有符号 `I32Div` 在最高位为 1 时结论不同）。
    I32DivU,
    /// 无符号 32 位取余（`u32` 的 `%`）。
    I32RemU,
    /// 64 位有符号加法。
    I64Add,
    /// 64 位有符号减法。
    I64Sub,
    /// 64 位有符号乘法。
    I64Mul,
    /// 64 位有符号整除。
    I64Div,
    /// 64 位有符号取余。
    I64Rem,
    /// 无符号 64 位整除（`u64` 的 `/`；与有符号 `I64Div` 在最高位为 1 时结论不同）。
    I64DivU,
    /// 无符号 64 位取余（`u64` 的 `%`）。
    I64RemU,
    /// 64 位有符号取负。
    I64Neg,
    /// 64 位有符号相等比较。
    I64Eq,
    /// 64 位有符号不等比较。
    I64Ne,
    /// 64 位有符号小于比较。
    I64Lt,
    /// 64 位有符号小于等于比较。
    I64Le,
    /// 64 位有符号大于比较。
    I64Gt,
    /// 64 位有符号大于等于比较。
    I64Ge,
    /// 64 位无符号小于比较（`u64` 的 `<`；与有符号 `I64Lt` 在最高位为 1 时结论不同）。
    I64LtU,
    /// 64 位按位与。
    I64And,
    /// 64 位按位或。
    I64Or,
    /// 64 位按位异或。
    I64Xor,
    /// 64 位左移（位移量按 `& 63` 截断，与 wasm/JVM/MSIL 掩码语义一致）。
    I64Shl,
    /// 64 位算术右移（位移量按 `& 63` 截断，与 wasm `i64.shr_s`/JVM `lshr`/MSIL `shr` 一致）。
    I64Shr,
    /// 64 位逻辑右移（位移量按 `& 63` 截断，与 wasm `i64.shr_u`/JVM `lushr`/MSIL `shr.un` 一致）。
    I64ShrU,
    /// 布尔取反。
    BoolNot,
    /// 布尔与。
    BoolAnd,
    /// 布尔或。
    BoolOr,
    /// 进入业务阶段（参数：阶段名 `String`，可选 `i32` 暂停预算毫秒）。
    BeginPhase,
    /// 结束业务阶段（可选参数：阶段名 `String`；缺省弹出栈顶）。
    EndPhase,
    /// 按 `i32` 长度分配元素槽全为 `Null` 的固定数组布局对象。
    ArrayNew,
    /// 布局对象的元素槽数量（参数：数组对象）。
    ArrayLength,
    /// 按下标读取元素槽（参数：数组对象、下标）。
    ArrayGet,
    /// 按下标写入元素槽（参数：数组对象、下标、值）。
    ArraySet,
    /// `asgard.render` 合同：创建元素节点（参数：标签名 `String`）。
    DomCreateElement,
    /// `asgard.render` 合同：创建文本节点（参数：文本 `String`）。
    DomCreateText,
    /// `asgard.render` 合同：追加子节点（参数：父元素、子节点）。
    DomAppend,
    /// `asgard.render` 合同：设置元素属性（参数：元素、名 `String`、值 `String`）。
    DomSetAttr,
    /// `asgard.render` 合同：读取元素属性（参数：元素、名 `String`）。
    DomGetAttr,
    /// `asgard.render` 合同：设置元素文本内容（参数：元素、文本 `String`）。
    DomSetText,
}

/// 加载期解析后的导入槽。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedImport {
    /// `nyar.host` 内建操作。
    Host(HostOp),
    /// 非宿主模块：热路径不得再按符号名猜；须由宿主按 import 下标另行绑定。
    External,
}

/// 将导入表项解析为 [`ResolvedImport`]（仅加载 / verify 期调用）。
pub fn resolve_import(import: &OdinImport) -> Result<ResolvedImport, OdinRuntimeError> {
    if import.module_name == HOST_IMPORT_MODULE {
        let op = parse_host_op(&import.symbol_name)
            .ok_or_else(|| OdinRuntimeError::ModuleLoad(format!("unknown host import `{HOST_IMPORT_MODULE}::{}`", import.symbol_name)))?;
        Ok(ResolvedImport::Host(op))
    }
    else {
        Ok(ResolvedImport::External)
    }
}

/// 加载期符号 → [`HostOp`]；未知符号返回 `None`。
pub fn parse_host_op(symbol: &str) -> Option<HostOp> {
    Some(match symbol {
        "print" => HostOp::Print,
        "string_concat" => HostOp::StringConcat,
        "console_log" => HostOp::ConsoleLog,
        "println" => HostOp::Println,
        "i32_to_i64" => HostOp::I32ToI64,
        "i32_to_f64" => HostOp::I32ToF64,
        "i64_to_f64" => HostOp::I64ToF64,
        "f64_add" => HostOp::F64Add,
        "f64_sub" => HostOp::F64Sub,
        "f64_mul" => HostOp::F64Mul,
        "f64_div" => HostOp::F64Div,
        "f64_neg" => HostOp::F64Neg,
        "f64_to_i32" => HostOp::F64ToI32,
        "f64_to_i64" => HostOp::F64ToI64,
        "f64_eq" => HostOp::F64Eq,
        "f64_ne" => HostOp::F64Ne,
        "f64_lt" => HostOp::F64Lt,
        "f64_le" => HostOp::F64Le,
        "f64_gt" => HostOp::F64Gt,
        "f64_ge" => HostOp::F64Ge,
        "utf8.length_bytes" => HostOp::Utf8LengthBytes,
        "utf8.length_chars" => HostOp::Utf8LengthChars,
        "utf8.slice" => HostOp::Utf8Slice,
        "utf8.concat" => HostOp::Utf8Concat,
        "utf8.eq" => HostOp::Utf8Eq,
        "utf8.ne" => HostOp::Utf8Ne,
        "utf8_to_utf16" => HostOp::Utf8ToUtf16,
        "utf16_to_utf8" => HostOp::Utf16ToUtf8,
        "utf16_length" => HostOp::Utf16Length,
        "utf8_to_c_str" => HostOp::Utf8ToCStr,
        "c_str_to_utf8" => HostOp::CStrToUtf8,
        "utf16_to_c_str" => HostOp::Utf16ToCStr,
        "c_str_to_utf16" => HostOp::CStrToUtf16,
        "c_str_length" => HostOp::CStrLength,
        "i32_div" => HostOp::I32Div,
        "i32_div_u" => HostOp::I32DivU,
        "i32_rem_u" => HostOp::I32RemU,
        "i64_add" => HostOp::I64Add,
        "i64_sub" => HostOp::I64Sub,
        "i64_mul" => HostOp::I64Mul,
        "i64_div" => HostOp::I64Div,
        "i64_rem" => HostOp::I64Rem,
        "i64_div_u" => HostOp::I64DivU,
        "i64_rem_u" => HostOp::I64RemU,
        "i64_neg" => HostOp::I64Neg,
        "i64_eq" => HostOp::I64Eq,
        "i64_ne" => HostOp::I64Ne,
        "i64_lt" => HostOp::I64Lt,
        "i64_le" => HostOp::I64Le,
        "i64_gt" => HostOp::I64Gt,
        "i64_ge" => HostOp::I64Ge,
        "i64_lt_u" => HostOp::I64LtU,
        "i64_and" => HostOp::I64And,
        "i64_or" => HostOp::I64Or,
        "i64_xor" => HostOp::I64Xor,
        "i64_shl" => HostOp::I64Shl,
        "i64_shr" => HostOp::I64Shr,
        "i64_shr_u" => HostOp::I64ShrU,
        "bool_not" => HostOp::BoolNot,
        "bool_and" => HostOp::BoolAnd,
        "bool_or" => HostOp::BoolOr,
        "begin_phase" => HostOp::BeginPhase,
        "end_phase" => HostOp::EndPhase,
        "array.new" => HostOp::ArrayNew,
        "array.length" => HostOp::ArrayLength,
        "array.get" => HostOp::ArrayGet,
        "array.set" => HostOp::ArraySet,
        "dom_create_element" => HostOp::DomCreateElement,
        "dom_create_text" => HostOp::DomCreateText,
        "dom_append" => HostOp::DomAppend,
        "dom_set_attr" => HostOp::DomSetAttr,
        "dom_get_attr" => HostOp::DomGetAttr,
        "dom_set_text" => HostOp::DomSetText,
        _ => return None,
    })
}

/// 执行已解析的宿主操作（热路径无符号字符串）。
pub fn execute_host_op(op: HostOp, args: &[Value], heap: &mut ObjectHeap) -> Result<Value, OdinRuntimeError> {
    match op {
        HostOp::Print => {
            let text = args.iter().map(value_display).collect::<Vec<_>>().join("\t");
            output_runtime::emit_line(&text);
            Ok(args.last().cloned().unwrap_or(Value::Null))
        }
        HostOp::StringConcat => {
            let left = value_display(args.first().ok_or(OdinRuntimeError::StackUnderflow)?);
            let right = value_display(args.get(1).ok_or(OdinRuntimeError::StackUnderflow)?);
            Ok(Value::String(format!("{left}{right}")))
        }
        HostOp::ConsoleLog => {
            let value = args.first().ok_or(OdinRuntimeError::StackUnderflow)?;
            output_runtime::emit_line(&value.to_string());
            Ok(Value::Null)
        }
        HostOp::Println => match args.first() {
            Some(Value::I32(value)) => {
                output_runtime::emit_line(&value.to_string());
                Ok(Value::I32(*value))
            }
            Some(Value::String(value)) => {
                output_runtime::emit_line(value);
                Ok(Value::String(value.clone()))
            }
            Some(other) => Err(OdinRuntimeError::TypeMismatch { expected: "i32 or utf8", actual: other.type_name().to_string() }),
            None => Err(OdinRuntimeError::StackUnderflow),
        },
        HostOp::BeginPhase => {
            let phase = match args.first() {
                Some(Value::String(name)) if !name.is_empty() => name.clone(),
                Some(other) => {
                    return Err(OdinRuntimeError::TypeMismatch {
                        expected: "non-empty string phase name",
                        actual: other.type_name().to_string(),
                    });
                }
                None => {
                    return Err(OdinRuntimeError::UnsupportedFeature("begin_phase requires a phase name string argument".into()));
                }
            };
            let pause_budget_ms = match args.get(1) {
                Some(Value::I32(value)) if *value >= 0 => Some(*value as u32),
                Some(Value::I64(value)) if *value >= 0 && *value <= u32::MAX as i64 => Some(*value as u32),
                Some(other) => {
                    return Err(OdinRuntimeError::TypeMismatch {
                        expected: "non-negative i32 pause_budget_ms",
                        actual: other.type_name().to_string(),
                    });
                }
                None => None,
            };
            let mut intent = nyar_gc::WorkloadIntent::empty(nyar_gc::IntentSource::PhaseEvent);
            intent.phase = Some(phase);
            intent.pause_budget_ms = pause_budget_ms;
            heap.begin_phase(intent).map_err(|error| OdinRuntimeError::ModuleLoad(error.to_string()))?;
            Ok(Value::Null)
        }
        HostOp::EndPhase => {
            let phase = match args.first() {
                Some(Value::String(name)) => Some(name.as_str()),
                Some(other) => {
                    return Err(OdinRuntimeError::TypeMismatch { expected: "string phase name", actual: other.type_name().to_string() });
                }
                None => None,
            };
            heap.end_phase(phase).map_err(|error| OdinRuntimeError::ModuleLoad(error.to_string()))?;
            Ok(Value::Null)
        }
        HostOp::I32ToI64 => match args.first() {
            Some(Value::I32(value)) => Ok(Value::I64(*value as i64)),
            Some(Value::I64(value)) => Ok(Value::I64(*value)),
            Some(other) => Err(OdinRuntimeError::TypeMismatch { expected: "i32", actual: other.type_name().to_string() }),
            None => Err(OdinRuntimeError::StackUnderflow),
        },
        // ---- 浮点：严格 IEEE-754，不 trap；转换统一「饱和 + NaN→0」 ----
        HostOp::I32ToF64 => match args.first() {
            Some(Value::I32(value)) => Ok(Value::F64(*value as f64)),
            Some(other) => Err(OdinRuntimeError::TypeMismatch { expected: "i32", actual: other.type_name().to_string() }),
            None => Err(OdinRuntimeError::StackUnderflow),
        },
        HostOp::I64ToF64 => Ok(Value::F64(i64_arg(args, 0)? as f64)),
        HostOp::F64Add => Ok(Value::F64(f64_arg(args, 0)? + f64_arg(args, 1)?)),
        HostOp::F64Sub => Ok(Value::F64(f64_arg(args, 0)? - f64_arg(args, 1)?)),
        HostOp::F64Mul => Ok(Value::F64(f64_arg(args, 0)? * f64_arg(args, 1)?)),
        // 除零不报错：IEEE-754 下 1/0 = +inf、-1/0 = -inf、0/0 = NaN。
        HostOp::F64Div => Ok(Value::F64(f64_arg(args, 0)? / f64_arg(args, 1)?)),
        HostOp::F64Neg => Ok(Value::F64(-f64_arg(args, 0)?)),
        HostOp::F64ToI32 => Ok(Value::I32(f64_to_i32_saturating(f64_arg(args, 0)?))),
        HostOp::F64ToI64 => Ok(Value::I64(f64_to_i64_saturating(f64_arg(args, 0)?))),
        // 比较直接复用 Rust `f64` 运算符：NaN 参与的有序比较为 false，`!=` 为 true，
        // `==` 对 `+0.0`/`-0.0` 为 true（IEEE-754 相等，不是按位比较）。
        HostOp::F64Eq => Ok(Value::Bool(f64_arg(args, 0)? == f64_arg(args, 1)?)),
        HostOp::F64Ne => Ok(Value::Bool(f64_arg(args, 0)? != f64_arg(args, 1)?)),
        HostOp::F64Lt => Ok(Value::Bool(f64_arg(args, 0)? < f64_arg(args, 1)?)),
        HostOp::F64Le => Ok(Value::Bool(f64_arg(args, 0)? <= f64_arg(args, 1)?)),
        HostOp::F64Gt => Ok(Value::Bool(f64_arg(args, 0)? > f64_arg(args, 1)?)),
        HostOp::F64Ge => Ok(Value::Bool(f64_arg(args, 0)? >= f64_arg(args, 1)?)),
        // ---- UTF-8 文本：索引空间是码点，越界结构化失败 ----
        HostOp::Utf8LengthBytes => Ok(Value::I32(text_runtime::length_bytes(utf8_arg(args, 0, "utf8")?))),
        HostOp::Utf8LengthChars => Ok(Value::I32(text_runtime::length_chars(utf8_arg(args, 0, "utf8")?))),
        HostOp::Utf8Slice => {
            let text = utf8_arg(args, 0, "utf8")?;
            let start = i32_arg(args, 1)? as i64;
            let count = i32_arg(args, 2)? as i64;
            Ok(Value::String(text_runtime::slice(text, start, count, "utf8.slice")?))
        }
        HostOp::Utf8Concat => {
            let left = utf8_arg(args, 0, "utf8")?;
            let right = utf8_arg(args, 1, "utf8")?;
            Ok(Value::String(text_runtime::concat(left, right)))
        }
        HostOp::Utf8Eq => Ok(Value::Bool(text_runtime::equals(utf8_arg(args, 0, "utf8")?, utf8_arg(args, 1, "utf8")?))),
        HostOp::Utf8Ne => Ok(Value::Bool(!text_runtime::equals(utf8_arg(args, 0, "utf8")?, utf8_arg(args, 1, "utf8")?))),
        HostOp::Utf8ToUtf16 => {
            let units = text_runtime::to_utf16(utf8_arg(args, 0, "utf8")?);
            Ok(build_u16_array(heap, &units))
        }
        HostOp::Utf16ToUtf8 => {
            let units = u16_array_arg(heap, args, 0)?;
            Ok(Value::String(text_runtime::from_utf16(&units, "utf16_to_utf8")?))
        }
        HostOp::Utf16Length => Ok(Value::I32(text_runtime::utf16_length(&u16_array_arg(heap, args, 0)?))),
        HostOp::Utf8ToCStr => {
            let bytes = text_runtime::to_c_str(utf8_arg(args, 0, "utf8")?, "utf8_to_c_str")?;
            Ok(build_byte_array(heap, &bytes))
        }
        HostOp::CStrToUtf8 => {
            let bytes = byte_array_arg(heap, args, 0)?;
            Ok(Value::String(text_runtime::c_str_to_utf8(&bytes, "c_str_to_utf8")?))
        }
        HostOp::Utf16ToCStr => {
            let units = u16_array_arg(heap, args, 0)?;
            let bytes = text_runtime::utf16_to_c_str(&units, "utf16_to_c_str")?;
            Ok(build_byte_array(heap, &bytes))
        }
        HostOp::CStrToUtf16 => {
            let bytes = byte_array_arg(heap, args, 0)?;
            let units = text_runtime::c_str_to_utf16(&bytes, "c_str_to_utf16")?;
            Ok(build_u16_array(heap, &units))
        }
        HostOp::CStrLength => Ok(Value::I32(text_runtime::c_str_length(&byte_array_arg(heap, args, 0)?, "c_str_length")?)),
        HostOp::I32Div => {
            let lhs = i32_arg(args, 0)?;
            let rhs = i32_arg(args, 1)?;
            if rhs == 0 {
                return Err(OdinRuntimeError::DivisionByZero { operation: "host i32_div" });
            }
            lhs.checked_div(rhs).map(Value::I32).ok_or(OdinRuntimeError::ArithmeticOverflow { operation: "host i32_div" })
        }
        HostOp::I32DivU => {
            let lhs = i32_arg(args, 0)?;
            let rhs = i32_arg(args, 1)?;
            if rhs == 0 {
                return Err(OdinRuntimeError::DivisionByZero { operation: "host i32_div_u" });
            }
            Ok(Value::I32((lhs as u32 / rhs as u32) as i32))
        }
        HostOp::I32RemU => {
            let lhs = i32_arg(args, 0)?;
            let rhs = i32_arg(args, 1)?;
            if rhs == 0 {
                return Err(OdinRuntimeError::DivisionByZero { operation: "host i32_rem_u" });
            }
            Ok(Value::I32((lhs as u32 % rhs as u32) as i32))
        }
        HostOp::I64Add => Ok(Value::I64(i64_arg(args, 0)? + i64_arg(args, 1)?)),
        HostOp::I64Sub => Ok(Value::I64(i64_arg(args, 0)? - i64_arg(args, 1)?)),
        HostOp::I64Mul => Ok(Value::I64(i64_arg(args, 0)? * i64_arg(args, 1)?)),
        HostOp::I64Div => {
            let lhs = i64_arg(args, 0)?;
            let rhs = i64_arg(args, 1)?;
            if rhs == 0 {
                return Err(OdinRuntimeError::DivisionByZero { operation: "host i64_div" });
            }
            // `i64::MIN / -1` 在 Rust 是溢出 panic；转成结构化错误，避免 abort 进程。
            lhs.checked_div(rhs).map(Value::I64).ok_or(OdinRuntimeError::ArithmeticOverflow { operation: "host i64_div" })
        }
        HostOp::I64Rem => {
            let lhs = i64_arg(args, 0)?;
            let rhs = i64_arg(args, 1)?;
            if rhs == 0 {
                return Err(OdinRuntimeError::DivisionByZero { operation: "host i64_rem" });
            }
            // `i64::MIN % -1` 同样溢出（数学结果为 0，但硬件整除去溢出检查）。
            lhs.checked_rem(rhs).map(Value::I64).ok_or(OdinRuntimeError::ArithmeticOverflow { operation: "host i64_rem" })
        }
        HostOp::I64DivU => {
            let lhs = i64_arg(args, 0)?;
            let rhs = i64_arg(args, 1)?;
            if rhs == 0 {
                return Err(OdinRuntimeError::DivisionByZero { operation: "host i64_div_u" });
            }
            Ok(Value::I64((lhs as u64 / rhs as u64) as i64))
        }
        HostOp::I64RemU => {
            let lhs = i64_arg(args, 0)?;
            let rhs = i64_arg(args, 1)?;
            if rhs == 0 {
                return Err(OdinRuntimeError::DivisionByZero { operation: "host i64_rem_u" });
            }
            Ok(Value::I64((lhs as u64 % rhs as u64) as i64))
        }
        HostOp::I64Neg => Ok(Value::I64(-i64_arg(args, 0)?)),
        HostOp::I64Eq => Ok(Value::Bool(i64_arg(args, 0)? == i64_arg(args, 1)?)),
        HostOp::I64Ne => Ok(Value::Bool(i64_arg(args, 0)? != i64_arg(args, 1)?)),
        HostOp::I64Lt => Ok(Value::Bool(i64_arg(args, 0)? < i64_arg(args, 1)?)),
        HostOp::I64Le => Ok(Value::Bool(i64_arg(args, 0)? <= i64_arg(args, 1)?)),
        HostOp::I64Gt => Ok(Value::Bool(i64_arg(args, 0)? > i64_arg(args, 1)?)),
        HostOp::I64Ge => Ok(Value::Bool(i64_arg(args, 0)? >= i64_arg(args, 1)?)),
        HostOp::I64LtU => Ok(Value::Bool((i64_arg(args, 0)? as u64) < (i64_arg(args, 1)? as u64))),
        HostOp::I64And => Ok(Value::I64(i64_arg(args, 0)? & i64_arg(args, 1)?)),
        HostOp::I64Or => Ok(Value::I64(i64_arg(args, 0)? | i64_arg(args, 1)?)),
        HostOp::I64Xor => Ok(Value::I64(i64_arg(args, 0)? ^ i64_arg(args, 1)?)),
        HostOp::I64Shl => Ok(Value::I64(i64_arg(args, 0)? << (i64_arg(args, 1)? & 63) as u32)),
        HostOp::I64Shr => Ok(Value::I64(i64_arg(args, 0)? >> (i64_arg(args, 1)? & 63) as u32)),
        HostOp::I64ShrU => Ok(Value::I64((i64_arg(args, 0)? as u64 >> (i64_arg(args, 1)? & 63) as u32) as i64)),
        HostOp::BoolNot => Ok(Value::Bool(!args.first().ok_or(OdinRuntimeError::StackUnderflow)?.to_bool())),
        HostOp::BoolAnd => {
            let lhs = args.first().ok_or(OdinRuntimeError::StackUnderflow)?.to_bool();
            let rhs = args.get(1).ok_or(OdinRuntimeError::StackUnderflow)?.to_bool();
            Ok(Value::Bool(lhs && rhs))
        }
        HostOp::BoolOr => {
            let lhs = args.first().ok_or(OdinRuntimeError::StackUnderflow)?.to_bool();
            let rhs = args.get(1).ok_or(OdinRuntimeError::StackUnderflow)?.to_bool();
            Ok(Value::Bool(lhs || rhs))
        }
        HostOp::ArrayNew => {
            let length = i32_arg(args, 0)?;
            if length < 0 {
                return Err(OdinRuntimeError::TypeMismatch { expected: "non-negative array length", actual: format!("i32({length})") });
            }
            let slots = vec![Value::Null; length as usize];
            Ok(array_runtime::build_fixed_array(heap, 0, &slots))
        }
        HostOp::ArrayLength => array_runtime::array_length(heap, args.first().ok_or(OdinRuntimeError::StackUnderflow)?),
        HostOp::ArrayGet => {
            let array = args.first().ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            let index = args.get(1).ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            array_runtime::array_get(heap, &array, &index)
        }
        HostOp::ArraySet => {
            let array = args.first().ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            let index = args.get(1).ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            let value = args.get(2).ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            array_runtime::array_set(heap, &array, &index, &value)
        }
        HostOp::DomCreateElement => {
            let tag = string_arg(args, 0, "dom_create_element tag")?;
            dom_runtime::create_element(heap, &tag)
        }
        HostOp::DomCreateText => {
            let text = string_arg(args, 0, "dom_create_text text")?;
            dom_runtime::create_text(heap, &text)
        }
        HostOp::DomAppend => {
            let parent = args.first().ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            let child = args.get(1).ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            dom_runtime::append(heap, &parent, &child)
        }
        HostOp::DomSetAttr => {
            let node = args.first().ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            let name = string_arg(args, 1, "dom_set_attr name")?;
            let value = string_arg(args, 2, "dom_set_attr value")?;
            dom_runtime::set_attr(heap, &node, &name, &value)
        }
        HostOp::DomGetAttr => {
            let node = args.first().ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            let name = string_arg(args, 1, "dom_get_attr name")?;
            dom_runtime::get_attr(heap, &node, &name)
        }
        HostOp::DomSetText => {
            let node = args.first().ok_or(OdinRuntimeError::StackUnderflow)?.clone();
            let text = string_arg(args, 1, "dom_set_text text")?;
            dom_runtime::set_text(heap, &node, &text)
        }
    }
}

fn value_display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::I32(value) => value.to_string(),
        Value::I64(value) => value.to_string(),
        Value::F32(value) => value.to_string(),
        Value::F64(value) => {
            if value.fract() == 0.0 && value.is_finite() {
                format!("{:.0}", value)
            }
            else {
                value.to_string()
            }
        }
        Value::String(value) => value.clone(),
        Value::Object(_) => "object".to_string(),
        Value::Coroutine(_) => "coroutine".to_string(),
    }
}

fn i64_arg(args: &[Value], index: usize) -> Result<i64, OdinRuntimeError> {
    match args.get(index) {
        Some(Value::I64(value)) => Ok(*value),
        Some(Value::I32(value)) => Ok(*value as i64),
        Some(other) => Err(OdinRuntimeError::TypeMismatch { expected: "i64", actual: other.type_name().to_string() }),
        None => Err(OdinRuntimeError::StackUnderflow),
    }
}

fn i32_arg(args: &[Value], index: usize) -> Result<i32, OdinRuntimeError> {
    match args.get(index) {
        Some(Value::I32(value)) => Ok(*value),
        Some(Value::I64(value)) => Ok(*value as i32),
        Some(other) => Err(OdinRuntimeError::TypeMismatch { expected: "i32", actual: other.type_name().to_string() }),
        None => Err(OdinRuntimeError::StackUnderflow),
    }
}

/// 严格浮点取参：只接受 `F64` 载体（不做隐式加宽）；缺参 / 其它载体一律结构化失败。
fn f64_arg(args: &[Value], index: usize) -> Result<f64, OdinRuntimeError> {
    match args.get(index) {
        Some(Value::F64(value)) => Ok(*value),
        Some(other) => Err(OdinRuntimeError::TypeMismatch { expected: "f64", actual: other.type_name().to_string() }),
        None => Err(OdinRuntimeError::StackUnderflow),
    }
}

/// 浮点转 `i32`：饱和 + NaN→0（与 wasm `i32.trunc_sat_f64_s` 同口径）。
///
/// 显式判界而不依赖 `as` 的平台行为：`NaN` → 0、`+inf`/超上界 → `i32::MAX`、
/// `-inf`/超下界 → `i32::MIN`，有限值朝零截断。
fn f64_to_i32_saturating(value: f64) -> i32 {
    if value.is_nan() {
        return 0;
    }
    let truncated = value.trunc();
    if truncated <= i32::MIN as f64 {
        i32::MIN
    }
    else if truncated >= i32::MAX as f64 {
        i32::MAX
    }
    else {
        truncated as i32
    }
}

/// 浮点转 `i64`：饱和 + NaN→0（与 wasm `i64.trunc_sat_f64_s` 同口径）。
///
/// `i64::MAX as f64` 实际是 2^63，超出 i64 表示范围，不能直接当上界比较，
/// 因此用 2^63 作为开区间上界、`-2^63` 作为闭区间下界显式判界。
fn f64_to_i64_saturating(value: f64) -> i64 {
    if value.is_nan() {
        return 0;
    }
    let truncated = value.trunc();
    const I64_MIN_AS_F64: f64 = -9_223_372_036_854_775_808.0;
    const I64_MAX_PLUS_ONE_AS_F64: f64 = 9_223_372_036_854_775_808.0;
    if truncated <= I64_MIN_AS_F64 {
        i64::MIN
    }
    else if truncated >= I64_MAX_PLUS_ONE_AS_F64 {
        i64::MAX
    }
    else {
        truncated as i64
    }
}

/// 严格 UTF-16 取参：只接受码元固定数组（`Value::I32` 槽位，0..=0xFFFF）。
fn u16_array_arg(heap: &ObjectHeap, args: &[Value], index: usize) -> Result<Vec<u16>, OdinRuntimeError> {
    let value = args.get(index).ok_or(OdinRuntimeError::StackUnderflow)?;
    let Value::Object(object_id) = value
    else {
        return Err(OdinRuntimeError::TypeMismatch { expected: "utf16 array", actual: value.type_name().to_string() });
    };
    let slots = match heap.get(*object_id) {
        Some(ObjectPayload::LayoutObject { slots, .. }) => slots,
        Some(_) => {
            return Err(OdinRuntimeError::TypeMismatch { expected: "utf16 array", actual: "non-layout object".to_string() });
        }
        None => return Err(OdinRuntimeError::ModuleLoad(format!("invalid object id {object_id}"))),
    };
    slots
        .iter()
        .map(|element| match element {
            Value::I32(unit) if (0..=0xFFFF).contains(unit) => Ok(*unit as u16),
            other => Err(OdinRuntimeError::TypeMismatch { expected: "utf16 code unit", actual: other.type_name().to_string() }),
        })
        .collect()
}

fn build_u16_array(heap: &mut ObjectHeap, units: &[u16]) -> Value {
    let elements = units.iter().map(|unit| Value::I32(*unit as i32)).collect::<Vec<_>>();
    array_runtime::build_fixed_array(heap, 0, &elements)
}

/// 严格 C 字符串取参：只接受字节固定数组（`Value::I32` 槽位，0..=255）。
fn byte_array_arg(heap: &ObjectHeap, args: &[Value], index: usize) -> Result<Vec<u8>, OdinRuntimeError> {
    let value = args.get(index).ok_or(OdinRuntimeError::StackUnderflow)?;
    let Value::Object(object_id) = value
    else {
        return Err(OdinRuntimeError::TypeMismatch { expected: "c_str byte array", actual: value.type_name().to_string() });
    };
    let slots = match heap.get(*object_id) {
        Some(ObjectPayload::LayoutObject { slots, .. }) => slots,
        Some(_) => {
            return Err(OdinRuntimeError::TypeMismatch { expected: "c_str byte array", actual: "non-layout object".to_string() });
        }
        None => return Err(OdinRuntimeError::ModuleLoad(format!("invalid object id {object_id}"))),
    };
    slots
        .iter()
        .map(|element| match element {
            Value::I32(byte) if (0..=0xFF).contains(byte) => Ok(*byte as u8),
            other => Err(OdinRuntimeError::TypeMismatch { expected: "c_str byte", actual: other.type_name().to_string() }),
        })
        .collect()
}

fn build_byte_array(heap: &mut ObjectHeap, bytes: &[u8]) -> Value {
    let elements = bytes.iter().map(|byte| Value::I32(*byte as i32)).collect::<Vec<_>>();
    array_runtime::build_fixed_array(heap, 0, &elements)
}

/// 严格 UTF-8 取参：只接受 `Value::String`；缺参 / 其它载体一律结构化失败。
fn utf8_arg<'a>(args: &'a [Value], index: usize, what: &'static str) -> Result<&'a str, OdinRuntimeError> {
    match args.get(index) {
        Some(Value::String(value)) => Ok(value.as_str()),
        Some(other) => Err(OdinRuntimeError::TypeMismatch { expected: what, actual: other.type_name().to_string() }),
        None => Err(OdinRuntimeError::StackUnderflow),
    }
}

fn string_arg(args: &[Value], index: usize, what: &'static str) -> Result<String, OdinRuntimeError> {
    match args.get(index) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(other) => Err(OdinRuntimeError::TypeMismatch { expected: what, actual: other.type_name().to_string() }),
        None => Err(OdinRuntimeError::StackUnderflow),
    }
}
