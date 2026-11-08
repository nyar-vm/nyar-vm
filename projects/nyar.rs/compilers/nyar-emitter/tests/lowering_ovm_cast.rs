//! OVM `as` 表示转换 lowering：规则只看「源符号性 + 两端位宽」。
//!
//! 位宽 ≤ 32 以 `i32` 为载体（窄整数必须处于自身类型的规范形态），64 位以 `i64`
//! 为载体。拓宽按源符号性做符号/零扩展，收窄按目标位宽截断，等宽换符号性保持位模式。
//! 整数↔`f64` 用带符号性区分的宿主转换（无符号转换不得复用有符号指令），
//! `f64`→整数走 checked 语义（朝零截断，NaN / 越界结构化失败）。
//! 没有冻结语义的组合（`f32`、布尔/文本↔数值）必须结构化失败，不许近似。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{
        ArrayInitialization, Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef,
    },
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::{InstructionId, NyarType, ProvenanceId};
use odin_bytecode::{OdinConstant, OdinHeadCode, OdinInstruction, OdinModuleData, decode_at};

fn i8_(signed: bool) -> NyarType {
    NyarType::Integer8 { signed }
}

fn i16_(signed: bool) -> NyarType {
    NyarType::Integer16 { signed }
}

fn i32_(signed: bool) -> NyarType {
    NyarType::Integer32 { signed }
}

fn i64_(signed: bool) -> NyarType {
    NyarType::Integer64 { signed }
}

fn main_name() -> QualifiedName {
    QualifiedName::new(vec![Identifier::new("main")])
}

/// `main(source: S) -> T { Cast(source) }`。
fn cast_function(source_type: NyarType, target_type: NyarType) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: target_type.clone(),
        param_types: vec![source_type.clone()],
        value_types: BTreeMap::from([(ValueRef(0), source_type), (ValueRef(1), target_type.clone())]),
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: vec![ValueRef(0)],
            instructions: vec![Instruction {
                id: InstructionId::from_index(1).expect("指令身份"),
                results: vec![ValueRef(1)],
                kind: InstructionKind::Cast { source: Operand::Value(ValueRef(0)), ty: target_type },
                provenance: ProvenanceId::from_index(1).expect("provenance"),
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
        }],
        diagnostics: Vec::new(),
    }
}

/// 常量源：`main() -> T { Cast(5) }`，源位宽必须来自 semantic MIR 的常量合同。
fn cast_constant_function(target_type: NyarType) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: target_type.clone(),
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), target_type.clone())]),
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: vec![Instruction {
                id: InstructionId::from_index(1).expect("指令身份"),
                results: vec![ValueRef(0)],
                kind: InstructionKind::Cast { source: Operand::Constant(Constant::Int(5)), ty: target_type },
                provenance: ProvenanceId::from_index(1).expect("provenance"),
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

fn try_lower(function: ExecutableFunction) -> Result<OdinModuleData, String> {
    let name = main_name();
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(name.clone(), function)]));
    plan.testing_module_name("ovm_cast").testing_entry_operation(name);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

fn main_instructions(module: &OdinModuleData) -> Vec<OdinInstruction> {
    let main = module.functions.iter().find(|function| function.name == "main").expect("main 必须导出");
    decode_all(module, main.code_offset, main.code_length)
}

fn decode_all(module: &OdinModuleData, offset: i32, length: i32) -> Vec<OdinInstruction> {
    let start = offset as usize;
    let end = start + length as usize;
    let mut pc = start;
    let mut instructions = Vec::new();
    while pc < end {
        let instruction = decode_at(&module.code_bytes, pc);
        assert!(instruction.size > 0, "无法解码的字节码 pc={pc}");
        pc += usize::from(instruction.size);
        instructions.push(instruction);
    }
    instructions
}

/// `CallImport` 的宿主符号序列（其余指令不计）。
fn host_symbols(module: &OdinModuleData) -> Vec<String> {
    main_instructions(module)
        .iter()
        .filter(|instruction| instruction.code == OdinHeadCode::CallImport)
        .map(|instruction| module.imports.get(instruction.operand1 as usize).expect("导入下标必须有效").symbol_name.clone())
        .collect()
}

fn codes(module: &OdinModuleData) -> Vec<OdinHeadCode> {
    main_instructions(module).iter().map(|instruction| instruction.code).collect()
}

fn pool_integer32(module: &OdinModuleData) -> Vec<i32> {
    module
        .constants
        .iter()
        .filter_map(|constant| match constant {
            OdinConstant::Integer32(value) => Some(*value),
            _ => None,
        })
        .collect()
}

fn pool_integer64(module: &OdinModuleData) -> Vec<i64> {
    module
        .constants
        .iter()
        .filter_map(|constant| match constant {
            OdinConstant::Integer64(value) => Some(*value),
            _ => None,
        })
        .collect()
}

/// 常量池去重后的整数键（发射侧每次 `intern` 都会追加，池内可重复）。
fn unique_pool_integer32(module: &OdinModuleData) -> Vec<i32> {
    let mut values = pool_integer32(module);
    values.sort_unstable();
    values.dedup();
    values
}

fn unique_pool_integer64(module: &OdinModuleData) -> Vec<i64> {
    let mut values = pool_integer64(module);
    values.sort_unstable();
    values.dedup();
    values
}

fn lower(source: NyarType, target: NyarType) -> OdinModuleData {
    try_lower(cast_function(source, target)).unwrap_or_else(|error| panic!("Cast 必须能 lowering: {error}"))
}

/// 等宽换符号性（`u64 → i64`）位保持：不产生任何宿主调用或常量，但必须搬运到结果槽。
#[test]
fn u64_to_i64_preserves_bits_and_still_stores_result() {
    let module = lower(i64_(false), i64_(true));
    assert!(host_symbols(&module).is_empty(), "u64→i64 只是解释变化，不得产生转换调用: {:?}", host_symbols(&module));
    assert!(pool_integer32(&module).is_empty() && pool_integer64(&module).is_empty(), "u64→i64 不得产生常量");
    assert!(
        codes(&module).contains(&OdinHeadCode::StoreLocal),
        "Cast 命中结果槽是语义的一部分，跳过不发射会让槽停留在默认值: {:?}",
        codes(&module)
    );
}

/// 有符号窄化到宽化：`i32 → i64` 符号扩展。
#[test]
fn i32_to_i64_widens_with_sign_extension() {
    let module = lower(i32_(true), i64_(true));
    assert_eq!(host_symbols(&module), vec!["i32_to_i64"], "i32→i64 只需一次符号扩展");
}

/// 窄无符号先按自身位宽规范化再拓宽：`u16 → i64` = 掩码 + 符号扩展（值已非负即零扩展）。
#[test]
fn u16_to_i64_masks_then_widens() {
    let module = lower(i16_(false), i64_(true));
    assert_eq!(host_symbols(&module), vec!["i32_to_i64"], "u16→i64 的拓宽入口不变: {:?}", host_symbols(&module));
    assert_eq!(unique_pool_integer32(&module), vec![0xFFFF], "u16 必须先掩码到自身位宽");
    assert!(codes(&module).contains(&OdinHeadCode::I32And), "u16 规范化必须发射位与: {:?}", codes(&module));
}

/// `u32` 半程落在 `i32` 载体上为负：零扩展必须补 2^32，不能只靠符号扩展。
#[test]
fn u32_to_i64_zero_extends_upper_half() {
    let module = lower(i32_(false), i64_(true));
    assert_eq!(
        host_symbols(&module),
        vec!["i32_to_i64", "i32_to_i64", "i64_shr_u", "i64_mul", "i64_add"],
        "u32→i64 必须按符号位补 2^32: {:?}",
        host_symbols(&module)
    );
    assert_eq!(unique_pool_integer64(&module), vec![1i64 << 32], "补偿量必须是 i64 常量 2^32");
    assert!(codes(&module).contains(&OdinHeadCode::Dup), "u32→i64 需要复制原值以同时求符号位");
}

/// `Character`（32 位无符号码位）走同一套无符号 32 位规则，不按类型名特判。
#[test]
fn character_to_i64_uses_unsigned_32_rule() {
    let module = lower(NyarType::Character, i64_(true));
    assert_eq!(
        host_symbols(&module),
        vec!["i32_to_i64", "i32_to_i64", "i64_shr_u", "i64_mul", "i64_add"],
        "Character→i64 必须复用 u32 规则: {:?}",
        host_symbols(&module)
    );
}

/// `i64 → i32` 收窄是位截断，不是范围校验：先截断再走必然成功的窄化。
#[test]
fn i64_to_i32_truncates_before_checked_narrowing() {
    let module = lower(i64_(true), i32_(true));
    assert_eq!(
        host_symbols(&module),
        vec!["i32_to_i64", "i64_shl", "i32_to_i64", "i64_shr", "i64_to_i32"],
        "i64→i32 必须先截断低 32 位再窄化: {:?}",
        host_symbols(&module)
    );
    assert_eq!(unique_pool_integer32(&module), vec![32], "截断位移量 32 进 i32 池");
    assert!(pool_integer64(&module).is_empty(), "位移量可由 i32 常量拓宽，不得占用 i64 池");
}

/// `i64 → u8`：截断到 32 位后再按目标位宽掩码。
#[test]
fn i64_to_u8_truncates_then_masks() {
    let module = lower(i64_(true), i8_(false));
    assert_eq!(
        host_symbols(&module),
        vec!["i32_to_i64", "i64_shl", "i32_to_i64", "i64_shr", "i64_to_i32"],
        "i64→u8 的换载体步骤与 i64→i32 相同: {:?}",
        host_symbols(&module)
    );
    assert_eq!(unique_pool_integer32(&module), vec![32, 0xFF], "目标位宽掩码必须按 u8 生成");
    assert_eq!(
        codes(&module).iter().filter(|code| **code == OdinHeadCode::I32And).count(),
        1,
        "目标位宽规范化只发一次掩码: {:?}",
        codes(&module)
    );
}

/// `i64 → i16`：截断到 32 位后按有符号目标做符号扩展（先移位保留低 16 位）。
#[test]
fn i64_to_i16_truncates_then_sign_extends() {
    let module = lower(i64_(true), i16_(true));
    assert_eq!(unique_pool_integer32(&module), vec![16, 32], "目标位宽 16 的移位量必须进池");
    assert_eq!(
        codes(&module).iter().filter(|code| **code == OdinHeadCode::I32Shl).count(),
        1,
        "符号扩展只在目标位宽上做一次左移: {:?}",
        codes(&module)
    );
    assert!(codes(&module).contains(&OdinHeadCode::I32ShrS), "有符号收窄必须做算术右移: {:?}", codes(&module));
}

/// `i8 → i32`：源规范化（符号扩展）后目标 32 位无需再动。
#[test]
fn i8_to_i32_sign_extends_source_only() {
    let module = lower(i8_(true), i32_(true));
    assert!(host_symbols(&module).is_empty(), "窄整数到 32 位不得产生宿主调用: {:?}", host_symbols(&module));
    assert_eq!(unique_pool_integer32(&module), vec![24], "i8 符号扩展位移量必须是 32-8=24");
    assert!(codes(&module).contains(&OdinHeadCode::I32ShrS), "有符号源必须算术右移: {:?}", codes(&module));
}

/// `i32 → u32`：位模式不变，但结果仍要落到结果槽。
#[test]
fn i32_to_u32_keeps_bits() {
    let module = lower(i32_(true), i32_(false));
    assert!(host_symbols(&module).is_empty(), "等宽换符号性无转换调用");
    assert!(codes(&module).contains(&OdinHeadCode::StoreLocal), "必须写出结果槽");
}

/// 常量源按 semantic MIR 合同是 `Integer64`：先按 i64 槽发射（小值走 `i32_to_i64`），再按目标收窄。
#[test]
fn constant_source_uses_semantic_integer64_width() {
    let module = try_lower(cast_constant_function(i32_(true))).expect("常量源 Cast 必须能 lowering");
    assert!(unique_pool_integer32(&module).contains(&5), "常量源必须以 Integer32 载荷进池: {:?}", module.constants);
    assert!(pool_integer64(&module).is_empty(), "小值不需要 Integer64 池条目");
    let symbols = host_symbols(&module);
    assert_eq!(
        symbols,
        vec!["i32_to_i64", "i32_to_i64", "i64_shl", "i32_to_i64", "i64_shr", "i64_to_i32"],
        "常量源必须按 semantic `Int → Integer64` 拓宽后再走 i64→i32 截断: {:?}",
        symbols
    );
}

/// 布尔 → 整数没有冻结语义：结构化失败，不得把 `true` 近似成 1。
#[test]
fn boolean_to_integer_fails_structured() {
    let error = try_lower(cast_function(NyarType::Boolean, i32_(true))).expect_err("布尔→整数必须结构化失败");
    assert!(error.contains("没有 `bool` → `i32`") && error.contains("确定转换语义"), "错误必须点名缺失的转换: {error}");
}

/// `i64 → f64`：有符号源必须走 `i64_to_f64`。
#[test]
fn i64_to_f64_uses_signed_conversion() {
    let module = lower(i64_(true), NyarType::Float64);
    assert_eq!(host_symbols(&module), vec!["i64_to_f64"], "有符号 64 位转 f64 必须走有符号转换");
}

/// `u64 → f64`：必须走无符号转换，复用 `i64_to_f64` 会让 2^63 以上的值静默变负。
#[test]
fn u64_to_f64_uses_unsigned_conversion() {
    let module = lower(i64_(false), NyarType::Float64);
    assert_eq!(host_symbols(&module), vec!["u64_to_f64"], "无符号 64 位转 f64 必须走无符号转换");
}

/// `i32 → f64`：有符号 32 位源走 `i32_to_f64`。
#[test]
fn i32_to_f64_uses_signed_conversion() {
    let module = lower(i32_(true), NyarType::Float64);
    assert_eq!(host_symbols(&module), vec!["i32_to_f64"]);
}

/// `u32` / `Character → f64`：无符号 32 位载体走 `u32_to_f64`。
#[test]
fn u32_and_character_to_f64_use_unsigned_conversion() {
    assert_eq!(host_symbols(&lower(i32_(false), NyarType::Float64)), vec!["u32_to_f64"]);
    assert_eq!(host_symbols(&lower(NyarType::Character, NyarType::Float64)), vec!["u32_to_f64"]);
}

/// 窄无符号整数先在 `i32` 载体上掩码成非负，再走无符号转换。
#[test]
fn narrow_unsigned_to_f64_masks_before_conversion() {
    let module = lower(i8_(false), NyarType::Float64);
    assert!(unique_pool_integer32(&module).contains(&0xFF), "必须先掩掉高位: {:?}", module.constants);
    assert!(codes(&module).contains(&OdinHeadCode::I32And), "掩码必须在转换前发射");
    assert_eq!(host_symbols(&module), vec!["u32_to_f64"]);
}

/// `f64 → i64`：checked 语义（截断后越界/NaN 结构化失败，不饱和）。
#[test]
fn f64_to_i64_uses_checked_conversion() {
    let module = lower(NyarType::Float64, i64_(true));
    assert_eq!(host_symbols(&module), vec!["f64_to_i64_checked"], "`as` 不得复用饱和版 `f64_to_i64`");
}

/// `f64 → u64`：checked 无符号语义。
#[test]
fn f64_to_u64_uses_checked_unsigned_conversion() {
    let module = lower(NyarType::Float64, i64_(false));
    assert_eq!(host_symbols(&module), vec!["f64_to_u64_checked"]);
}

/// `f64 → u8`：先 checked 转 `u32`，再按目标位宽掩码（与整数收窄口径一致）。
#[test]
fn f64_to_u8_normalizes_after_checked_conversion() {
    let module = lower(NyarType::Float64, i8_(false));
    assert!(unique_pool_integer32(&module).contains(&0xFF), "窄目标必须掩掉高位: {:?}", module.constants);
    assert!(codes(&module).contains(&OdinHeadCode::I32And), "掩码必须在 checked 转换之后发射");
    assert_eq!(host_symbols(&module), vec!["f64_to_u32_checked"]);
}

/// `f64 → i16`：先 checked 转 `i32`，再做 16 位符号扩展。
#[test]
fn f64_to_i16_sign_extends_after_checked_conversion() {
    let module = lower(NyarType::Float64, i16_(true));
    assert!(codes(&module).contains(&OdinHeadCode::I32Shl) && codes(&module).contains(&OdinHeadCode::I32ShrS), "必须有符号扩展");
    assert_eq!(host_symbols(&module), vec!["f64_to_i32_checked"]);
}

/// `f32` 是独立的 OVM 浮点载体：`f32`↔`f64` 走专用单步头码，不得借另一精度中转。
#[test]
fn f32_f64_conversions_use_dedicated_codes() {
    let promote = lower(NyarType::Float32, NyarType::Float64);
    assert!(host_symbols(&promote).is_empty(), "f32→f64 不得产生宿主调用: {:?}", host_symbols(&promote));
    assert!(codes(&promote).contains(&OdinHeadCode::F64PromoteF32), "f32→f64 必须走 `F64PromoteF32`: {:?}", codes(&promote));

    let demote = lower(NyarType::Float64, NyarType::Float32);
    assert!(host_symbols(&demote).is_empty(), "f64→f32 不得产生宿主调用: {:?}", host_symbols(&demote));
    assert!(codes(&demote).contains(&OdinHeadCode::F32DemoteF64), "f64→f32 必须走 `F32DemoteF64`: {:?}", codes(&demote));
}

/// 整数↔`f32` 的单步精确转换尚未冻结：双向都必须结构化失败，
/// 不得借 `f64` 中转（double rounding）或用 `i32` 位模式冒充数值转换。
#[test]
fn integer_f32_casts_fail_structured() {
    let error = try_lower(cast_function(NyarType::Float32, i64_(true))).expect_err("f32→整数必须结构化失败");
    assert!(
        error.contains("`f32` → `i64`") && error.contains("`f32` 是独立载体") && error.contains("禁止借 `f64` 中转"),
        "错误必须点名缺失的转换与独立载体约束: {error}"
    );
    let error = try_lower(cast_function(i64_(true), NyarType::Float32)).expect_err("整数→f32 必须结构化失败");
    assert!(error.contains("`i64` → `f32`") && error.contains("尚未冻结"), "错误必须点名两端类型与未冻结的单步转换: {error}");
}

/// 浮点 → 布尔/文本没有 `as` 合同：结构化失败。
#[test]
fn float_to_boolean_fails_structured() {
    let error = try_lower(cast_function(NyarType::Float64, NyarType::Boolean)).expect_err("f64→bool 必须结构化失败");
    assert!(error.contains("`f64` → `bool`"), "错误必须点名两端类型: {error}");
}

/// 整数 → 文本没有冻结语义：结构化失败。
#[test]
fn integer_to_text_fails_structured() {
    let error = try_lower(cast_function(i64_(true), NyarType::Utf8)).expect_err("整数→文本必须结构化失败");
    assert!(error.contains("utf8"), "错误必须点名目标类型: {error}");
}

/// 运行时长度的语言数组构造（canonical `ArrayNew`）在 OVM 尚无实现：
/// 必须保持结构化失败，不得静默产出空槽。
#[test]
fn array_new_remains_fail_closed() {
    let array_type = NyarType::Array(Box::new(i64_(true)));
    let mut function = cast_function(i64_(true), array_type.clone());
    function.value_types.insert(ValueRef(1), array_type.clone());
    function.return_type = array_type.clone();
    function.blocks[0].instructions[0].kind =
        InstructionKind::ArrayNew { array_type, length: Operand::Value(ValueRef(0)), initialization: ArrayInitialization::Default };
    let error = try_lower(function).expect_err("ArrayNew 在 OVM 必须结构化失败");
    assert!(error.contains("OVM 后端未实现指令 ArrayNew"), "错误必须是 fail-closed 的指令拒绝: {error}");
}
