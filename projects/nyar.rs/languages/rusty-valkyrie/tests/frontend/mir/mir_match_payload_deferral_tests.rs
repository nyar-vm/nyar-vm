//! `SumPayloadGet` 不得在 match 判别（check）块急切发射。
//!
//! sum 变体的 nullary 载荷槽按合同为 null；判别块里急切提取载荷会让 wasm
//! `ref.cast`/`struct.get` 在 `None` scrutinee 上 trap。载荷必须在判别成功的
//! 后继块（`_check` / `if_let_then`）顶部物化。本组锁定 SSA 块结构：
//! 含 `SumVariantIs` 的判别块不得同块携带 `SumPayloadGet`。

use rusty_valkyrie::{MirLowerer, ValkyrieCompiler};
use rusty_valkyrie::compiler::mir::ssa::{MirModule, MirOperation};

const PRIMITIVES: &str = r#"
namespace std::primitive;
[primitive("std::primitive::i32")] structure i32 { }
"#;

const OPTION_DECL: &str = r#"
[tag(ProbeOptionKind)]
unite ProbeOption<T> {
    [tag(0)]
    [constructor]
    Some { value: T },
    [tag(1, default)]
    None,
}

micro make_none() -> ProbeOption<i32> {
    return None
}
"#;

fn compile(source: &str) -> MirModule {
    let source = format!("{PRIMITIVES}\n{OPTION_DECL}\n{source}");
    let hir = ValkyrieCompiler::new(rusty_valkyrie::compiler::types::SourceID { version_id: 9710 })
        .compile_source(&source)
        .expect("fixture 必须完成 HIR 分析");
    MirLowerer::lower_module_semantic(&hir)
}

/// 同一 SSA 块不得同时承担 tag 判别（`SumVariantIs`）与载荷提取（`SumPayloadGet`）。
fn assert_variant_is_never_shares_block_with_payload_get(module: &MirModule, context: &str) {
    let mut saw_variant_is = false;
    let mut saw_payload_get = false;
    for function in &module.functions {
        for block in &function.blocks {
            let has_variant_is =
                block.instructions.iter().any(|instruction| matches!(instruction.kind, MirOperation::SumVariantIs { .. }));
            let has_payload_get =
                block.instructions.iter().any(|instruction| matches!(instruction.kind, MirOperation::SumPayloadGet { .. }));
            saw_variant_is |= has_variant_is;
            saw_payload_get |= has_payload_get;
            assert!(
                !(has_variant_is && has_payload_get),
                "{context}: 块 `{}` 同时承担 tag 判别与载荷提取，未匹配路径会读 null 载荷",
                block.label
            );
        }
    }
    assert!(saw_variant_is, "{context}: fixture 必须真的产生 `SumVariantIs`");
    assert!(saw_payload_get, "{context}: fixture 必须真的产生 `SumPayloadGet`");
}

#[test]
fn match_check_block_never_extracts_payload_eagerly() {
    let module = compile(r#"
micro pick_or(input: ProbeOption<i32>, fallback: i32) -> i32 {
    match input {
        case Some(value):
            value
        case None:
            fallback
    }
}
"#);
    assert_variant_is_never_shares_block_with_payload_get(&module, "match 表达式");
}

/// 反例：嵌套 extractor（`Some(Some(x))`）当前只消费外层 tag 判别与外层载荷；
/// 内层 tag 不构成匹配条件。锁定现状为显式边界，不允许静默扩展语义。
#[test]
fn nested_extractor_matches_on_outer_tag_only() {
    let module = compile(r#"
micro nested(input: ProbeOption<ProbeOption<i32>>, fallback: i32) -> i32 {
    match input {
        case Some(Some(value)):
            value
        else:
            fallback
    }
}
"#);
    let function = module.functions.iter().find(|function| function.display_symbol.contains("nested")).expect("fixture 缺少 `nested`");
    let variant_is_count = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| matches!(instruction.kind, MirOperation::SumVariantIs { .. }))
        .count();
    assert_eq!(variant_is_count, 1, "嵌套 extractor 当前只判别外层 tag；内层判别语义未定案前不得静默扩展");
}
