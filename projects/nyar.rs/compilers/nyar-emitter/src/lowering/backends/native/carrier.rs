//! Node-API / CPython 宿主承载服务（target-lane 私有）。
//!
//! 宿主扩展的包装体构造（注册入口、方法回调、模块定义与 `.data` 布局、文本
//! arena 合同、宿主导入面）是**目标侧承载能力**：语言适配器只提交已冻结的
//! 导出事实（[`HostProperty`] / [`HostMethod`]：符号、整数形参个数、类型化返回
//! 载体）与模块名，不各自复制一套宿主 ABI。
//!
//! 本模块**不要求任何特定函数名**：残差语言把 `main` 暴露成属性 `value` 是
//! 适配器自己的约定，以 [`HostProperty`] 提交；只有方法导出的语言（例如正式
//! 编译 lane 的托管导出面）可以不提交任何属性。
//!
//! 宿主边界合同（所有权、错误与生命周期）：
//!
//! - 整数按完整 `i64` 域跨越边界（`napi_create_int64` / `PyLong_FromLongLong`），
//!   不经过 32 位 C `long` 或 JS number 截断；
//! - 布尔按宿主原生布尔类型跨越（`napi_get_boolean` / `PyBool_FromLong`），
//!   不与整数混为同一返回类别；
//! - 字符串返回携带镜像内描述符，宿主侧复制成原生字符串；描述符属于镜像、
//!   永不单独释放，不存在跨 CRT 的所有权转移；
//! - 实参解析失败不静默成 `0`：Node 抛 `TypeError`、CPython 原样传播宿主异常，
//!   `PyModuleDef_Init` / 装箱失败按宿主错误返回。

use super::host::NativePlatform;
use super::{NativeDataTarget, NativeTextInstruction, NativeTextProgramFunction};

/// 宿主边界值种类：每个宿主闭合的编组能力在这里显式建模。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostValueKind {
    /// 完整 `i64` 域整数。
    Int64,
    /// 原生布尔。
    Bool,
    /// 镜像字符串描述符 → 宿主字符串。
    Utf8,
}

/// 一个模块级属性导出：宿主读取 `<module>.<name>` 时得到 `symbol` 的返回值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProperty {
    /// 宿主可见属性名。
    pub name: String,
    /// 镜像内零参符号名。
    pub symbol: String,
    /// 返回载体。
    pub returns: HostValueKind,
}

/// 一个可暴露给宿主扩展模块的方法（适配器提交的冻结导出事实）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMethod {
    /// 宿主可见方法名，同时是镜像内被调用符号名。
    pub name: String,
    /// 整数形参个数。
    pub parameters: usize,
    /// 返回载体。
    pub returns: HostValueKind,
}

/// CPython 多阶段模块定义（`PyModuleDef`）头部在 `.data` 中的镜像长度。
///
/// x86-64 布局（全部 8 字节字段）：
///
/// - `0..40`：`PyModuleDef_Base { ob_refcnt = 1, ob_type = NULL, m_init = NULL,
///   m_index = 0, m_copy = NULL }`（即 `PyModuleDef_HEAD_INIT`）；
/// - `40`：`m_name`，`R_X86_64_RELATIVE` 指向紧随其后的模块名；
/// - `48`：`m_doc = NULL`；
/// - `56`：`m_size = 0`（多阶段初始化，模块状态由宿主管理）；
/// - `64`：`m_methods`，有方法时指向 NUL 结尾的 `PyMethodDef` 数组；
/// - `72..104`：`m_slots` 指向 `Py_mod_exec` 槽表，其余为 `NULL`；
/// - `104` 起：模块名、各属性名、各方法名与 `PyArg_ParseTuple` 格式串、方法表。
const PY_MODULE_DEF_BASE_LEN: u32 = 104;

/// `PyMethodDef` 每项 32 字节：`ml_name`、`ml_meth`、`ml_flags`、`ml_doc`。
const PY_METHOD_DEF_LEN: u32 = 32;

/// `METH_VARARGS`：方法以 `(self, args)` 调用，实参由 `PyArg_ParseTuple` 解析。
const PY_METH_VARARGS: u32 = 0x0001;

/// `Py_mod_exec` 槽位编号（`{0, NULL}` 是终止项，合法槽位从 1 起）。
const PY_MOD_EXEC: i32 = 2;

/// 多阶段执行钩子的符号名（`PyInit_` 之外的第二入口）。
pub const PYTHON_EXEC_NAME: &str = "__nyar_py_exec";

/// Node-API 回调最多 6 个整数形参：与 SysV64 的整数实参寄存器数一致，
/// `napi_get_cb_info` 的 `argv` 数组按该容量预置。
const NODE_METHOD_MAX_PARAMETERS: usize = 6;

/// 镜像文本 arena 容量：运行期构造的字符串存放在 `.data` 的这段载荷里。
///
/// 所有权归镜像；扩展模块的入口包装体在每次宿主调用开始时把游标归零，
/// 因此单个镜像的峰值占用有界。预算耗尽按 `ud2` 陷入失败闭合。
pub const NATIVE_TEXT_ARENA_CAPACITY: u32 = 64 * 1024;

/// 在 `.data` 末尾追加文本 arena：8 字节游标 + [`NATIVE_TEXT_ARENA_CAPACITY`] 字节载荷。
///
/// 返回游标偏移；`arena_offset + 8` 是载荷起点，游标值按载荷起点相对编码。
pub fn append_text_arena(data: &mut Vec<u8>) -> u32 {
    let offset = u32::try_from(data.len()).expect("native 数据段长度超出 u32");
    data.extend_from_slice(&0u64.to_le_bytes());
    data.resize(data.len() + NATIVE_TEXT_ARENA_CAPACITY as usize, 0);
    offset
}

/// 入口处重置 arena 游标：`[arena_offset] = 0`。
///
/// `reset_slot` 是包装体可用的临时帧槽；必须在调用任何被包装符号之前执行。
pub fn reset_text_arena(reset_slot: u16, arena_offset: u32) -> Vec<NativeTextInstruction> {
    use NativeTextInstruction as Lir;
    vec![
        Lir::LoadDataAddress(arena_offset),
        Lir::StoreSlot(reset_slot),
        Lir::LoadI64(0),
        Lir::LoadSlotToRcx(reset_slot),
        Lir::StoreRaxToRcx,
    ]
}

/// 模块名必须是合法 C 标识符：`PyInit_<module>` 要能在 C 中声明。
pub fn is_c_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

/// 任一导出是否以布尔跨越宿主边界（决定 `napi_get_boolean` / `PyBool_FromLong` 导入）。
pub fn any_returns_bool(properties: &[HostProperty], methods: &[HostMethod]) -> bool {
    properties.iter().any(|property| property.returns == HostValueKind::Bool)
        || methods.iter().any(|method| method.returns == HostValueKind::Bool)
}

/// 任一导出是否返回镜像字符串描述符（决定字符串复制导入）。
pub fn any_returns_utf8(methods: &[HostMethod]) -> bool {
    methods.iter().any(|method| method.returns == HostValueKind::Utf8)
}

fn python_wrapper_name(method: &str) -> String {
    format!("__nyar_py_wrap_{method}")
}

fn node_wrapper_name(method: &str) -> String {
    format!("__nyar_node_wrap_{method}")
}

/// 追加一个 `PyModuleDef_Slot`（`slot` + 4 字节填充 + `value` 指针槽）。
fn push_module_slot(data: &mut Vec<u8>, slot: i32) {
    data.extend_from_slice(&slot.to_le_bytes());
    data.extend_from_slice(&[0u8; 4]);
    data.extend_from_slice(&[0u8; 8]);
}

fn push_cstring(data: &mut Vec<u8>, value: &str) {
    data.extend_from_slice(value.as_bytes());
    data.push(0);
}

/// 构造 `.data`：多阶段模块定义、模块名、属性名、方法名/格式串、方法表与
/// `PyModuleDef_Slot` 表（`Py_mod_exec`）。
///
/// 多阶段初始化由宿主创建模块并按槽位执行：`PyInit_<module>` 只返回定义地址，
/// 模块实例与每解释器状态由 CPython 管理（`m_size = 0`）。
///
/// 返回数据块、自相对重定位、每个属性的名字偏移与每个方法的格式串偏移（包装体需要）。
pub fn python_module_data(
    module_name: &str,
    properties: &[HostProperty],
    methods: &[HostMethod],
) -> (Vec<u8>, Vec<(u32, NativeDataTarget)>, Vec<u32>, Vec<u32>) {
    let mut data = vec![0u8; PY_MODULE_DEF_BASE_LEN as usize];
    let mut relocations = Vec::new();
    data[..8].copy_from_slice(&1i64.to_le_bytes()); // ob_refcnt = 1
    // `m_size = 0`：多阶段初始化，不申请每模块状态。

    let name_offset = data.len() as u32;
    push_cstring(&mut data, module_name);
    relocations.push((40, NativeDataTarget::Data(name_offset)));

    let mut property_name_offsets = Vec::with_capacity(properties.len());
    for property in properties {
        property_name_offsets.push(data.len() as u32);
        push_cstring(&mut data, &property.name);
    }

    let mut method_name_offsets = Vec::with_capacity(methods.len());
    let mut format_offsets = Vec::with_capacity(methods.len());
    for method in methods {
        method_name_offsets.push(data.len() as u32);
        push_cstring(&mut data, &method.name);
        // `PyArg_ParseTuple` 的 `n` 是 `Py_ssize_t`（两侧平台都是 8 字节有符号），
        // 与残差的 `i64` 载体一一对应；`:<name>` 让宿主错误信息带上方法名。
        let format = format!("{}:{}", "n".repeat(method.parameters), method.name);
        format_offsets.push(data.len() as u32);
        push_cstring(&mut data, &format);
    }

    if !methods.is_empty() {
        let methods_offset = data.len() as u32;
        // `m_methods` 指向 NUL 结尾的 `PyMethodDef` 数组；多阶段执行时由
        // `_PyModule_ExecDef` 在槽位执行前安装。
        relocations.push((64, NativeDataTarget::Data(methods_offset)));
        for (index, method) in methods.iter().enumerate() {
            let entry = data.len() as u32;
            data.extend_from_slice(&[0u8; PY_METHOD_DEF_LEN as usize]);
            relocations.push((entry, NativeDataTarget::Data(method_name_offsets[index])));
            relocations.push((entry + 8, NativeDataTarget::Function(python_wrapper_name(&method.name))));
            data[entry as usize + 16..entry as usize + 20].copy_from_slice(&PY_METH_VARARGS.to_le_bytes());
        }
        data.extend_from_slice(&[0u8; PY_METHOD_DEF_LEN as usize]); // 空终止项
    }

    // `PyModuleDef_Slot { int slot; void *value; }`（16 字节，含 4 字节对齐填充）：
    // `Py_mod_exec`（2）→ `__nyar_py_exec`，然后 `{0, NULL}` 终止。
    let slots_offset = data.len() as u32;
    push_module_slot(&mut data, PY_MOD_EXEC);
    push_module_slot(&mut data, 0);
    relocations.push((72, NativeDataTarget::Data(slots_offset)));
    relocations.push((slots_offset + 8, NativeDataTarget::Function(PYTHON_EXEC_NAME.to_string())));

    (data, relocations, property_name_offsets, format_offsets)
}

/// `PyObject* wrapper(PyObject* self, PyObject* args)`：以 `PyArg_ParseTuple` 解析
/// 整数实参并调用同名符号，结果按类型化载体跨越边界：
/// `Int64 → PyLong_FromLongLong`、`Bool → PyBool_FromLong`、`Utf8 → PyUnicode_FromStringAndSize`。
///
/// 整数必须使用 64 位装箱：Windows 的 C `long` 是 32 位，`PyLong_FromLong` 会把
/// `4294967297` 静默截成 `1`。
///
/// `PyArg_ParseTuple` 返回 0 表示解析失败（类型/个数不符），此时宿主已经设置异常，
/// 包装体原样返回 `NULL`，不伪造结果。
pub fn python_method_wrapper(
    method: &HostMethod,
    format_offset: u32,
    dll: &str,
    arena: Option<u32>,
) -> NativeTextProgramFunction {
    use NativeTextInstruction as Lir;
    const ARGS_SLOT: u16 = 0;
    const FORMAT_SLOT: u16 = 1;
    const TEMP_SLOT: u16 = 2;
    const FIRST_ARGUMENT_SLOT: u16 = 3;
    const FAIL_LABEL: u32 = 0;
    let argument_slot = |index: usize| FIRST_ARGUMENT_SLOT + index as u16;
    let descriptor_slot = FIRST_ARGUMENT_SLOT + method.parameters as u16;
    let pointer_slot = descriptor_slot + 1;
    let length_slot = descriptor_slot + 2;

    let mut instructions = Vec::new();
    if let Some(arena) = arena {
        instructions.extend(reset_text_arena(TEMP_SLOT, arena));
    }
    instructions.extend([
        // 保存 `args`（self 不参与残差语义）。
        Lir::LoadParam { index: 1, slot: ARGS_SLOT },
        Lir::LoadSlotToArg { slot: ARGS_SLOT, index: 0 },
        Lir::LoadDataAddress(format_offset),
        Lir::StoreSlot(FORMAT_SLOT),
        Lir::LoadSlotToArg { slot: FORMAT_SLOT, index: 1 },
    ]);
    for index in 0..method.parameters {
        // `PyArg_ParseTuple` 把第 index 个 `long` 写入该帧槽的地址。
        instructions.push(Lir::LoadSlotAddress(argument_slot(index)));
        instructions.push(Lir::StoreSlot(TEMP_SLOT));
        instructions.push(Lir::LoadSlotToArg { slot: TEMP_SLOT, index: (2 + index) as u8 });
    }
    instructions.push(Lir::CallImport { module: dll.to_string(), field: "PyArg_ParseTuple".to_string() });
    // 返回 0 = 解析失败（异常已由宿主设置）。
    instructions.push(Lir::JumpIfZero(FAIL_LABEL));
    for index in 0..method.parameters {
        instructions.push(Lir::LoadSlotToArg { slot: argument_slot(index), index: index as u8 });
    }
    instructions.push(Lir::Call(method.name.clone()));
    match method.returns {
        HostValueKind::Utf8 => {
            // 符号返回的是镜像内字符串描述符地址；宿主侧复制成 `str`，描述符本身
            // 属于镜像、永不释放，因此不需要跨 CRT 的所有权转移。
            instructions.extend([
                Lir::StoreSlot(descriptor_slot),
                Lir::LoadSlot(descriptor_slot),
                Lir::AddRaxImmediate(8),
                Lir::StoreSlot(pointer_slot),
                Lir::LoadSlot(descriptor_slot),
                Lir::LoadDescriptorLength,
                Lir::StoreSlot(length_slot),
                Lir::LoadSlotToArg { slot: pointer_slot, index: 0 },
                Lir::LoadSlotToArg { slot: length_slot, index: 1 },
                Lir::CallImport { module: dll.to_string(), field: "PyUnicode_FromStringAndSize".to_string() },
                Lir::Return,
            ]);
        }
        HostValueKind::Bool => {
            instructions.push(Lir::StoreSlot(TEMP_SLOT));
            instructions.push(Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 0 });
            instructions.push(Lir::CallImport { module: dll.to_string(), field: "PyBool_FromLong".to_string() });
            instructions.push(Lir::Return);
        }
        HostValueKind::Int64 => {
            instructions.push(Lir::StoreSlot(TEMP_SLOT));
            instructions.push(Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 0 });
            instructions.push(Lir::CallImport { module: dll.to_string(), field: "PyLong_FromLongLong".to_string() });
            instructions.push(Lir::Return);
        }
    }
    instructions.push(Lir::Label(FAIL_LABEL));
    instructions.push(Lir::LoadI64(0));
    instructions.push(Lir::Return);

    NativeTextProgramFunction { name: python_wrapper_name(&method.name), slot_count: length_slot + 1, instructions }
}

/// `PyInit_<module>` 入口：多阶段初始化返回经 `PyModuleDef_Init` 初始化的定义地址。
///
/// CPython 的导入器用 `PyObject_TypeCheck(m, &PyModuleDef_Type)` 判定多阶段模块，
/// 因此定义必须先由 `PyModuleDef_Init` 设置 `ob_type`；随后宿主调用
/// `PyModule_FromDefAndSpec2` 创建模块并执行 `Py_mod_exec` 槽位。
pub fn python_module_registration(module_name: &str, dll: &str) -> NativeTextProgramFunction {
    use NativeTextInstruction as Lir;
    NativeTextProgramFunction {
        name: format!("PyInit_{module_name}"),
        slot_count: 1,
        instructions: vec![
            Lir::LoadDataAddress(0),
            Lir::StoreSlot(0),
            Lir::LoadSlotToArg { slot: 0, index: 0 },
            Lir::CallImport { module: dll.to_string(), field: "PyModuleDef_Init".to_string() },
            Lir::Return,
        ],
    }
}

/// `int __nyar_py_exec(PyObject* module)`：`Py_mod_exec` 槽位的执行体。
///
/// 模块实例由宿主创建；这里逐个安装属性（方法表由 `_PyModule_ExecDef` 依据
/// `m_methods` 先行安装）。成功返回 0，失败返回 -1，不吞掉宿主错误。
///
/// `Int64` 先经 `PyLong_FromLongLong` 装箱再交给 `PyModule_AddObject`：
/// `PyModule_AddIntConstant` 走 C `long`，在 Windows 上只有 32 位，会把 i64 截断；
/// `Bool` 经 `PyBool_FromLong` 得到原生 `bool`，不与整数混为同一返回类别。
pub fn python_module_exec(properties: &[HostProperty], property_name_offsets: &[u32], dll: &str, arena: Option<u32>) -> NativeTextProgramFunction {
    use NativeTextInstruction as Lir;
    const MODULE_SLOT: u16 = 0;
    const VALUE_SLOT: u16 = 1;
    const OBJECT_SLOT: u16 = 2;
    const RESULT_SLOT: u16 = 3;
    const ZERO_SLOT: u16 = 4;
    const TEMP_SLOT: u16 = 5;
    const FAIL_LABEL: u32 = 0;
    let mut instructions = vec![Lir::LoadParam { index: 0, slot: MODULE_SLOT }];
    if let Some(arena) = arena {
        // 入口实参已落槽：重置使用 rax/rcx 不会破坏 Win64 的 rcx 形参。
        instructions.extend(reset_text_arena(TEMP_SLOT, arena));
    }
    instructions.push(Lir::LoadI64(0));
    instructions.push(Lir::StoreSlot(ZERO_SLOT));
    for (property, name_offset) in properties.iter().zip(property_name_offsets) {
        instructions.extend([
            Lir::Call(property.symbol.clone()),
            Lir::StoreSlot(VALUE_SLOT),
            // 装箱：整数走 64 位有符号，布尔走原生 `bool`。
            Lir::LoadSlotToArg { slot: VALUE_SLOT, index: 0 },
            Lir::CallImport {
                module: dll.to_string(),
                field: match property.returns {
                    HostValueKind::Bool => "PyBool_FromLong",
                    _ => "PyLong_FromLongLong",
                }
                .to_string(),
            },
            Lir::StoreSlot(OBJECT_SLOT),
            // `NULL` 表示装箱失败（异常已由宿主设置）。
            Lir::LoadSlot(OBJECT_SLOT),
            Lir::LoadSlotToRcx(ZERO_SLOT),
            Lir::Ne,
            Lir::JumpIfZero(FAIL_LABEL),
            // `PyModule_AddObject(module, name, object)`：成功时接管对象引用。
            Lir::LoadSlotToArg { slot: MODULE_SLOT, index: 0 },
            Lir::LoadDataAddress(*name_offset),
            Lir::StoreSlot(TEMP_SLOT),
            Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 1 },
            Lir::LoadSlotToArg { slot: OBJECT_SLOT, index: 2 },
            Lir::CallImport { module: dll.to_string(), field: "PyModule_AddObject".to_string() },
            Lir::StoreSlot(RESULT_SLOT),
            Lir::LoadSlot(RESULT_SLOT),
            Lir::LoadSlotToRcx(ZERO_SLOT),
            Lir::Eq,
            Lir::JumpIfZero(FAIL_LABEL),
        ]);
    }
    instructions.extend([Lir::LoadI64(0), Lir::Return, Lir::Label(FAIL_LABEL), Lir::LoadI64(-1), Lir::Return]);
    NativeTextProgramFunction { name: PYTHON_EXEC_NAME.to_string(), slot_count: 6, instructions }
}

/// Node-API 数据段：属性名、错误消息与各方法名（全部 NUL 结尾）。
pub fn node_module_data(properties: &[HostProperty], methods: &[HostMethod]) -> (Vec<u8>, Vec<u32>, Vec<u32>, u32) {
    let mut data = Vec::new();
    let mut property_name_offsets = Vec::with_capacity(properties.len());
    for property in properties {
        property_name_offsets.push(data.len() as u32);
        push_cstring(&mut data, &property.name);
    }
    let error_message_offset = data.len() as u32;
    push_cstring(&mut data, "expected integer arguments");
    let mut method_name_offsets = Vec::with_capacity(methods.len());
    for method in methods {
        method_name_offsets.push(data.len() as u32);
        push_cstring(&mut data, &method.name);
    }
    (data, property_name_offsets, method_name_offsets, error_message_offset)
}

/// `napi_register_module_v1(env, exports)` 入口：把每个属性的零参符号结果写成
/// `exports.<name>`，并为每个方法注册同名回调，最后返回 `exports` 对象。
///
/// 属性集合由适配器提交：残差语言可以暴露 `{ value }`，正式编译 lane 可以只
/// 暴露方法；本模块不要求任何特定符号名。
pub fn node_addon_registration(
    properties: &[HostProperty],
    property_name_offsets: &[u32],
    methods: &[HostMethod],
    method_name_offsets: &[u32],
    platform: NativePlatform,
    dll: &str,
    arena: Option<u32>,
) -> NativeTextProgramFunction {
    use NativeTextInstruction as Lir;
    const ENV_SLOT: u16 = 0;
    const EXPORTS_SLOT: u16 = 1;
    const VALUE_SLOT: u16 = 2;
    const TEMP_SLOT: u16 = 3;
    const FUNCTION_SLOT: u16 = 4;
    const ZERO_SLOT: u16 = 5;
    const OUT_SLOT: u16 = 6;

    let mut instructions = vec![
        Lir::LoadParam { index: 0, slot: ENV_SLOT },
        Lir::LoadParam { index: 1, slot: EXPORTS_SLOT },
        Lir::LoadI64(0),
        Lir::StoreSlot(ZERO_SLOT),
    ];
    if let Some(arena) = arena {
        instructions.extend(reset_text_arena(TEMP_SLOT, arena));
    }
    for (property, name_offset) in properties.iter().zip(property_name_offsets) {
        instructions.extend([
            Lir::Call(property.symbol.clone()),
            Lir::StoreSlot(VALUE_SLOT),
            // `napi_create_int64(env, value, &out)` / `napi_get_boolean(env, value, &out)`。
            Lir::LoadSlotToArg { slot: ENV_SLOT, index: 0 },
            Lir::LoadSlotToArg { slot: VALUE_SLOT, index: 1 },
            Lir::LoadSlotAddress(OUT_SLOT),
            Lir::StoreSlot(TEMP_SLOT),
            Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 2 },
            Lir::CallImport {
                module: dll.to_string(),
                field: match property.returns {
                    HostValueKind::Bool => "napi_get_boolean",
                    _ => "napi_create_int64",
                }
                .to_string(),
            },
            // `napi_set_named_property(env, exports, name, out)`。
            Lir::LoadSlotToArg { slot: ENV_SLOT, index: 0 },
            Lir::LoadSlotToArg { slot: EXPORTS_SLOT, index: 1 },
            Lir::LoadDataAddress(*name_offset),
            Lir::StoreSlot(TEMP_SLOT),
            Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 2 },
            Lir::LoadSlotToArg { slot: OUT_SLOT, index: 3 },
            Lir::CallImport { module: dll.to_string(), field: "napi_set_named_property".to_string() },
        ]);
    }
    for (method, name_offset) in methods.iter().zip(method_name_offsets) {
        // `napi_create_function(env, name, len, wrapper, data, &fn)`。
        instructions.extend([
            Lir::LoadSlotToArg { slot: ENV_SLOT, index: 0 },
            Lir::LoadDataAddress(*name_offset),
            Lir::StoreSlot(TEMP_SLOT),
            Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 1 },
            Lir::LoadI64(method.name.len() as i64),
            Lir::StoreSlot(TEMP_SLOT),
            Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 2 },
            Lir::LoadFunctionAddress(node_wrapper_name(&method.name)),
            Lir::StoreSlot(TEMP_SLOT),
            Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 3 },
        ]);
        if platform.is_windows() {
            // 第 5、6 个实参走 Win64 栈：`data = NULL`、`result = &fn`。
            instructions.push(Lir::ZeroStackArg(0));
            instructions.push(Lir::StoreSlotAddressToStackArg { slot: FUNCTION_SLOT, index: 1 });
        }
        else {
            instructions.push(Lir::LoadSlotToArg { slot: ZERO_SLOT, index: 4 });
            instructions.push(Lir::LoadSlotAddress(FUNCTION_SLOT));
            instructions.push(Lir::StoreSlot(TEMP_SLOT));
            instructions.push(Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 5 });
        }
        instructions.extend([
            Lir::CallImport { module: dll.to_string(), field: "napi_create_function".to_string() },
            // `napi_set_named_property(env, exports, name, fn)`。
            Lir::LoadSlotToArg { slot: ENV_SLOT, index: 0 },
            Lir::LoadSlotToArg { slot: EXPORTS_SLOT, index: 1 },
            Lir::LoadDataAddress(*name_offset),
            Lir::StoreSlot(TEMP_SLOT),
            Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 2 },
            Lir::LoadSlotToArg { slot: FUNCTION_SLOT, index: 3 },
            Lir::CallImport { module: dll.to_string(), field: "napi_set_named_property".to_string() },
        ]);
    }
    instructions.push(Lir::LoadSlot(EXPORTS_SLOT));
    instructions.push(Lir::Return);

    NativeTextProgramFunction { name: "napi_register_module_v1".to_string(), slot_count: 7, instructions }
}

/// `napi_value wrapper(napi_env env, napi_callback_info info)`：把 Node 实参解析成
/// `i64` 后调用同名符号，结果按类型化载体跨越边界：
/// `Int64 → napi_create_int64`（完整 i64 域，不经过 JS number 截断）、
/// `Bool → napi_get_boolean`、`Utf8 → napi_create_string_utf8`。
///
/// 参数个数不符或不是整数时抛 `TypeError` 并返回 `NULL`，不把错误静默成 0。
pub fn node_method_wrapper(method: &HostMethod, error_message_offset: u32, platform: NativePlatform, dll: &str, arena: Option<u32>) -> NativeTextProgramFunction {
    use NativeTextInstruction as Lir;
    const ENV_SLOT: u16 = 0;
    const INFO_SLOT: u16 = 1;
    const ARGC_SLOT: u16 = 2;
    const ZERO_SLOT: u16 = 3;
    const TEMP_SLOT: u16 = 4;
    // `argv` 数组占用的帧槽；槽号递增对应地址递减，数组基址取最高槽号。
    const ARGV_FIRST_SLOT: u16 = 5;
    const RESULT_SLOT: u16 = 11;
    const OUT_FIRST_SLOT: u16 = 12;
    // 字符串返回的额外帧槽：`argv`/out 区最多到 17，以下从 18 起，互不重叠。
    const DESCRIPTOR_SLOT: u16 = 18;
    const POINTER_SLOT: u16 = 19;
    const LENGTH_SLOT: u16 = 20;
    const STRING_SLOT: u16 = 21;
    const FAIL_LABEL: u32 = 0;
    const CALLBACK_OK_LABEL: u32 = 1;
    let argv_slot = |index: usize| ARGV_FIRST_SLOT + NODE_METHOD_MAX_PARAMETERS as u16 - 1 - index as u16;
    let out_slot = |index: usize| OUT_FIRST_SLOT + index as u16;
    let continue_label = |index: usize| 2 + index as u32;
    let parameters = method.parameters;

    let mut instructions = vec![
        Lir::LoadParam { index: 0, slot: ENV_SLOT },
        Lir::LoadParam { index: 1, slot: INFO_SLOT },
        // `argc` 是 in/out：传入 `argv` 容量，返回实际个数。
        Lir::LoadI64(NODE_METHOD_MAX_PARAMETERS as i64),
        Lir::StoreSlot(ARGC_SLOT),
        Lir::LoadI64(0),
        Lir::StoreSlot(ZERO_SLOT),
    ];
    if let Some(arena) = arena {
        instructions.extend(reset_text_arena(TEMP_SLOT, arena));
    }
    instructions.extend([
        // `napi_get_cb_info(env, info, &argc, argv, NULL, NULL)`。
        Lir::LoadSlotToArg { slot: ENV_SLOT, index: 0 },
        Lir::LoadSlotToArg { slot: INFO_SLOT, index: 1 },
        Lir::LoadSlotAddress(ARGC_SLOT),
        Lir::StoreSlot(TEMP_SLOT),
        Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 2 },
        Lir::LoadSlotAddress(ARGV_FIRST_SLOT + NODE_METHOD_MAX_PARAMETERS as u16 - 1),
        Lir::StoreSlot(TEMP_SLOT),
        Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 3 },
    ]);
    // `napi_get_cb_info` 的第 5、6 个实参是 `this` 与 `data`，恒为 `NULL`。
    if platform.is_windows() {
        instructions.push(Lir::ZeroStackArg(0));
        instructions.push(Lir::ZeroStackArg(1));
    }
    else {
        instructions.push(Lir::LoadSlotToArg { slot: ZERO_SLOT, index: 4 });
        instructions.push(Lir::LoadSlotToArg { slot: ZERO_SLOT, index: 5 });
    }
    instructions.extend([
        Lir::CallImport { module: dll.to_string(), field: "napi_get_cb_info".to_string() },
        // `napi_get_cb_info` 返回 0 才是成功。
        Lir::JumpIfZero(CALLBACK_OK_LABEL),
        Lir::Jump(FAIL_LABEL),
        Lir::Label(CALLBACK_OK_LABEL),
        // 实际个数必须与方法形参一致。
        Lir::LoadI64(parameters as i64),
        Lir::StoreSlot(TEMP_SLOT),
        Lir::LoadSlot(ARGC_SLOT),
        Lir::LoadSlotToRcx(TEMP_SLOT),
        Lir::Eq,
        Lir::JumpIfZero(FAIL_LABEL),
    ]);
    for index in 0..parameters {
        // `napi_get_value_int64(env, argv[index], &out)`。
        instructions.extend([
            Lir::LoadSlotToArg { slot: ENV_SLOT, index: 0 },
            Lir::LoadSlotToArg { slot: argv_slot(index), index: 1 },
            Lir::LoadSlotAddress(out_slot(index)),
            Lir::StoreSlot(TEMP_SLOT),
            Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 2 },
            Lir::CallImport { module: dll.to_string(), field: "napi_get_value_int64".to_string() },
            Lir::JumpIfZero(continue_label(index)),
            Lir::Jump(FAIL_LABEL),
            Lir::Label(continue_label(index)),
        ]);
    }
    for index in 0..parameters {
        instructions.push(Lir::LoadSlotToArg { slot: out_slot(index), index: index as u8 });
    }
    instructions.push(Lir::Call(method.name.clone()));
    match method.returns {
        HostValueKind::Utf8 => {
            // 镜像内字符串描述符 → Node 字符串；描述符永不释放，无需所有权转移。
            instructions.extend([
                Lir::StoreSlot(DESCRIPTOR_SLOT),
                Lir::LoadSlot(DESCRIPTOR_SLOT),
                Lir::AddRaxImmediate(8),
                Lir::StoreSlot(POINTER_SLOT),
                Lir::LoadSlot(DESCRIPTOR_SLOT),
                Lir::LoadDescriptorLength,
                Lir::StoreSlot(LENGTH_SLOT),
                // `napi_create_string_utf8(env, ptr, len, &out)`。
                Lir::LoadSlotToArg { slot: ENV_SLOT, index: 0 },
                Lir::LoadSlotToArg { slot: POINTER_SLOT, index: 1 },
                Lir::LoadSlotToArg { slot: LENGTH_SLOT, index: 2 },
                Lir::LoadSlotAddress(STRING_SLOT),
                Lir::StoreSlot(TEMP_SLOT),
                Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 3 },
                Lir::CallImport { module: dll.to_string(), field: "napi_create_string_utf8".to_string() },
                Lir::LoadSlot(STRING_SLOT),
                Lir::Return,
            ]);
        }
        HostValueKind::Bool | HostValueKind::Int64 => {
            let field = match method.returns {
                HostValueKind::Bool => "napi_get_boolean",
                HostValueKind::Int64 => "napi_create_int64",
                HostValueKind::Utf8 => unreachable!("matched above"),
            };
            instructions.extend([
                Lir::StoreSlot(RESULT_SLOT),
                // `napi_create_int64(env, value, &result)` / `napi_get_boolean(env, value, &result)`。
                Lir::LoadSlotToArg { slot: ENV_SLOT, index: 0 },
                Lir::LoadSlotToArg { slot: RESULT_SLOT, index: 1 },
                Lir::LoadSlotAddress(RESULT_SLOT),
                Lir::StoreSlot(TEMP_SLOT),
                Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 2 },
                Lir::CallImport { module: dll.to_string(), field: field.to_string() },
                Lir::LoadSlot(RESULT_SLOT),
                Lir::Return,
            ]);
        }
    }
    // `napi_throw_type_error(env, NULL, message)`。
    instructions.extend([
        Lir::Label(FAIL_LABEL),
        Lir::LoadSlotToArg { slot: ENV_SLOT, index: 0 },
        Lir::LoadSlotToArg { slot: ZERO_SLOT, index: 1 },
        Lir::LoadDataAddress(error_message_offset),
        Lir::StoreSlot(TEMP_SLOT),
        Lir::LoadSlotToArg { slot: TEMP_SLOT, index: 2 },
        Lir::CallImport { module: dll.to_string(), field: "napi_throw_type_error".to_string() },
        Lir::LoadI64(0),
        Lir::Return,
    ]);

    NativeTextProgramFunction { name: node_wrapper_name(&method.name), slot_count: STRING_SLOT + 1, instructions }
}
