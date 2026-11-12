//! 无标签元组位置成员访问（`pair.0` / `pair.1`）的 HIR 冻结与 MIR `TupleGet` 降低。
//!
//! 规范依据：`language.md` Cardinal 一节裁定合同允许元组索引；位置访问是正式结构操作，
//! 解析器接受整数字面量成员名，HIR 冻结位置事实，MIR 复用既有 `TupleGet`，不扩 opcode。

use rusty_valkyrie::{
    ValkyrieCompiler,
    mir::{
        ssa::{MirDiagnostic, MirLowerer, MirModule, MirOperand, MirOperation},
        validation::validate_semantic_module,
    },
    types::hir::ValkyrieType,
};

const PRIMITIVES: &str = r#"
[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::i64")]
structure i64 { }

[primitive("std::primitive::usize")]
structure usize { }

[primitive("std::text::char")]
structure char { }
"#;

fn compile(source: &str) -> MirModule {
    let hir = ValkyrieCompiler::default()
        .compile_source(&format!("{PRIMITIVES}\n{source}"))
        .unwrap_or_else(|error| panic!("源码必须通过 HIR: {error}"));
    MirLowerer::lower_module_semantic(&hir)
}

fn instructions(module: &MirModule) -> impl Iterator<Item = &rusty_valkyrie::mir::ssa::MirInstruction> {
    module.functions.iter().flat_map(|function| function.blocks.iter()).flat_map(|block| &block.instructions)
}

/// 正例：`(usize, char)` 的 `.0` / `.1` 冻结成位置事实并降低为 `TupleGet`，结果类型取元素类型。
#[test]
fn unlabeled_tuple_position_access_lowers_to_tuple_get() {
    let module = compile(
        "micro first(pair: (usize, char)) -> usize { return pair.0 } \
         micro second(pair: (usize, char)) -> char { return pair.1 }",
    );
    assert!(
        !module.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MirDiagnostic::UnlabeledTupleFieldAccess { .. } | MirDiagnostic::TupleIndexOutOfRange { .. }
        )),
        "位置访问不得产生按名访问或越界诊断: {:?}",
        module.diagnostics
    );
    validate_semantic_module(&module).expect("位置访问须通过语义校验");
    let mut got = Vec::new();
    for function in &module.functions {
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let MirOperation::TupleGet { object, index } = &instruction.kind {
                let MirOperand::Value(object) = object
                else {
                    panic!("元组读取对象必须是 SSA 值");
                };
                let ValkyrieType::Tuple(members) = &function.value_types[object]
                else {
                    panic!("对象类型必须是无标签元组: {:?}", function.value_types[object]);
                };
                assert_eq!(members.len(), 2, "fixture 元组 arity 为 2");
                let result_type = &function.value_types[&instruction.results[0]];
                assert_eq!(result_type, &members[*index as usize], "读取结果类型必须取该位置的元素类型");
                got.push((function.symbol.to_string(), *index));
            }
        }
    }
    got.sort();
    assert_eq!(got, vec![("main.first".to_string(), 0), ("main.second".to_string(), 1)], "数字成员名必须冻结成源码书写的位置索引");
}

/// 正例：元组字面量构造后 `let x = p.0` 的推断类型是元素类型，MIR 产出 `TupleGet`。
#[test]
fn tuple_literal_position_access_infers_element_type() {
    let module = compile("micro pick() -> i64 { let p: (i64, i64) = (1i64, 2i64); let x = p.0; return x }");
    assert!(module.diagnostics.is_empty(), "构造后位置读取不得产生诊断: {:?}", module.diagnostics);
    validate_semantic_module(&module).expect("构造后位置读取须通过语义校验");
    let tuple_gets =
        instructions(&module).filter(|instruction| matches!(instruction.kind, MirOperation::TupleGet { index: 0, .. })).collect::<Vec<_>>();
    assert_eq!(tuple_gets.len(), 1, "必须产出一个位置 0 的 TupleGet");
}

/// 反例：非元组接收者上的数字成员名不得按位置放宽，保持结构化字段失败。
#[test]
fn position_member_on_non_tuple_is_structured_failure() {
    let module = compile("micro bad(x: i64) -> i64 { return x.0 }");
    assert!(
        module.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MirDiagnostic::UnresolvedFieldIdentity { field } | MirDiagnostic::UnlabeledTupleFieldAccess { field }
            if field == "0"
        )),
        "非元组上的 `.0` 必须结构化失败，实际诊断: {:?}",
        module.diagnostics
    );
    assert!(
        !instructions(&module).any(|instruction| matches!(instruction.kind, MirOperation::TupleGet { .. })),
        "禁止对非元组对象产出 TupleGet"
    );
}

/// 反例：`(i64, i64)` 上的 `.2` 越界是结构化错误 `TupleIndexOutOfRange`，不得静默。
#[test]
fn tuple_index_out_of_range_is_structured_failure() {
    let module = compile("micro bad(pair: (i64, i64)) -> i64 { return pair.2 }");
    let out_of_range = module
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic, MirDiagnostic::TupleIndexOutOfRange { index, arity, .. } if *index == 2 && *arity == 2))
        .collect::<Vec<_>>();
    assert_eq!(out_of_range.len(), 1, "越界位置必须给出恰好一条结构化诊断，实际: {:?}", module.diagnostics);
    assert!(!instructions(&module).any(|instruction| matches!(instruction.kind, MirOperation::TupleGet { .. })), "越界读取不得产出 TupleGet");
}
