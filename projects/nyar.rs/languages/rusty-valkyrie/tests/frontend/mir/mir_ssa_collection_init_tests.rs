//! 集合字面量 Semantic MIR 展开合同：`CollectionInit` 消费 HIR 冻结两槽调用合同，
//! 展开为 `Invoke(with_length, [n])` + 逐项 `Invoke(append, [容器, 项])`。
//!
//! 验收面：callee 身份直接取 HIR 冻结 `ItemInstanceId`（不按名回查、无新 opcode）、
//! 求值保持源序单次、空字面量只发 `with_length(0)`、嵌套字面量内层先完成展开
//! （外层 `append` 实参是内层容器结果临时值）、结果过 `validate_semantic_module`。

use nyar_types::ItemInstanceId;
use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::mir::validation::validate_semantic_module,
    mir::ssa::{MirConstant, MirDiagnostic, MirFunction, MirLowerer, MirOperand, MirOperation, MirValueRef},
    types::hir::{HirExpr, HirExprKind, HirFunction, HirModule, HirResolvedCall, HirStatementKind},
};

/// 与 HIR 侧集合字面量夹具同形的完整单源合同源码。
const CONTRACT_SOURCE: &str = r#"
namespace std.collection;

trait LiteralSequence<T> {
    micro with_length(n: usize): Self
    micro append(mut self, value: T): unit
}

class ArrayList<T> {
    _items: [T]
    _capacity: usize
}

class LinkedList<T> {
    _items: [T]
}

imply ArrayList<T>: LiteralSequence<T> {
    micro with_length(n: usize): Self {
        return ArrayList { _items: [], _capacity: n }
    }

    micro append(mut self, value: T): unit {
        self._capacity = 0
    }
}

imply LinkedList<T>: LiteralSequence<T> {
    micro with_length(n: usize): Self {
        return LinkedList { _items: [] }
    }

    micro append(mut self, value: T): unit {
        self._items = []
    }
}

micro build() -> ArrayList<u32> {
    let a: ArrayList<u32> = [0, 1, 2]
    return a
}

micro build_empty() -> ArrayList<u32> {
    let e: ArrayList<u32> = []
    return e
}

micro build_nested() -> ArrayList<LinkedList<u32>> {
    let m: ArrayList<LinkedList<u32>> = [[0, 1], [2, 3]]
    return m
}
"#;

fn function<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("{name}"))
}

/// 取函数体第一条 `let` 的初始值表达式（必须是 `CollectionInit`）。
fn first_let_initializer(function: &HirFunction) -> &HirExpr {
    for statement in &function.body.statements {
        if let HirStatementKind::Let { initializer: Some(initializer), .. } = &statement.kind {
            return initializer;
        }
    }
    panic!("函数体必须含 let 绑定");
}

/// 解构 `CollectionInit`，返回两槽与项列表。
fn expect_collection_init(initializer: &HirExpr) -> (&HirResolvedCall, &HirResolvedCall, &Vec<HirExpr>) {
    match &initializer.kind {
        HirExprKind::CollectionInit { with_length, append, items, .. } => (with_length, append, items),
        other => panic!("初始值必须是 CollectionInit，实际 {other:?}"),
    }
}

/// 按程序序收集一个 MIR 函数里的全部 `Call` 指令。
fn collect_calls(mir_function: &MirFunction) -> Vec<(&MirOperand, &Vec<MirOperand>, &[MirValueRef])> {
    mir_function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::Call { callee, arguments } => Some((callee, arguments, instruction.results.as_slice())),
            _ => None,
        })
        .collect()
}

/// 回查定义某个值引用的指令。
fn defining_instruction<'a>(mir_function: &'a MirFunction, value: &MirValueRef) -> &'a rusty_valkyrie::mir::ssa::MirInstruction {
    mir_function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|instruction| instruction.results.contains(value))
        .unwrap_or_else(|| panic!("值引用 {value:?} 必须由本函数内指令定义"))
}

/// 取出一个整型常量实参的字面量值。
fn int_constant_of(mir_function: &MirFunction, operand: &MirOperand) -> i64 {
    let MirOperand::Value(value) = operand
    else {
        panic!("实参必须是已 lower 的 SSA 值，实际 {operand:?}")
    };
    match &defining_instruction(mir_function, value).kind {
        MirOperation::LoadConstant { constant: MirConstant::Int(literal), .. } => *literal,
        other => panic!("实参必须来自整型常量定义，实际 {other:?}"),
    }
}

/// `let a: ArrayList<u32> = [0,1,2]` 展开为 1 次 `with_length` + 3 次 `append`，
/// callee 身份与 HIR 冻结实例一致，实参求值保持源序。
#[test]
fn collection_init_expands_to_with_length_then_source_order_appends() {
    let hir = ValkyrieCompiler::default().compile_source(CONTRACT_SOURCE).expect("合同源码必须完成 HIR");
    let (with_length, append, items) = expect_collection_init(first_let_initializer(function(&hir, "build")));
    assert_eq!(items.len(), 3);
    let wl_instance: ItemInstanceId = with_length.instance.expect("with_length 实例必须由固定点铸造");
    let ap_instance = append.instance.expect("append 实例必须由固定点铸造");

    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(
        !mir.diagnostics.iter().any(|diagnostic| matches!(diagnostic, MirDiagnostic::UnsupportedExpression { .. })),
        "CollectionInit 不得再落 UnsupportedExpression：{:?}",
        mir.diagnostics
    );
    assert!(validate_semantic_module(&mir).is_ok(), "展开后的 Semantic MIR 必须通过合同校验");

    let build = mir.functions.iter().find(|f| f.display_symbol.ends_with("::build")).expect("build 函数");
    let calls = collect_calls(build);
    assert_eq!(calls.len(), 4, "1 次 with_length + 3 次 append");
    assert_eq!(calls[0].0, &MirOperand::Callable(wl_instance), "首调用必须是冻结的 with_length 实例");
    assert_eq!(calls[0].1.len(), 1, "with_length 只有一个长度实参");
    assert_eq!(int_constant_of(build, &calls[0].1[0]), 3, "长度实参必须是项数常量");
    assert_eq!(calls[0].2.len(), 1, "with_length 结果必须绑定 SSA 值");
    let container = MirOperand::Value(calls[0].2[0]);
    assert_eq!(build.value_types.get(&calls[0].2[0]), Some(&with_length.return_type), "容器临时值类型必须是冻结合同的返回类型");
    for (index, call) in calls[1..].iter().enumerate() {
        assert_eq!(call.0, &MirOperand::Callable(ap_instance), "第 {} 个追加调用必须是冻结的 append 实例", index + 1);
        assert_eq!(call.1.len(), 2, "append 实参是 (接收者, 元素)");
        assert_eq!(call.1[0], container, "接收者必须是同一个容器临时值");
        assert!(call.2.is_empty(), "append 返回 unit，不绑定结果");
    }
    let literals: Vec<i64> = calls[1..].iter().map(|call| int_constant_of(build, &call.1[1])).collect();
    assert_eq!(literals, vec![0, 1, 2], "元素求值必须保持源序单次");
}

/// 空字面量只发 `with_length(0)`，不产生任何 `append` 调用。
#[test]
fn empty_collection_init_expands_to_with_length_zero_only() {
    let hir = ValkyrieCompiler::default().compile_source(CONTRACT_SOURCE).expect("合同源码必须完成 HIR");
    let (with_length, _, items) = expect_collection_init(first_let_initializer(function(&hir, "build_empty")));
    assert!(items.is_empty());
    let wl_instance = with_length.instance.expect("with_length 实例必须由固定点铸造");

    let mir = MirLowerer::lower_module_semantic(&hir);
    let build = mir.functions.iter().find(|f| f.display_symbol.ends_with("::build_empty")).expect("build_empty 函数");
    let calls = collect_calls(build);
    assert_eq!(calls.len(), 1, "空字面量只发 with_length");
    assert_eq!(calls[0].0, &MirOperand::Callable(wl_instance));
    assert_eq!(calls[0].1.len(), 1);
    assert_eq!(int_constant_of(build, &calls[0].1[0]), 0, "空字面量长度必须是常量 0");
}

/// 嵌套字面量内层先完成展开：外层 `append` 的元素实参是内层 `with_length` 的结果临时值。
#[test]
fn nested_collection_init_completes_inner_expansion_before_outer_append() {
    let hir = ValkyrieCompiler::default().compile_source(CONTRACT_SOURCE).expect("合同源码必须完成 HIR");
    let (outer_wl, outer_ap, outer_items) = expect_collection_init(first_let_initializer(function(&hir, "build_nested")));
    assert_eq!(outer_items.len(), 2);
    let (inner_wl, inner_ap, inner_items) = expect_collection_init(&outer_items[0]);
    assert_eq!(inner_items.len(), 2);
    let outer_wl_instance = outer_wl.instance.expect("外层 with_length 实例");
    let outer_ap_instance = outer_ap.instance.expect("外层 append 实例");
    let inner_wl_instance = inner_wl.instance.expect("内层 with_length 实例");
    let inner_ap_instance = inner_ap.instance.expect("内层 append 实例");
    assert_ne!(outer_wl_instance, inner_wl_instance, "外层与内层容器是不同实例");

    let mir = MirLowerer::lower_module_semantic(&hir);
    let build = mir.functions.iter().find(|f| f.display_symbol.ends_with("::build_nested")).expect("build_nested 函数");
    let callees: Vec<MirOperand> = collect_calls(build).into_iter().map(|call| call.0.clone()).collect();
    let expected = vec![
        MirOperand::Callable(outer_wl_instance),
        MirOperand::Callable(inner_wl_instance),
        MirOperand::Callable(inner_ap_instance),
        MirOperand::Callable(inner_ap_instance),
        MirOperand::Callable(outer_ap_instance),
        MirOperand::Callable(inner_wl_instance),
        MirOperand::Callable(inner_ap_instance),
        MirOperand::Callable(inner_ap_instance),
        MirOperand::Callable(outer_ap_instance),
    ];
    assert_eq!(callees, expected, "展开序列必须是外层 with_length、逐个内层完整展开后外层 append");

    let calls = collect_calls(build);
    let outer_container = MirOperand::Value(calls[0].2[0]);
    for call in calls.iter().filter(|call| call.0 == &MirOperand::Callable(outer_ap_instance)) {
        assert_eq!(call.1[0], outer_container, "外层 append 接收者必须是外层容器临时值");
        let MirOperand::Value(element) = &call.1[1]
        else {
            panic!("外层 append 元素实参必须是 SSA 值")
        };
        match &defining_instruction(build, element).kind {
            MirOperation::Call { callee, .. } => {
                assert_eq!(callee, &MirOperand::Callable(inner_wl_instance), "外层 append 实参必须是内层容器构造结果临时值")
            }
            other => panic!("内层容器结果必须由 with_length 调用定义，实际 {other:?}"),
        }
    }
}
