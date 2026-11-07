use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    mir::{
        ssa::{MirDiagnostic, MirLowerer, MirModule, MirOperation, MirOperand},
        validation::validate_semantic_module,
    },
    types::{
        Identifier,
        hir::{HirTupleField, ValkyrieType},
    },
};

const PRIMITIVES: &str = r#"
[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::i64")]
structure i64 { }
"#;

fn compile(source: &str) -> MirModule {
    let hir = ValkyrieCompiler::default()
        .compile_source(&format!("{PRIMITIVES}\n{source}"))
        .unwrap_or_else(|error| panic!("源码必须通过 HIR: {error}"));
    MirLowerer::lower_module_semantic(&hir)
}

fn only_function(module: &MirModule) -> &rusty_valkyrie::mir::ssa::MirFunction {
    module.functions.first().expect("至少一个函数")
}

fn instructions(module: &MirModule) -> impl Iterator<Item = &rusty_valkyrie::mir::ssa::MirInstruction> {
    module.functions.iter().flat_map(|function| function.blocks.iter()).flat_map(|block| &block.instructions)
}

#[test]
fn labeled_tuple_field_access_lowers_to_positional_tuple_get() {
    let module = compile(
        "micro first(pair: (ordinal: i32, value: i64)) -> i32 { return pair.ordinal } \
         micro second(pair: (ordinal: i32, value: i64)) -> i64 { return pair.value }",
    );
    assert!(
        !module.diagnostics.iter().any(|diagnostic| matches!(diagnostic, MirDiagnostic::UnlabeledTupleFieldAccess { .. })),
        "带标签元组不得产生无名元组诊断: {:?}",
        module.diagnostics
    );
    let mut got = Vec::new();
    for function in &module.functions {
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let MirOperation::TupleGet { object, index } = &instruction.kind {
                let MirOperand::Value(object) = object
                else {
                    panic!("元组读取对象必须是 SSA 值");
                };
                assert_eq!(function.value_types[object], labeled_pair());
                got.push((function.symbol.to_string(), *index, function.value_types[&instruction.results[0]].clone()));
            }
        }
    }
    got.sort();
    assert_eq!(
        got,
        vec![
            ("main.first".to_string(), 0, ValkyrieType::Integer32 { signed: true }),
            ("main.second".to_string(), 1, ValkyrieType::Integer64 { signed: true }),
        ],
        "标签解析必须冻结成位置索引，且结果类型取字段类型"
    );
}

#[test]
fn unlabeled_tuple_field_access_is_a_structured_failure() {
    let module = compile("micro first(pair: (i32, i64)) -> i32 { return pair.ordinal }");
    let diagnostics = module
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic, MirDiagnostic::UnlabeledTupleFieldAccess { .. }))
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1, "无名元组按名访问必须结构化失败，实际诊断: {:?}", module.diagnostics);
    assert!(
        !instructions(&module).any(|instruction| matches!(instruction.kind, MirOperation::TupleGet { .. })),
        "禁止退回按位置解析"
    );
}

#[test]
fn explicit_reconstruction_produces_unlabeled_tuple() {
    let module = compile(
        "micro strip(pair: (ordinal: i32, value: i64)) -> (i32, i64) { return (pair.ordinal, pair.value) }",
    );
    assert!(module.diagnostics.is_empty(), "显式重构造不得产生诊断: {:?}", module.diagnostics);
    validate_semantic_module(&module).expect("显式重构造须通过语义校验");
    let function = only_function(&module);
    let tuple_news = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| matches!(instruction.kind, MirOperation::TupleNew { .. }))
        .collect::<Vec<_>>();
    assert_eq!(tuple_news.len(), 1, "必须产出一个无标签元组字面量");
    assert_eq!(
        function.value_types[&tuple_news[0].results[0]],
        // `i32`/`i64` 的 `[primitive]` 声明绑定到数值载体身份，字面量结果按载体收敛。
        ValkyrieType::Tuple(vec![
            ValkyrieType::Integer32 { signed: true },
            ValkyrieType::Integer64 { signed: true },
        ]),
        "字面量没有标签语法，结果必须是 `Tuple` 而不是 `LabeledTuple`"
    );
}

#[test]
fn implicit_assignment_from_labeled_to_positional_tuple_is_rejected() {
    let error = ValkyrieCompiler::default()
        .compile_source(&format!(
            "{PRIMITIVES}\nmicro strip(pair: (ordinal: i32, value: i64)) -> i32 {{ let p: (i32, i64) = pair; return 0 }}"
        ))
        .expect_err("隐式跨形态赋值必须结构化报错");
    assert!(
        matches!(error, HirError::LetInitializerTypeMismatch { .. }),
        "期望 `LetInitializerTypeMismatch`，实际 {error:?}"
    );
}

fn labeled_pair() -> ValkyrieType {
    // `[primitive]` 声明绑定到数值载体身份，带标签元组的字段类型同样按载体收敛。
    ValkyrieType::LabeledTuple(vec![
        HirTupleField { name: Identifier::new("ordinal"), ty: ValkyrieType::Integer32 { signed: true } },
        HirTupleField { name: Identifier::new("value"), ty: ValkyrieType::Integer64 { signed: true } },
    ])
}
