//! 字符/字节字面量的 MIR 编码：值按整数常量编码，类型归属 `char`/`u8`/形参特化。
//!
//! fixture 用 `[primitive]` 声明身份的内建名，不依赖任何 std adaptor。

use rusty_valkyrie::{
    MirLowerer, ValkyrieCompiler,
    compiler::{
        mir::ssa::{MirModule, MirOperation},
        types::hir::types::ValkyrieType,
    },
};

/// 数值/文本原始类型不是语言内建，必须绑定到源码声明身份。
const PRIMITIVES: &str = r#"
[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::u8")]
structure u8 { }

[primitive("std::primitive::u32")]
structure u32 { }

[primitive("std::text::char")]
structure char { }
"#;

fn compile(source: &str) -> MirModule {
    let hir = ValkyrieCompiler::default()
        .compile_source(&format!("{PRIMITIVES}\n{source}"))
        .expect("fixture 必须完成 HIR 分析");
    MirLowerer::lower_module_semantic(&hir)
}

/// 收集 `name` 函数内全部 `LoadConstant` 的（常量, 类型）。
fn load_constants(module: &MirModule, name: &str) -> Vec<(rusty_valkyrie::compiler::mir::ssa::MirConstant, Option<ValkyrieType>)> {
    let function =
        module.functions.iter().find(|function| function.display_symbol.contains(name)).unwrap_or_else(|| panic!("缺少函数 `{name}`"));
    function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::LoadConstant { constant, ty } => Some((constant.clone(), ty.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn character_literal_encodes_as_char_typed_constant() {
    let module = compile("micro probe(): i32 { let space: char = ' ' return 0 }");
    let constants = load_constants(&module, "probe");
    let space = constants.iter().find(|(constant, _)| matches!(constant, rusty_valkyrie::compiler::mir::ssa::MirConstant::Int(32)));
    let Some((_, Some(ty))) = space
    else {
        panic!("空格字符必须产出带类型的整数常量，实际 {constants:?}")
    };
    assert!(
        matches!(ty, ValkyrieType::Character) || matches!(ty, ValkyrieType::Named(name) if name.as_str() == "char"),
        "字符字面量必须归 char，实际 {ty:?}"
    );
}

#[test]
fn byte_literal_encodes_as_u8_typed_constant() {
    let module = compile("micro probe(): i32 { let zero: u8 = b'0' return 0 }");
    let constants = load_constants(&module, "probe");
    let zero = constants.iter().find(|(constant, _)| matches!(constant, rusty_valkyrie::compiler::mir::ssa::MirConstant::Int(48)));
    let Some((_, Some(ty))) = zero
    else {
        panic!("字节字面量必须产出带类型的整数常量，实际 {constants:?}")
    };
    assert!(
        matches!(ty, ValkyrieType::Integer8 { signed: false }) || matches!(ty, ValkyrieType::Named(name) if name.as_str() == "u8"),
        "字节字面量必须归 u8，实际 {ty:?}"
    );
}

#[test]
fn byte_literal_writes_specialized_formal_type_in_call() {
    // `b' '` 对 `u32` 形参特化后，MIR 常量必须写 `u32` 一侧而不是默认 `u8`（SMIR007 同源约束）。
    let module = compile(
        r#"
micro takes_u32(value: u32): bool { return true }
micro probe(): bool { return takes_u32(b' ') }
"#,
    );
    let constants = load_constants(&module, "probe");
    let space = constants.iter().find(|(constant, _)| matches!(constant, rusty_valkyrie::compiler::mir::ssa::MirConstant::Int(32)));
    let Some((_, Some(ty))) = space
    else {
        panic!("调用实参必须是带类型常量，实际 {constants:?}")
    };
    assert!(
        matches!(ty, ValkyrieType::Integer32 { signed: false }) || matches!(ty, ValkyrieType::Named(name) if name.as_str() == "u32"),
        "字节字面量对 u32 形参必须写 u32，实际 {ty:?}"
    );
}
