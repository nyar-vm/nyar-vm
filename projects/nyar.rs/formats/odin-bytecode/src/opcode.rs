//! 头码、编码形与指令编解码。
//!
//! 本模块是 opcode 数值、指令编码形、指令解码与写入 helper 的唯一所有者。
//! 它只描述字节流形状，不解释运行时语义。

use crate::errors::OdinDecodeError;

/// `.odin` 执行字节码 magic，大端写作 `ODIN`（`0x4F44494E`）。
///
/// `.odin` 是 Odin 后端的执行产物身份；与 `.nyar` 程序集身份不同，二者不互为别名。
pub const ODIN_MAGIC: u32 = 0x4F44_494E;

/// `.nyar` 程序集（Assembly）保留 magic，大端写作 `NYAR`（`0x4E594152`）。
///
/// 程序集由 `.nyar` validator 负责读取，本 crate 不解析它。旧版执行字节码曾使用
/// 同一 magic；`decode_module` 遇到它必须按 [`crate::OdinDecodeError::LegacyNyarMagic`]
/// 结构化拒绝并提示重建为 `.odin`，不得双读。
pub const NYAR_ASSEMBLY_MAGIC: u32 = 0x4E59_4152;

/// Current `.odin` format version.
///
/// 未正式发布前统一为 `0`（v0 基线）；旧版本的模块一律拒绝，不做兼容读取。
/// 新增 `Trap` 头码（0x0C）属于格式扩展，v1 起生效：旧产物被拒绝、缓存随之失效。
///
/// 名称沿用 `Odin*` 类型前缀约定（与 `OdinHeadCode` / `OdinInstruction` 一致），
/// 刻意不改成全大写，避免对公开 ABI 常量做无谓改名。
///
/// v1 → v2：常量池新增 `Integer64`（0x12），旧版本模块由 `decode_module` 结构化拒绝。
///
/// v2 → v3：新增 `CallIndirect` 头码（0x06），旧版本模块由 `decode_module` 结构化拒绝。
///
/// v3 → v4：`Function` 值载体从裸 `i32` 函数索引改为两槽闭包对象（槽 0 = code i32，
/// 槽 1 = 捕获环境对象或 null）。指令编码不变，但函数值语义已变，旧缓存产物与新
/// 产物混链会静默算错，因此版本递增、旧产物由 `decode_module` 结构化拒绝。
///
/// v4 → v5：执行产物身份从 `.nyar` 迁到 `.odin`，文件 magic 由 `NYAR` 改为 `ODIN`；
/// 旧 `.nyar` 执行产物按 `LegacyNyarMagic` 拒绝，缓存键随之包含格式版本。
///
/// v5 → v6：新增 `I64ReinterpretF64`（0x60）与 `F64ReinterpretI64`（0x61）头码。
/// 位模式重解释属于指令层，不再是宿主导入；旧运行时读到这两个字节会结构化拒绝，
/// 新产物不得被旧缓存混链，因此版本递增。
///
/// v6 → v7：新增 f32 载体指令——`I32ReinterpretF32`（0x62）、`F32ReinterpretI32`（0x63）
/// 与 f32 算术/比较/转换块（0x64–0x6C）。f32 是独立载体（`Value::F32`），
/// 不是 f64 的别名，也不借用 i32/f64 槽；旧运行时读到这些字节会结构化拒绝。
#[allow(non_upper_case_globals)]
pub const Odin_VERSION: u32 = 7;

/// Bytecode format contract version（S-W6 provenance / cache 键）。
/// 与 [`Odin_VERSION`] 同步；格式破坏时两者一并递增。
pub const BYTECODE_FORMAT_VERSION: u32 = Odin_VERSION;

/// 已删除的 v1 `CallNative` 头码（`0xD1`）。
///
/// 不得再进入 [`OdinHeadCode`]；加载期验证器须拒绝含此字节的 v2 模块，改用 [`OdinHeadCode::CallImport`]。
pub const OBSOLETE_CALL_NATIVE: u8 = 0xD1;

/// File header size in bytes.
pub const HEADER_SIZE: usize = 16;

/// Section header size in bytes (kind + offset + size).
pub const SECTION_HEADER_SIZE: usize = 9;

/// Nyar instruction head code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum OdinHeadCode {
    /// 无操作。
    #[default]
    Nop = 0x00,
    /// 无条件跳转（operand1 = 目标 ip 偏移）。
    Jump = 0x01,
    /// 条件为真时跳转（operand1 = 目标 ip 偏移）。
    JumpIfTrue = 0x02,
    /// 条件为假时跳转（operand1 = 目标 ip 偏移）。
    JumpIfFalse = 0x03,
    /// 从当前帧返回。
    Return = 0x05,
    /// 间接调用（operand1 = argc）。
    ///
    /// 栈布局 `[arg0, …, argN-1, callee]`：弹出栈顶的 `callee`（`i32` 函数索引，
    /// 越界即运行期结构化错误），实参留在栈上由新帧按 `arity` 消费（与 [`Self::Call`] 一致）。
    /// 因为本指令只弹 `callee`，发射侧必须先压实参、再压 callee。
    CallIndirect = 0x06,
    /// 异步让出：弹出栈顶值作为 yielded 值，暂停当前帧并交还控制权。
    /// operand1 = resume_state（恢复后跳转的 ip 偏移标识）。
    Yield = 0x0A,
    /// 异步恢复：弹出栈顶的 Coroutine 值与 resume 值，恢复挂起帧继续执行。
    Resume = 0x0B,
    /// 执行期 trap：以结构化运行时错误终止当前调用，不再有后继指令。
    /// 用于发射 `Unreachable`（可达性已被前端声明为不可达）的真 trap 语义。
    Trap = 0x0C,
    /// 触发代数效应（raise）：弹出栈顶的 effect payload，捕获当前续延并调用 handler。
    /// handler 可选择 resume（控制流回到 resume_target，resume 值在栈顶）或不 resume。
    /// 若 `Effectful::Resume = !`，resume 路径不可达，VM 在尝试 resume 时报错。
    PerformEffect = 0x0D,
    /// 加载常量（operand1 = 常量池索引）。
    Constant = 0x10,
    /// 弹出栈顶值。
    Pop = 0x11,
    /// 复制栈顶值。
    Dup = 0x12,
    /// 读取局部变量（operand1 = 槽位索引）。
    LoadLocal = 0x20,
    /// 写入局部变量（operand1 = 槽位索引）。
    StoreLocal = 0x21,
    /// 读取形参（operand1 = 参数索引）。
    LoadArg = 0x22,
    /// 读取全局槽（operand1 = 全局索引）。
    LoadGlobal = 0x23,
    /// 写入全局槽（operand1 = 全局索引）。
    StoreGlobal = 0x24,
    /// 32 位有符号整数加法。
    I32Add = 0x30,
    /// 32 位有符号整数减法。
    I32Sub = 0x31,
    /// 32 位有符号整数乘法。
    I32Mul = 0x32,
    /// 32 位有符号整数除法。
    I32DivS = 0x33,
    /// 32 位有符号整数取余。
    I32RemS = 0x35,
    /// 32 位整数按位与。
    I32And = 0x36,
    /// 32 位整数按位或。
    I32Or = 0x37,
    /// 32 位整数按位异或。
    I32Xor = 0x38,
    /// 32 位整数左移（移位量按 32 取模）。
    I32Shl = 0x39,
    /// 32 位整数算术右移（移位量按 32 取模）。
    I32ShrS = 0x3A,
    /// 32 位整数逻辑右移（移位量按 32 取模）。
    I32ShrU = 0x3B,
    /// 32 位有符号整数相等比较。
    I32Eq = 0x40,
    /// 32 位有符号整数不等比较。
    I32Ne = 0x41,
    /// 32 位有符号整数小于比较。
    I32LtS = 0x42,
    /// 32 位无符号整数小于比较。
    I32LtU = 0x43,
    /// 32 位有符号整数小于等于比较。
    I32LeS = 0x44,
    /// 32 位无符号整数小于等于比较。
    I32LeU = 0x45,
    /// 32 位有符号整数大于比较。
    I32GtS = 0x46,
    /// 32 位无符号整数大于比较。
    I32GtU = 0x47,
    /// 32 位有符号整数大于等于比较。
    I32GeS = 0x48,
    /// 32 位无符号整数大于等于比较。
    I32GeU = 0x49,
    /// 调用函数（operand1 = 函数索引）。
    Call = 0x04,
    /// 静态调用（operand1 = 函数索引，operand2 = 参数个数）。
    CallStatic = 0x0F,
    /// 调用内置（operand1 = intrinsic 稠密下标，operand2 = argc）。
    CallIntrinsic = 0xD0,
    /// 调用模块导入表项（operand1 = import index，operand2 = argc）。
    /// 热路径只消费下标；链接名留在 imports section。
    CallImport = 0xD2,
    /// 按 layout_id 分配稠密槽位对象（operand1 = layout index）。
    /// 热路径不读字段名；布局描述在 layouts section。
    ObjectNew = 0x50,
    /// 按字段槽读取（operand1 = field_slot）；栈：`[obj] -> [value]`。
    FieldGet = 0x51,
    /// 按字段槽写入（operand1 = field_slot）；栈：`[obj, value] -> [obj]`。
    FieldSet = 0x52,
    /// `f64` 位模式重解释为 64 位整数：栈 `[f64] -> [i64]`，只搬位、不做数值转换。
    ///
    /// 语义与 Wasm `i64.reinterpret_f64` 一致：符号位、指数、次正规数与 NaN payload
    /// 原样保留；无立即数。
    I64ReinterpretF64 = 0x60,
    /// 64 位整数位模式重解释为 `f64`：栈 `[i64] -> [f64]`，只搬位、不做数值转换。
    ///
    /// 语义与 Wasm `f64.reinterpret_i64` 一致；无立即数。
    F64ReinterpretI64 = 0x61,
    /// `f32` 位模式重解释为 32 位整数：栈 `[f32] -> [i32]`。
    ///
    /// 语义与 Wasm `i32.reinterpret_f32` 一致：32 位宽度、NaN payload 23 位与
    /// `-0.0`/次正规位模式原样保留；无立即数。禁止经 f64 中转。
    I32ReinterpretF32 = 0x62,
    /// 32 位整数位模式重解释为 `f32`：栈 `[i32] -> [f32]`。
    ///
    /// 语义与 Wasm `f32.reinterpret_i32` 一致；无立即数。
    F32ReinterpretI32 = 0x63,
    /// 单精度加法：栈 `[f32, f32] -> [f32]`。
    ///
    /// 运算在单精度上完成，不提升到 f64 再降回（那会引入 double rounding）。
    F32Add = 0x64,
    /// 单精度减法：栈 `[f32, f32] -> [f32]`。
    F32Sub = 0x65,
    /// 单精度乘法：栈 `[f32, f32] -> [f32]`。
    F32Mul = 0x66,
    /// 单精度除法：栈 `[f32, f32] -> [f32]`。
    F32Div = 0x67,
    /// 单精度取负：栈 `[f32] -> [f32]`。
    ///
    /// 必须保留 `-0.0` 与 NaN 符号，不得实现成 `0.0 - x`。
    F32Neg = 0x68,
    /// 单精度相等比较：栈 `[f32, f32] -> [i32]`，结果为 `0`/`1`。
    F32Eq = 0x69,
    /// 单精度小于比较：栈 `[f32, f32] -> [i32]`，结果为 `0`/`1`；NaN 参与时恒为 `0`。
    F32Lt = 0x6A,
    /// `f64` 收窄为 `f32`：栈 `[f64] -> [f32]`，round-to-nearest-even，
    /// 溢出到 `inf`、下溢到 0 或次正规。单步完成，不经过其他精度。
    F32DemoteF64 = 0x6B,
    /// `f32` 拓宽为 `f64`：栈 `[f32] -> [f64]`，精确无损。
    F64PromoteF32 = 0x6C,
}

impl OdinHeadCode {
    /// Parse a head code from a raw byte.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Nop),
            0x01 => Some(Self::Jump),
            0x02 => Some(Self::JumpIfTrue),
            0x03 => Some(Self::JumpIfFalse),
            0x04 => Some(Self::Call),
            0x05 => Some(Self::Return),
            0x06 => Some(Self::CallIndirect),
            0x0A => Some(Self::Yield),
            0x0B => Some(Self::Resume),
            0x0C => Some(Self::Trap),
            0x0D => Some(Self::PerformEffect),
            0x0F => Some(Self::CallStatic),
            0x10 => Some(Self::Constant),
            0x11 => Some(Self::Pop),
            0x12 => Some(Self::Dup),
            0x20 => Some(Self::LoadLocal),
            0x21 => Some(Self::StoreLocal),
            0x22 => Some(Self::LoadArg),
            0x23 => Some(Self::LoadGlobal),
            0x24 => Some(Self::StoreGlobal),
            0x30 => Some(Self::I32Add),
            0x31 => Some(Self::I32Sub),
            0x32 => Some(Self::I32Mul),
            0x33 => Some(Self::I32DivS),
            0x35 => Some(Self::I32RemS),
            0x36 => Some(Self::I32And),
            0x37 => Some(Self::I32Or),
            0x38 => Some(Self::I32Xor),
            0x39 => Some(Self::I32Shl),
            0x3A => Some(Self::I32ShrS),
            0x3B => Some(Self::I32ShrU),
            0x40 => Some(Self::I32Eq),
            0x41 => Some(Self::I32Ne),
            0x42 => Some(Self::I32LtS),
            0x43 => Some(Self::I32LtU),
            0x44 => Some(Self::I32LeS),
            0x45 => Some(Self::I32LeU),
            0x46 => Some(Self::I32GtS),
            0x47 => Some(Self::I32GtU),
            0x48 => Some(Self::I32GeS),
            0x49 => Some(Self::I32GeU),
            0xD0 => Some(Self::CallIntrinsic),
            0xD2 => Some(Self::CallImport),
            0x50 => Some(Self::ObjectNew),
            0x51 => Some(Self::FieldGet),
            0x52 => Some(Self::FieldSet),
            0x60 => Some(Self::I64ReinterpretF64),
            0x61 => Some(Self::F64ReinterpretI64),
            0x62 => Some(Self::I32ReinterpretF32),
            0x63 => Some(Self::F32ReinterpretI32),
            0x64 => Some(Self::F32Add),
            0x65 => Some(Self::F32Sub),
            0x66 => Some(Self::F32Mul),
            0x67 => Some(Self::F32Div),
            0x68 => Some(Self::F32Neg),
            0x69 => Some(Self::F32Eq),
            0x6A => Some(Self::F32Lt),
            0x6B => Some(Self::F32DemoteF64),
            0x6C => Some(Self::F64PromoteF32),
            _ => None,
        }
    }
}

/// OVM 私有 intrinsic 指令身份。
///
/// 本枚举是 `NyarHeadCode::CallIntrinsic` 的 `operand1` 编码合同，属于
/// OVM 字节码格式与执行分派，与任何语言语义枚举无关。语言 builtin 身份
/// （如 `builtin.array.push`）不得由本枚举推导，本枚举也不得反向充当语言
/// builtin / 注册表。编号是已冻结的字节码兼容值，不得重排。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum IntrinsicOpcode {
    /// 向可增长数组存储 push（`operand2` = argc）。
    ArrayPush = 0,
    /// 数组 / 列表长度。
    ArrayLen = 1,
    /// 数组存储下标读取。
    ArrayGet = 2,
    /// 数组存储下标写入。
    ArraySet = 3,
    /// 引用解引用（OVM 当前未实现，执行期返回 `UnsupportedFeature`）。
    RefDeref = 4,
    /// 空值检查。
    IsNull = 5,
    /// 空值解包。
    UnwrapNull = 6,
}

impl IntrinsicOpcode {
    /// 返回写入 `CallIntrinsic` `operand1` 的编码值。
    pub const fn raw(self) -> i32 {
        self as i32
    }

    /// 由 `CallIntrinsic` `operand1` 还原指令身份；未知编号 fail-closed。
    pub fn from_operand(value: i32) -> Result<Self, OdinDecodeError> {
        match value {
            0 => Ok(Self::ArrayPush),
            1 => Ok(Self::ArrayLen),
            2 => Ok(Self::ArrayGet),
            3 => Ok(Self::ArraySet),
            4 => Ok(Self::RefDeref),
            5 => Ok(Self::IsNull),
            6 => Ok(Self::UnwrapNull),
            other => Err(OdinDecodeError::UnknownIntrinsicOpcode(other)),
        }
    }
}

/// Instruction encoding form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OdinInstructionForm {
    /// 仅头码，无立即数。
    Plain,
    /// 头码 + 一个 `i32` 立即数。
    Imm1,
    /// 头码 + 两个 `i32` 立即数。
    Imm2,
    /// 头码 + 三个 `i32` 立即数。
    Imm3,
    /// 非法或未知编码形。
    Invalid,
}

impl OdinHeadCode {
    /// Returns the encoding form for this head code.
    pub fn form(self) -> OdinInstructionForm {
        match self {
            Self::Jump
            | Self::JumpIfTrue
            | Self::JumpIfFalse
            | Self::Call
            | Self::CallStatic
            | Self::CallIndirect
            | Self::Constant
            | Self::Yield
            | Self::PerformEffect
            | Self::LoadLocal
            | Self::StoreLocal
            | Self::LoadArg
            | Self::LoadGlobal
            | Self::StoreGlobal
            | Self::ObjectNew
            | Self::FieldGet
            | Self::FieldSet => OdinInstructionForm::Imm1,
            Self::CallIntrinsic | Self::CallImport => OdinInstructionForm::Imm2,
            Self::Nop
            | Self::Trap
            | Self::Return
            | Self::Resume
            | Self::Pop
            | Self::Dup
            | Self::I32Add
            | Self::I32Sub
            | Self::I32Mul
            | Self::I32DivS
            | Self::I32RemS
            | Self::I32And
            | Self::I32Or
            | Self::I32Xor
            | Self::I32Shl
            | Self::I32ShrS
            | Self::I32ShrU
            | Self::I32Eq
            | Self::I32Ne
            | Self::I32LtS
            | Self::I32LtU
            | Self::I32LeS
            | Self::I32LeU
            | Self::I32GtS
            | Self::I32GtU
            | Self::I32GeS
            | Self::I32GeU
            | Self::I64ReinterpretF64
            | Self::F64ReinterpretI64
            | Self::I32ReinterpretF32
            | Self::F32ReinterpretI32
            | Self::F32Add
            | Self::F32Sub
            | Self::F32Mul
            | Self::F32Div
            | Self::F32Neg
            | Self::F32Eq
            | Self::F32Lt
            | Self::F32DemoteF64
            | Self::F64PromoteF32 => OdinInstructionForm::Plain,
        }
    }

    /// Returns the encoded instruction size for this head code.
    pub fn code_size(self) -> u8 {
        match self.form() {
            OdinInstructionForm::Plain => 1,
            OdinInstructionForm::Imm1 => 5,
            OdinInstructionForm::Imm2 => 9,
            OdinInstructionForm::Imm3 => 13,
            OdinInstructionForm::Invalid => 0,
        }
    }
}

/// Pre-decoded instruction view used by the interpreter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OdinInstruction {
    /// Instruction head code.
    pub code: OdinHeadCode,
    /// Encoded size in the bytecode stream; `0` means invalid.
    pub size: u8,
    /// 第一个立即数操作数。
    pub operand1: i32,
    /// 第二个立即数操作数。
    pub operand2: i32,
    /// 第三个立即数操作数。
    pub operand3: i32,
}

impl OdinInstruction {
    /// Creates a decoded instruction.
    pub fn new(code: OdinHeadCode, operand1: i32, operand2: i32, operand3: i32) -> Self {
        Self { code, size: code.code_size(), operand1, operand2, operand3 }
    }

    /// Whether this instruction is valid.
    pub fn is_valid(self) -> bool {
        self.size != 0
    }
}

/// Decode one instruction at `pc` from `bytecode`.
pub fn decode_at(bytecode: &[u8], pc: usize) -> OdinInstruction {
    if pc >= bytecode.len() {
        return OdinInstruction::default();
    }

    let Some(code) = OdinHeadCode::from_u8(bytecode[pc])
    else {
        return OdinInstruction::default();
    };

    let size = try_instruction_size(bytecode, pc, code);
    if size == 0 {
        return OdinInstruction::default();
    }

    let mut operand1 = 0;
    let mut operand2 = 0;
    let mut operand3 = 0;

    if size >= 5 && pc + 4 < bytecode.len() {
        operand1 = i32::from_le_bytes(bytecode[pc + 1..pc + 5].try_into().expect("slice"));
    }
    if size >= 9 && pc + 8 < bytecode.len() {
        operand2 = i32::from_le_bytes(bytecode[pc + 5..pc + 9].try_into().expect("slice"));
    }
    if size >= 13 && pc + 12 < bytecode.len() {
        operand3 = i32::from_le_bytes(bytecode[pc + 9..pc + 13].try_into().expect("slice"));
    }

    OdinInstruction { code, size, operand1, operand2, operand3 }
}

/// 写入无操作数指令（`Plain` 形）。
pub fn emit_plain(code: &mut Vec<u8>, opcode: OdinHeadCode) {
    debug_assert_eq!(opcode.form(), OdinInstructionForm::Plain);
    code.push(opcode as u8);
}

/// 写入单立即数指令（`Imm1` 形，小端 `i32`）。
pub fn emit_imm1(code: &mut Vec<u8>, opcode: OdinHeadCode, operand: i32) {
    debug_assert_eq!(opcode.form(), OdinInstructionForm::Imm1);
    code.push(opcode as u8);
    code.extend_from_slice(&operand.to_le_bytes());
}

/// 写入双立即数指令（`Imm2` 形，两个小端 `i32`）。
pub fn emit_imm2(code: &mut Vec<u8>, opcode: OdinHeadCode, operand1: i32, operand2: i32) {
    debug_assert_eq!(opcode.form(), OdinInstructionForm::Imm2);
    code.push(opcode as u8);
    code.extend_from_slice(&operand1.to_le_bytes());
    code.extend_from_slice(&operand2.to_le_bytes());
}

fn try_instruction_size(bytecode: &[u8], pc: usize, code: OdinHeadCode) -> u8 {
    match code.form() {
        OdinInstructionForm::Plain => 1,
        OdinInstructionForm::Imm1 => {
            if pc + 4 < bytecode.len() {
                5
            }
            else {
                0
            }
        }
        OdinInstructionForm::Imm2 => {
            if pc + 8 < bytecode.len() {
                9
            }
            else {
                0
            }
        }
        OdinInstructionForm::Imm3 => {
            if pc + 12 < bytecode.len() {
                13
            }
            else {
                0
            }
        }
        OdinInstructionForm::Invalid => 0,
    }
}
