//! 数组长度结构操作的前端身份只来自源码声明。
//!
//! `core/source/array/length.v` 中的
//! `[intrinsic("builtin.array.length")] micro length<T>(array: [T]): usize { }`
//! 是唯一前端来源；删除该声明后调用必须结构化失败，前端不得合成候选。

use rusty_valkyrie::{
    HirError, MirLowerer, MirOperation, ValkyrieCompiler,
    types::hir::{BuiltinStructureOp, HirExpr, HirExprKind, HirFunction, ValkyrieType},
};

/// 数值原始类型名属于 `core/primitive` 的声明族；未声明时 canonical 以 CAN003 拒绝。
const PRIMITIVES: &str = "[primitive(\"core::primitive::usize\")] structure usize { } \
    [primitive(\"core::primitive::i32\")] structure i32 { } ";

/// 与 `valkyrie.v` core 声明逐字对齐的唯一前端来源。
const ARRAY_LENGTH_INTRINSIC_DECLARATION: &str = "[intrinsic(\"builtin.array.length\")] micro length<T>(array: [T]): usize { } ";

/// 与 `asgard.router` 相同的调用形式：`length(segments)`。
const PROBE: &str = "micro probe(values: [i32]) -> usize { return length(values) }";

/// 语句位置的调用没有返回注解可补，是天然的“无上下文类型事实”样本。
const STATEMENT_PROBE: &str = "micro probe(values: [i32]) { length(values) }";

fn probe_source(intrinsic_declaration: Option<&str>) -> String {
    format!("{PRIMITIVES}{}{PROBE}", intrinsic_declaration.unwrap_or(""))
}

fn find_function<'a>(module: &'a rusty_valkyrie::compiler::types::hir::HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 `{name}` 声明"))
}

fn probe_return_call(function: &HirFunction) -> &HirExpr {
    // 尾部 `return` 是块尾表达式（`body.expr`），不是普通语句。
    let expression = function.body.expr.as_deref().unwrap_or_else(|| panic!("expected trailing return expression: {:?}", function.body));
    let HirExprKind::Return(Some(expression)) = &expression.kind
    else {
        panic!("expected return expression: {expression:?}");
    };
    expression.as_ref()
}

#[test]
fn declared_array_length_intrinsic_degrades_to_ordinary_call() {
    // 核心合同：源码 intrinsic 自由函数**永远降级为普通 Call**，不得产出 intrinsic 专用 opcode。
    // intrinsic 专用 opcode 只允许由 adaptor 链接进来的后端专用绑定产生。
    let compiler = ValkyrieCompiler::default();
    let hir = compiler.compile_source(&probe_source(Some(ARRAY_LENGTH_INTRINSIC_DECLARATION))).expect("源码内声明的数组长度结构操作必须解析");

    let expression = probe_return_call(find_function(&hir, "probe"));
    let HirExprKind::Call { callee, args, resolved } = &expression.kind
    else {
        panic!("expected length call, got {expression:?}");
    };
    assert_eq!(args.len(), 1, "receiver must be the only argument: {args:?}");
    let resolved = resolved.as_ref().expect("length call must resolve through the declared intrinsic candidate");
    // HIR 层仍然保留 intrinsic identity（由 overload 阶段结构化绑定）——这是正确的，
    // intrinsic 身份是语义事实，不是 lowering 策略。
    assert_eq!(resolved.intrinsic, Some(BuiltinStructureOp::ArrayLen), "数组长度身份必须来自声明冻结的 intrinsic，而不是调用点名字");
    assert!(
        matches!(resolved.return_type, ValkyrieType::Nominal { .. }),
        "声明可见的 usize 必须已冻结为 Nominal identity，got {:?}",
        resolved.return_type
    );
    let HirExprKind::Path(path) = &callee.kind
    else {
        panic!("expected bare length callee, got {callee:?}");
    };
    assert_eq!(path.parts().len(), 1, "callee must stay the bare selection name: {path}");

    let semantic = MirLowerer::lower_module_semantic(&hir);
    assert!(semantic.diagnostics.is_empty(), "声明齐全时 lowering 不得产生诊断: {:?}", semantic.diagnostics);

    // 关键断言：MIR 里**不得**产出 ArrayLength 或任何 intrinsic 专用 opcode——
    // 源码 intrinsic 自由函数必须降级为普通 Call。
    let has_intrinsic_opcode = semantic.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions).any(|instruction| {
        matches!(
            instruction.kind,
            MirOperation::ArrayLength { .. }
                | MirOperation::ArrayGet { .. }
                | MirOperation::ArraySet { .. }
                | MirOperation::NullIsNull { .. }
                | MirOperation::NullUnwrap { .. }
        )
    });
    assert!(!has_intrinsic_opcode, "源码 intrinsic 自由函数不得产出 intrinsic 专用 opcode，必须降级为普通 Call");

    // 验证降级后的 Call 里 callee 是 Callable（instance），参数包含 receiver。
    let mut calls = Vec::new();
    for function in &semantic.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let MirOperation::Call { callee, arguments } = &instruction.kind {
                    calls.push((callee.clone(), arguments.len()));
                }
            }
        }
    }
    // probe 函数里有一次 length(values) 调用；Array.Length 语言 builtin 候选也会被命中一次
    // （suffix `[ ]` 等）。至少有一个 Call 是我们关注的 length 自由函数调用。
    assert!(!calls.is_empty(), "length 调用必须降级为普通 Call");
    let callable_calls = calls.iter().filter(|(callee, _)| matches!(callee, rusty_valkyrie::mir::ssa::MirOperand::Callable(_))).count();
    assert!(callable_calls >= 1, "至少有一个 Call 是 Callable instance");
}

#[test]
fn missing_array_length_return_type_fact_degrades_to_call_not_opcode() {
    // 修后合同：intrinsic 自由函数永远降级为 Call，即使 return_type 事实缺失也不会有
    // intrinsic lowering 路径的类型检查——那是 adaptor/backend 的事，不在前端 MIR lowering 层。
    let hir = ValkyrieCompiler::default()
        .compile_source(&format!("{PRIMITIVES}{ARRAY_LENGTH_INTRINSIC_DECLARATION}{STATEMENT_PROBE}"))
        .expect("声明齐全时 HIR 必须先成立");

    let semantic = MirLowerer::lower_module_semantic(&hir);
    // 关键断言：不得产生任何 intrinsic 专用 opcode。
    let has_intrinsic_opcode = semantic.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions).any(|instruction| {
        matches!(
            instruction.kind,
            MirOperation::ArrayLength { .. }
                | MirOperation::ArrayGet { .. }
                | MirOperation::ArraySet { .. }
                | MirOperation::NullIsNull { .. }
                | MirOperation::NullUnwrap { .. }
        )
    });
    assert!(!has_intrinsic_opcode, "不得产生 intrinsic 专用 opcode，必须降级为 Call");

    // 所有 value_types 都不能有 Named("usize")——原 intrinsic lowering 禁止的错误模式在 Call 路径
    // 下也应被 overload 解析正确覆盖（返回类型是声明冻结的 Nominal，不是名字）。
    assert!(
        semantic
            .functions
            .iter()
            .all(|function| function.value_types.values().all(|ty| !matches!(ty, ValkyrieType::Named(name) if name.as_str() == "usize"))),
        "不得回落成 Named(\"usize\")"
    );
}

#[test]
fn undeclared_array_length_intrinsic_fails_without_frontend_synthesis() {
    let error =
        ValkyrieCompiler::default().compile_source_to_program(&probe_source(None)).expect_err("缺少 intrinsic 声明时数组长度调用不得成功");
    let rendered = format!("{error:?}");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "缺少声明必须结构化失败为 UnresolvedCall: {rendered}");
    assert!(rendered.contains("length"), "失败必须指出未解析的 length 调用: {rendered}");
}
