use rusty_valkyrie::*;
use std::collections::BTreeSet;

fn plan_from_groups(groups: &[(&str, bool, Vec<(String, String)>)]) -> SemanticClosure {
    let keys = groups.iter().map(|(key, _, _)| *key).collect::<Vec<_>>();
    let inputs = groups
        .iter()
        .enumerate()
        .map(|(index, (key, is_entry, files))| ClosureGroupInput {
            dependency_key: key,
            module_name: key,
            // 夹具按依赖顺序提供组：先出现的组视为可被后出现的组直接引用。
            direct_dependencies: &keys[..index],
            is_entry: *is_entry,
            files: files.as_slice(),
        })
        .collect::<Vec<_>>();
    plan_semantic_closure(&inputs, "wasm32").expect("semantic closure planning succeeds")
}

/// 显式声明依赖可见性的夹具：用于验证「未声明的依赖组不构成可见性」。
fn plan_with_visibility(groups: &[(&str, bool, Vec<(String, String)>)], visibility: &[&[&str]]) -> SemanticClosure {
    let inputs = groups
        .iter()
        .zip(visibility)
        .map(|((key, is_entry, files), dependencies)| ClosureGroupInput {
            dependency_key: key,
            module_name: key,
            direct_dependencies: dependencies,
            is_entry: *is_entry,
            files: files.as_slice(),
        })
        .collect::<Vec<_>>();
    plan_semantic_closure(&inputs, "wasm32").expect("semantic closure planning succeeds")
}

fn file(path: &str, source: &str) -> (String, String) {
    (path.to_string(), source.to_string())
}

#[test]
fn unreachable_inventory_syntax_error_does_not_expand_into_body_paths() {
    let groups = [
        (
            "library",
            false,
            vec![file("source/good.v", "micro answer() -> i32 { return 23 }"), file("source/broken.v", "micro broken() { value = }")],
        ),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { let value: i32 = answer(); return value }")]),
    ];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/good.v".into()]));
    assert_eq!(closure.groups[1].body_lowering_source_paths, BTreeSet::from(["solution.v".into()]));
    assert!(!closure.digest.is_empty());
    assert!(closure.demanded_declarations.iter().any(|key| key.name == "answer"));
    // 根循环恢复后语法错误文件仍可索引：坏 item 被丢弃，文件不再落入 unindexed；
    // 被丢弃的声明不得进入闭包（缺失声明在下游按 fail-closed 处理）。
    assert!(closure.unindexed_source_files.is_empty(), "恢复后坏文件仍产出（空的）索引: {:?}", closure.unindexed_source_files);
    assert!(!closure.demanded_declarations.iter().any(|key| key.name == "broken"), "坏 item 的声明不得进入闭包");
}

#[test]
fn entry_group_without_entry_declaration_is_structured_failure() {
    let groups = [("app", true, vec![file("solution.v", "micro run() -> i32 { return 23 }")])];
    let inputs = [ClosureGroupInput {
        dependency_key: "app",
        module_name: "app",
        direct_dependencies: &[],
        is_entry: true,
        files: groups[0].2.as_slice(),
    }];
    let error = plan_semantic_closure(&inputs, "wasm32").expect_err("entry group without `[export]`/`[main]` must fail");
    assert_eq!(
        error,
        SemanticClosureError::MissingEntryDeclaration {
            dependency_key: "app".to_string(),
            unindexed_source_files: Vec::new(),
            unseeded_member_families: Vec::new(),
        }
    );
}

#[test]
fn entry_group_body_paths_follow_explicit_entry_declarations_only() {
    let groups = [(
        "app",
        true,
        vec![file("solution.v", "[export] micro run() -> i32 { return 23 }"), file("source/not_entry.v", "micro helper() -> i32 { return 1 }")],
    )];
    let closure = plan_from_groups(&groups);
    // 入口组不是「整组文件都是程序」：未注解声明既不进入 seed，也不下沉其文件。
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["solution.v".into()]));
    assert!(!closure.demanded_declarations.iter().any(|key| key.name == "helper"));
}

#[test]
fn main_annotation_is_an_entry_seed() {
    let groups = [("app", true, vec![file("main.v", "[main] micro start() -> i32 { return 1 }")])];
    let closure = plan_from_groups(&groups);
    assert!(closure.demanded_declarations.iter().any(|key| key.name == "start"));
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["main.v".into()]));
}

#[test]
fn witness_member_export_is_an_entry_seed() {
    // 第 1 层：`imply` witness 与声明体成员共用同一条入口注解规则。
    // `[export]` 挂在 witness 上同样是导出根，不得因外层是 `imply` 而丢失 seed。
    let groups = [(
        "app",
        true,
        vec![file(
            "source/widget.v",
            "namespace app;\nstructure Widget { }\nimply Widget {\n    [export] micro probe() -> bool { return true }\n}",
        )],
    )];
    let closure = plan_from_groups(&groups);
    assert!(
        closure.demanded_declarations.iter().any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "probe"),
        "witness `[export]` 必须进入需求闭包: {:?}",
        closure.demanded_declarations
    );
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/widget.v".into()]));
}

#[test]
fn witness_member_families_without_surface_annotation_are_named_in_the_failure() {
    // 反例：整组没有任何表面注解时仍是结构化失败，但诊断必须指名现有 witness 成员族，
    // 不得把「成员族缺表面注解」与「程序合同缺失」混成同一句无信息量的报错。
    let files = vec![file(
        "source/widget.v",
        "namespace app;\nstructure Widget { }\nimply Widget {\n    micro new() { }\n    micro probe() -> bool { return true }\n}",
    )];
    let inputs = [ClosureGroupInput {
        dependency_key: "app",
        module_name: "app",
        direct_dependencies: &[],
        is_entry: true,
        files: files.as_slice(),
    }];
    let error = plan_semantic_closure(&inputs, "wasm32").expect_err("没有表面注解的入口组必须失败");
    let expected_family = "`imply Widget { new, probe }` (source/widget.v)".to_string();
    match &error {
        SemanticClosureError::MissingEntryDeclaration { unseeded_member_families, .. } => {
            assert_eq!(unseeded_member_families, &vec![expected_family]);
        }
        other => panic!("必须是 `MissingEntryDeclaration` 而不是其它失败: {other:?}"),
    }
    assert!(error.to_string().contains("witness member families declared without a surface annotation"), "{error}");
}

#[test]
fn transitive_cross_package_symbol_pulls_base_dependency_file() {
    let groups = [
        ("base", false, vec![file("source/answer.v", "micro answer() -> i32 { return 23 }")]),
        ("library", false, vec![file("source/relay.v", "micro relay() -> i32 { return answer() }")]),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return relay() }")]),
    ];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/answer.v".into()]));
    assert_eq!(closure.groups[1].body_lowering_source_paths, BTreeSet::from(["source/relay.v".into()]));
}

#[test]
fn cross_package_reachable_symbol_pulls_dependency_file_into_closure() {
    let groups = [
        (
            "library",
            false,
            vec![file("source/answer.v", "micro answer() -> i32 { return 23 }"), file("source/unused.v", "micro unused() -> i32 { return 0 }")],
        ),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return answer() }")]),
    ];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/answer.v".into()]));
    assert!(!closure.groups[0].body_lowering_source_paths.contains("source/unused.v"));
}

#[test]
fn namespace_chain_does_not_pull_same_name_from_other_namespace() {
    let groups = [
        (
            "library",
            false,
            vec![
                file("source/a.v", "namespace a;\nmicro helper() -> i32 { return 1 }"),
                file("source/b.v", "namespace b;\nmicro helper() -> i32 { return 2 }"),
            ],
        ),
        ("app", true, vec![file("solution.v", "namespace a;\n[export] micro run() -> i32 { return helper() }")]),
    ];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/a.v".into()]));
}

#[test]
fn same_file_same_name_micros_union_declaration_references() {
    // 同一文件内同名同 arity 的顶层 `micro` 共享一个声明身份（身份不含签名）。
    // 需求展开按身份进行，因此组内每条声明的引用事实都必须并集保留；否则只被
    // 另一重载引用的类型会从闭包上界消失（同文件 `abs(i64)` 覆盖 `abs(f64)`）。
    let groups = [
        (
            "std",
            false,
            vec![file(
                "source/math/basic.v",
                "namespace std.math;\nmicro abs(value: f64): f64 { return value }\nmicro abs(value: i64): i64 { return value }",
            )],
        ),
        ("app", true, vec![file("solution.v", "namespace app;\n[export] micro probe(x: i64) -> i64 { return std::math::abs(x) }")]),
    ];
    let closure = plan_from_groups(&groups);
    let abs = closure.declaration_index.values().find(|entry| entry.key.name == "abs").expect("`abs` 必须进入声明索引");
    let references = abs.references.iter().map(|reference| reference.join("::")).collect::<BTreeSet<_>>();
    assert!(references.contains("f64"), "同文件重载组必须保留 `f64` 签名引用: {references:?}");
    assert!(references.contains("i64"), "同文件重载组必须保留 `i64` 签名引用: {references:?}");
}

#[test]
fn member_reference_resolves_within_namespace_before_global_fallback() {
    let groups = [
        (
            "library",
            false,
            vec![
                file("source/a.v", "namespace a;\nstructure S { _x: i32 }\nimply S { micro is_empty(self): bool { return true } }"),
                file("source/b.v", "namespace b;\nstructure T { _x: i32 }\nimply T { micro is_empty(self): bool { return true } }"),
            ],
        ),
        ("app", true, vec![file("solution.v", "namespace a;\n[export] micro run(s: S) -> bool { return s.is_empty() }")]),
    ];
    let closure = plan_from_groups(&groups);
    let lowered = &closure.groups[0].body_lowering_source_paths;
    assert!(lowered.contains("source/a.v"));
    assert!(!lowered.contains("source/b.v"));
}

#[test]
fn declared_member_pulls_owning_aggregate_shell() {
    let groups = [
        (
            "library",
            false,
            vec![file("source/hash.v", "namespace a;\nstructure S { _x: i32 }\nimply S { micro hash(self): usize { return 0 } }")],
        ),
        ("app", true, vec![file("solution.v", "namespace a;\n[export] micro run(s: S) -> usize { return s.hash() }")]),
    ];
    let closure = plan_from_groups(&groups);
    let demanded = &closure.groups[0].body_lowering_declarations;
    assert!(demanded.iter().any(|key| key.kind == DeclarationKind::Structure && key.name == "S"));
    assert!(demanded.iter().any(|key| key.name == "hash"));
}

#[test]
fn undemanded_impl_witness_keeps_its_shell_out_of_closure() {
    let groups = [
        (
            "library",
            false,
            vec![
                file("source/iterator/_.v", "namespace std.iterator;\ntrait Iterator { }"),
                file(
                    "source/impl.v",
                    "namespace std.collection;\nstructure S { _x: i32 }\nimply S: std::iterator::Iterator { micro not_used(self): bool { return true } }",
                ),
            ],
        ),
        ("app", true, vec![file("solution.v", "namespace std.collection;\n[export] micro run(s: S) -> bool { return true }")]),
    ];
    let closure = plan_from_groups(&groups);
    let lowered = &closure.groups[0].body_lowering_source_paths;
    assert!(lowered.contains("source/impl.v"));
    // 未被需求的 witness 方法连带其 impl 外壳一起不进入闭包：外壳的 trait 路径因此不必解析，
    // 闭包不得以「外壳始终物化」为名把整个文件的无差别引用拉进来。
    assert!(!lowered.contains("source/iterator/_.v"));
    assert!(!closure.groups[0].body_lowering_declarations.iter().any(|key| key.name == "not_used"));
}

#[test]
fn qualified_constructor_resolves_through_dependency_visibility() {
    let groups = [
        (
            "std",
            false,
            vec![file(
                "source/collection/HashMap.v",
                "namespace std.collection;\nclass HashMap<K, V> { _impl: i32 }\nimply HashMap<K, V> { micro new(capacity: usize): Self { return Self { _impl: 0 } } }",
            )],
        ),
        (
            "app",
            true,
            vec![file(
                "solution.v",
                "namespace leetcode.two_sum;\n[export] micro two_sum(size: usize) -> HashMap<i32, i32> { return HashMap::new(0) }",
            )],
        ),
    ];
    let closure = plan_from_groups(&groups);
    let demanded = &closure.groups[0].body_lowering_declarations;
    // 跨 namespace 的类型引用与限定构造必须通过依赖可见性命中，而不是靠名字认亲。
    assert!(demanded.iter().any(|key| key.kind == DeclarationKind::Class && key.name == "HashMap"));
    assert!(demanded.iter().any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "new"));
}

#[test]
fn member_selector_promotes_used_member_and_skips_unused() {
    let groups = [
        (
            "library",
            false,
            vec![file(
                "source/s.v",
                "namespace a;\nstructure S { _x: i32 }\nimply S { micro used(self): bool { return true }\nmicro unused(self): bool { return true } }",
            )],
        ),
        ("app", true, vec![file("solution.v", "namespace a;\n[export] micro run(s: S) -> bool { return s.used() }")]),
    ];
    let closure = plan_from_groups(&groups);
    let demanded = &closure.groups[0].body_lowering_declarations;
    // 接收者类型在声明索引阶段不可知：成员只按「该名字出现在闭包需求中 + 拥有聚合已需求」提升。
    assert!(demanded.iter().any(|key| key.name == "used"));
    assert!(!demanded.iter().any(|key| key.name == "unused"));
}

#[test]
fn reference_into_undeclared_dependency_group_is_not_demanded() {
    let groups = [
        ("library", false, vec![file("source/answer.v", "micro answer() -> i32 { return 23 }")]),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return answer() }")]),
    ];
    let visibility: &[&[&str]] = &[&[], &[]];
    let closure = plan_with_visibility(&groups, visibility);
    // 未声明直接依赖即不构成可见性：名字命中也不得跨组把声明拉进闭包。
    assert!(closure.groups[0].body_lowering_source_paths.is_empty());
    assert!(!closure.demanded_declarations.iter().any(|key| key.name == "answer"));
}

#[test]
fn closure_digest_is_stable_for_identical_input() {
    let groups = [("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return 23 }")])];
    let first = plan_from_groups(&groups);
    let second = plan_from_groups(&groups);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.declaration_index.len(), 1);
}

#[test]
fn closure_digest_binds_demanded_source_content() {
    let baseline = [("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return 23 }")])];
    let changed = [("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return 24 }")])];
    assert_ne!(plan_from_groups(&baseline).digest, plan_from_groups(&changed).digest);
}

#[test]
fn closure_digest_ignores_unreachable_dependency_body_change() {
    let groups = |body: &str| {
        [
            ("library", false, vec![file("source/answer.v", "micro answer() -> i32 { return 23 }"), file("source/unused.v", body)]),
            ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return answer() }")]),
        ]
    };
    let first = plan_from_groups(&groups("micro unused() -> i32 { return 1 }"));
    let second = plan_from_groups(&groups("micro unused() -> i32 { return 2 }"));
    assert_eq!(first.digest, second.digest);
}

#[test]
fn operator_use_site_promotes_receiver_operator_member() {
    let groups = [
        (
            "core",
            false,
            vec![file(
                "source/primitive/usize.v",
                "namespace std::primitive;\nstructure usize { }\nimply usize {\n    infix `+`(self, rhs: Self): Self { return rhs }\n    infix `<`(self, rhs: Self): bool { return true }\n    infix `==`(self, rhs: Self): bool { return true }\n}",
            )],
        ),
        (
            "std",
            false,
            vec![file(
                "source/hash.v",
                "namespace std.collection;\nmicro hash_utf8(n: usize) -> usize {\n    let mut i: usize = 0\n    while i < n {\n        i = i + 1\n    }\n    return i\n}",
            )],
        ),
        ("app", true, vec![file("solution.v", "namespace app;\n[export] micro run() -> usize { return hash_utf8(3) }")]),
    ];
    let closure = plan_from_groups(&groups);
    let demanded = &closure.groups[0].body_lowering_declarations;
    // 运算符是普通成员调用：`i < n` 与 `i + 1` 都是 `usize` 的成员选择名，
    // 因此接收者类型 `usize` 上的对应运算符成员必须进入闭包。
    assert!(demanded.iter().any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "infix <"));
    assert!(demanded.iter().any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "infix +"));
    // 没有被选择名命中的运算符成员不得连带进入闭包。
    assert!(!demanded.iter().any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "infix =="));
}

#[test]
fn bare_member_selector_never_resolves_to_any_member_declaration() {
    let groups = [
        ("library", false, vec![file("source/s.v", "namespace a;\nstructure S { _x: i32 }\nimply S { micro hash(self): usize { return 0 } }")]),
        ("app", true, vec![file("solution.v", "namespace a;\n[export] micro run() -> usize { return hash() }")]),
    ];
    let closure = plan_from_groups(&groups);
    // 成员选择名不建立全库同名索引：没有接收者类型事实时，裸名 `hash` 不得命中任何成员声明，
    // 因此既不能把成员拉进闭包，也不能把只含该成员的文件下沉到 body lowering。
    assert!(!closure.groups[0].body_lowering_declarations.iter().any(|key| key.name == "hash"));
    assert!(closure.groups[0].body_lowering_source_paths.is_empty());
}

#[test]
fn member_selector_promotes_only_demanded_aggregate_owner() {
    let groups = [
        (
            "library",
            false,
            vec![
                file(
                    "source/swiss.v",
                    "namespace a;\nstructure SwissTable { _x: i32 }\nimply SwissTable { micro hash(self): usize { return 0 } }",
                ),
                file("source/bitset.v", "namespace a;\nstructure BitSet { _x: i32 }\nimply BitSet { micro hash(self): usize { return 0 } }"),
            ],
        ),
        ("app", true, vec![file("solution.v", "namespace a;\n[export] micro run(s: SwissTable) -> usize { return s.hash() }")]),
    ];
    let closure = plan_from_groups(&groups);
    let demanded = &closure.groups[0].body_lowering_declarations;
    // 同 namespace 内的同名成员不得互相污染：只有被需求的拥有聚合能凭选择名提升自己的成员。
    assert!(demanded.iter().any(|key| key.name == "hash" && key.owner.as_deref() == Some("SwissTable")));
    assert!(!demanded.iter().any(|key| key.owner.as_deref() == Some("BitSet")));
    assert!(!closure.groups[0].body_lowering_source_paths.contains("source/bitset.v"));
}

#[test]
fn member_owner_identity_is_resolved_through_type_alias() {
    let groups = [
        (
            "std",
            false,
            vec![
                file("source/text/_.v", "namespace std.text;\ntype utf8 = Utf8Text;"),
                file(
                    "source/text/Utf8Text.v",
                    "namespace std.text;\nclass Utf8Text { _repr: [u8] }\nimply Utf8Text { micro length(self) -> i32 { return 0 } }",
                ),
                file("source/hash/HashPrimitives.v", "namespace std.collection;\nimply utf8: Hash { micro hash(self): usize { return 0 } }"),
            ],
        ),
        ("app", true, vec![file("solution.v", "namespace app;\n[export] micro run(s: utf8) -> usize { return s.hash() }")]),
    ];
    let closure = plan_from_groups(&groups);
    let demanded = &closure.groups[0].body_lowering_declarations;
    // 别名不是独立名义聚合：`utf8` 的引用展开到目标聚合 `Utf8Text`，`imply utf8: Hash`
    // 的 witness 因此挂在该聚合身份上，而不是挂在别名身份或短名 `utf8` 上。
    assert!(demanded.iter().any(|key| key.kind == DeclarationKind::TypeAlias && key.name == "utf8"));
    assert!(demanded.iter().any(|key| key.kind == DeclarationKind::Class && key.name == "Utf8Text"));
    assert!(demanded.iter().any(|key| key.name == "hash" && key.owner.as_deref() == Some("utf8")));
    assert!(closure.groups[0].body_lowering_source_paths.contains("source/hash/HashPrimitives.v"));
}

#[test]
fn split_type_alias_entry_pulls_nominal_definition_file() {
    let groups = [
        (
            "stdlib",
            false,
            vec![
                file("text/Utf8Text.v", "namespace std.text;\nclass Utf8Text { _repr: [u8] }"),
                file("text/_.v", "namespace! std.text;\ntype utf8 = Utf8Text;"),
            ],
        ),
        ("app", true, vec![file("solution.v", "namespace std.text;\n[export] micro run(value: utf8) -> i32 { return 0 }")]),
    ];
    let closure = plan_with_visibility(&groups, &[&[], &["stdlib"]]);
    let std_paths = &closure.groups[0].body_lowering_source_paths;
    assert!(std_paths.contains("text/_.v"), "alias entry file must lower");
    assert!(std_paths.contains("text/Utf8Text.v"), "alias target nominal must enter body lowering");
}

/// 名义声明 + 数组字面量的 `let` 触发 `LiteralSequence` 合同成员提升：
/// `with_length`/`append` witness 体进入需求闭包，供 HIR 构造点按声明身份匹配消费。
#[test]
fn collection_literal_let_demands_literal_sequence_contract_members() {
    let library_source = r#"
namespace std.collection;

trait LiteralSequence<T> {
    micro with_length(n: usize): Self
    micro append(mut self, value: T): unit
}

class ArrayList<T> {
    _items: [T]
    _capacity: usize
}

imply ArrayList<T>: LiteralSequence<T> {
    micro with_length(n: usize): Self {
        return ArrayList { _items: [], _capacity: n }
    }

    micro append(mut self, value: T): unit {
        self._capacity = 0
    }
}
"#;
    let groups = [
        ("library", false, vec![file("source/collection.v", library_source)]),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 {\n    let a: ArrayList<u32> = [0, 1, 2]\n    return 0\n}")]),
    ];
    let closure = plan_from_groups(&groups);
    let demanded = &closure.demanded_declarations;
    assert!(
        demanded
            .iter()
            .any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "with_length" && key.owner.as_deref() == Some("ArrayList")),
        "with_length witness 必须进入需求闭包"
    );
    assert!(
        demanded
            .iter()
            .any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "append" && key.owner.as_deref() == Some("ArrayList")),
        "append witness 必须进入需求闭包"
    );
    assert!(closure.groups[0].body_lowering_source_paths.contains("source/collection.v"), "合同 impl 所在文件必须下沉 body");
}

/// 返回规划结果的夹具：自举身份失败必须在闭包边界结构化可见，不能 expect 成功。
fn try_plan_with_visibility(
    groups: &[(&str, bool, Vec<(String, String)>)],
    visibility: &[&[&str]],
) -> Result<SemanticClosure, SemanticClosureError> {
    let inputs = groups
        .iter()
        .zip(visibility)
        .map(|((key, is_entry, files), dependencies)| ClosureGroupInput {
            dependency_key: key,
            module_name: key,
            direct_dependencies: dependencies,
            is_entry: *is_entry,
            files: files.as_slice(),
        })
        .collect::<Vec<_>>();
    plan_semantic_closure(&inputs, "wasm32")
}

#[test]
fn primitive_annotations_register_bootstrap_identities_before_visibility() {
    // 自举的正向证明：app 未声明对 core 的可见性、也没有引用 `i32` 声明，
    // 声明不进闭包，但 `[primitive("…")]` 身份仍必须在可见性展开之前登记成表。
    let groups = [
        (
            "core",
            false,
            vec![file("source/primitive/i32.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32 { }")],
        ),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return 0 }")]),
    ];
    let closure = plan_with_visibility(&groups, &[&[], &[]]);
    assert_eq!(closure.primitive_identities.len(), 1);
    let identity = closure.primitive_identities.get("std::primitive::i32").expect("bootstrap identity registered before visibility");
    assert_eq!(identity.declaration.dependency_key, "core");
    assert_eq!(identity.declaration.name, "i32");
    // 反查同样按声明身份成立：下游名字表迁移后必须以这张表为唯一事实来源。
    assert_eq!(closure.primitive_identities.identity_of(&identity.declaration), Some("std::primitive::i32"));
    // 未被可见性命中的声明确实不在需求闭包内——身份表与闭包需求是两条独立通道。
    assert!(!closure.demanded_declarations.iter().any(|key| key.name == "i32"));
}

#[test]
fn cross_surface_primitive_identity_resolves_canonical_by_qualified_reference() {
    // 合法同源投影的正向合同：封闭生成单元（`asgard.generated`）为宿主调用重声明自己用到的
    // 标量拼写，它必须把规范包声明为直接依赖，身份引用才能在其可见面内解析到 std 的规范声明。
    // 来源恰为规范声明的载体登记为别名；std 声明在自身可见面内自命中，是规范 owner。
    // 裁决不看组名，也不依赖登记顺序。
    let std_files = || {
        vec![
            file("source/primitive/i32.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32 { }"),
            file("source/primitive/bool.v", "namespace std.primitive;\n[primitive(\"std::primitive::bool\")]\nstructure bool { }"),
        ]
    };
    let generated_files = || {
        vec![file(
            "asgard/generated.v",
            "[primitive(\"std::primitive::i32\")]\nstructure i32 { }\n\n[primitive(\"std::primitive::bool\")]\nstructure bool { }\n\n[export] micro run() -> i32 { return 0 }",
        )]
    };
    let cases = [
        (
            [("std".to_string(), false, std_files()), ("asgard.generated".to_string(), true, generated_files())],
            [&[][..], &["std"][..]],
        ),
        // 反序输入：结果必须逐条一致，证明 owner 不来自先到先得。
        (
            [("asgard.generated".to_string(), true, generated_files()), ("std".to_string(), false, std_files())],
            [&["std"][..], &[][..]],
        ),
    ];
    for (groups, visibility) in cases {
        let typed = groups.iter().map(|(key, is_entry, files)| (key.as_str(), *is_entry, files.clone())).collect::<Vec<_>>();
        let closure = try_plan_with_visibility(&typed, &visibility).expect("生成组声明对 std 的可见性后，同源投影必须登记成表");
        assert_eq!(closure.primitive_identities.len(), 2, "每个身份只保留规范 owner 一条记录");
        for identity in ["std::primitive::i32", "std::primitive::bool"] {
            let entry = closure.primitive_identities.get(identity).unwrap_or_else(|| panic!("`{identity}` 必须登记"));
            assert_eq!(entry.declaration.dependency_key, "std", "`{identity}` 的规范 owner 必须是自命中规范位置的声明: {:?}", entry.declaration);
            assert_eq!(closure.primitive_identities.identity_of(&entry.declaration), Some(identity));
            assert_eq!(entry.aliases.len(), 1, "`{identity}` 必须登记唯一的生成侧别名载体: {:?}", entry.aliases);
            assert_eq!(entry.aliases[0].dependency_key, "asgard.generated");
        }
    }
}

#[test]
fn identity_spelling_through_transparent_type_alias_owns_the_carrier() {
    // 拼写与声明分离的正向合同：std 用 `type utf8 = Utf8Text` 给载体一个透明拼写，
    // 注解写在载体声明上而身份字符串命名别名。别名不是第二个声明，穿透 Resolver 记录的
    // 别名目标后来源恰为携带者自身，必须登记为规范 owner，而不是按字符串认领或拒绝。
    let groups = [
        (
            "core",
            false,
            vec![file(
                "source/text/std.text.v",
                "namespace std.text;\n\n[primitive(\"std::text::utf8\")]\nclass Utf8Text {\n    _repr: [u8]\n}\n\ntype utf8 = Utf8Text;",
            )],
        ),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return 0 }")]),
    ];
    let closure = try_plan_with_visibility(&groups, &[&[], &[]]).expect("别名拼写必须解析到携带者声明");
    let entry = closure.primitive_identities.get("std::text::utf8").expect("`std::text::utf8` 必须登记");
    assert_eq!(entry.declaration.name, "Utf8Text", "规范 owner 是载体声明，不是别名声明: {:?}", entry.declaration);
    assert!(entry.aliases.is_empty(), "别名拼写不产生第二个携带者: {:?}", entry.aliases);
    assert_eq!(closure.primitive_identities.identity_of(&entry.declaration), Some("std::text::utf8"));
}

#[test]
fn cross_surface_carrier_without_visible_canonical_source_is_rejected() {
    // 独立声明冒领身份的反向合同：std 在输入里持有规范声明，但生成组没有声明对 std 的
    // 可见性——它的身份引用在自己的可见面内解析落空，字符串相同不能证明同源，必须拒绝。
    let groups = [
        ("std", false, vec![file("source/primitive/i32.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32 { }")]),
        (
            "asgard.generated",
            true,
            vec![file("asgard/generated.v", "[primitive(\"std::primitive::i32\")]\nstructure i32 { }\n\n[export] micro run() -> i32 { return 0 }")],
        ),
    ];
    let error = try_plan_with_visibility(&groups, &[&[], &[]]).expect_err("不可见规范声明的重声明必须失败");
    match error {
        SemanticClosureError::UnprovenPrimitiveIdentitySource { identity, declaration, resolved_source } => {
            assert_eq!(identity, "std::primitive::i32");
            assert!(declaration.contains("asgard.generated"), "失败必须指名无法证明来源的携带者: {declaration}");
            assert_eq!(resolved_source, None, "可见面内解析落空: {resolved_source:?}");
        }
        other => panic!("expected UnprovenPrimitiveIdentitySource, got {other}"),
    }
}

#[test]
fn independent_owners_claiming_canonical_qualified_name_are_rejected() {
    // 两个独立 owner 在各自依赖组里都持有恰好占据身份规范位置的声明：并列自命中冒领同一
    // 规范 owner，结构化拒绝，不得按组名或遍历顺序择一。
    let groups = [
        ("std.one", false, vec![file("source/i32.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32 { }")]),
        ("std.two", false, vec![file("source/i32.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32 { }")]),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return 0 }")]),
    ];
    let error = try_plan_with_visibility(&groups, &[&[], &[], &[]]).expect_err("并列全称命中必须失败");
    match error {
        SemanticClosureError::DuplicatePrimitiveIdentity { identity, declarations } => {
            assert_eq!(identity, "std::primitive::i32");
            assert_eq!(declarations.len(), 2, "两个冒领者都必须出现在诊断里: {declarations:?}");
        }
        other => panic!("expected DuplicatePrimitiveIdentity, got {other}"),
    }
}

#[test]
fn cross_surface_claims_without_canonical_reference_are_rejected() {
    // 多个携带者而无人占据身份规范位置：每个携带者的引用解析都落空，没有任何声明能
    // 证明自己是规范 owner，第一个携带者（声明身份序）即被结构化拒绝。
    let carrier = |key: &'static str, path: &str| (key, false, vec![file(path, "[primitive(\"std::primitive::i32\")]\nstructure i32 { }")]);
    let groups = [
        carrier("island.a", "a.v"),
        carrier("island.b", "b.v"),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return 0 }")]),
    ];
    let error = try_plan_with_visibility(&groups, &[&[], &[], &[]]).expect_err("无人占据规范位置的多携带者必须失败");
    match error {
        SemanticClosureError::UnprovenPrimitiveIdentitySource { identity, declaration, resolved_source } => {
            assert_eq!(identity, "std::primitive::i32");
            assert!(declaration.contains("island.a"), "失败必须指名首个无法证明来源的携带者: {declaration}");
            assert_eq!(resolved_source, None, "可见面内解析落空: {resolved_source:?}");
        }
        other => panic!("expected UnprovenPrimitiveIdentitySource, got {other}"),
    }
}

#[test]
fn single_carrier_without_canonical_reference_is_rejected() {
    // 唯一携带者没有豁免：规范包未进入输入、声明全称又不占据身份位置时，字符串相同
    // 无法证明同源声明，必须显式拒绝（fail-closed），不得静默授予 owner。
    let groups = [(
        "generated.only",
        true,
        vec![file("generated.v", "[primitive(\"std::primitive::i32\")]\nstructure i32 { }\n\n[export] micro run() -> i32 { return 0 }")],
    )];
    let error = try_plan_with_visibility(&groups, &[&[]]).expect_err("唯一携带者的全称冒领必须失败");
    match error {
        SemanticClosureError::UnprovenPrimitiveIdentitySource { identity, declaration, resolved_source } => {
            assert_eq!(identity, "std::primitive::i32");
            assert!(declaration.contains("generated.only"), "失败必须指名携带者: {declaration}");
            assert_eq!(resolved_source, None, "可见面内解析落空: {resolved_source:?}");
        }
        other => panic!("expected UnprovenPrimitiveIdentitySource, got {other}"),
    }
}

#[test]
fn standalone_carrier_at_canonical_location_owns_identity_for_bootstrap() {
    // 合法 bootstrap 的正向合同：封闭单元单独构建时，只要它自己占据身份的规范位置
    // （namespace 段 + 短名按声明索引解析自命中），来源即可验证，登记为规范 owner。
    let groups = [(
        "generated.only",
        true,
        vec![file(
            "generated.v",
            "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32 { }\n\n[export] micro run() -> i32 { return 0 }",
        )],
    )];
    let closure = plan_with_visibility(&groups, &[&[]]);
    let entry = closure.primitive_identities.get("std::primitive::i32").expect("自命中规范位置的携带者必须登记");
    assert_eq!(entry.declaration.dependency_key, "generated.only");
    assert!(entry.aliases.is_empty(), "无竞争者时不得登记别名: {:?}", entry.aliases);
}

#[test]
fn carrier_projecting_to_non_claiming_declaration_is_rejected() {
    // 携带者的身份引用解析到规范位置上不携带 `[primitive]` 注解的声明：来源不是任何
    // 身份声明的投影，无法证明同源，结构化拒绝，并把命中的来源声明写进诊断。
    let groups = [
        ("std", false, vec![file("source/i32.v", "namespace std.primitive;\nstructure i32 { }")]),
        (
            "carrier",
            true,
            vec![file("carrier.v", "[primitive(\"std::primitive::i32\")]\nstructure i32 { }\n\n[export] micro run() -> i32 { return 0 }")],
        ),
    ];
    let error = try_plan_with_visibility(&groups, &[&[], &["std"]]).expect_err("来源不携带身份的投影必须失败");
    match error {
        SemanticClosureError::UnprovenPrimitiveIdentitySource { identity, declaration, resolved_source } => {
            assert_eq!(identity, "std::primitive::i32");
            assert!(declaration.contains("carrier"), "失败必须指名携带者: {declaration}");
            let source = resolved_source.expect("引用命中了不携带身份的声明");
            assert!(source.contains("std"), "诊断必须给出命中的来源声明: {source}");
        }
        other => panic!("expected UnprovenPrimitiveIdentitySource, got {other}"),
    }
}

#[test]
fn ambiguous_canonical_location_is_rejected() {
    // 携带者可见面内同一绝对限定位置命中多个声明：来源歧义，无法证明是哪一条声明的
    // 投影，结构化拒绝并列出全部候选来源。
    let canonical = |key: &'static str| (key, false, vec![file("source/i32.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32 { }")]);
    let groups = [
        canonical("std.one"),
        canonical("std.two"),
        (
            "app",
            true,
            vec![file("app.v", "[primitive(\"std::primitive::i32\")]\nstructure i32 { }\n\n[export] micro run() -> i32 { return 0 }")],
        ),
    ];
    let error = try_plan_with_visibility(&groups, &[&[], &[], &["std.one", "std.two"]]).expect_err("来源位置歧义必须失败");
    match error {
        SemanticClosureError::AmbiguousPrimitiveIdentitySource { identity, declaration, sources } => {
            assert_eq!(identity, "std::primitive::i32");
            assert!(declaration.contains("app"), "失败必须指名携带者: {declaration}");
            assert_eq!(sources.len(), 2, "两个候选来源都必须出现在诊断里: {sources:?}");
        }
        other => panic!("expected AmbiguousPrimitiveIdentitySource, got {other}"),
    }
}

#[test]
fn alias_rename_does_not_move_canonical_owner() {
    // 别名载体改名（短名不再是身份末段）不改变规范 owner；载体身份引用在可见面内解析
    // 命中 std 的规范声明，仍按可验证来源登记为别名。
    let groups = [
        ("std", false, vec![file("source/i32.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32 { }")]),
        (
            "carrier",
            true,
            vec![file(
                "carrier.v",
                "[primitive(\"std::primitive::i32\")]\nstructure i32_carrier { }\n\n[export] micro run() -> i32 { return 0 }",
            )],
        ),
    ];
    let closure = plan_with_visibility(&groups, &[&[], &["std"]]);
    let entry = closure.primitive_identities.get("std::primitive::i32").expect("身份必须登记");
    assert_eq!(entry.declaration.dependency_key, "std", "规范 owner 不随载体改名移动: {:?}", entry.declaration);
    assert_eq!(entry.aliases.len(), 1);
    assert_eq!(entry.aliases[0].name, "i32_carrier");
}

#[test]
fn canonical_rename_breaks_the_reference_and_is_rejected() {
    // 规范声明改名（不再占据身份规范位置）后，载体与它的引用解析都落空：身份失去可
    // 验证来源，结构化拒绝，不得静默把载体升格。
    let groups = [
        ("std", false, vec![file("source/i32.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32_renamed { }")]),
        (
            "carrier",
            true,
            vec![file("carrier.v", "[primitive(\"std::primitive::i32\")]\nstructure i32 { }\n\n[export] micro run() -> i32 { return 0 }")],
        ),
    ];
    let error = try_plan_with_visibility(&groups, &[&[], &["std"]]).expect_err("身份引用失去规范 owner 必须失败");
    match error {
        SemanticClosureError::UnprovenPrimitiveIdentitySource { identity, declaration, resolved_source } => {
            assert_eq!(identity, "std::primitive::i32");
            assert!(declaration.contains("carrier"), "失败必须指名首个携带者: {declaration}");
            assert_eq!(resolved_source, None, "改名后规范位置为空: {resolved_source:?}");
        }
        other => panic!("expected UnprovenPrimitiveIdentitySource, got {other}"),
    }
}

#[test]
fn duplicate_primitive_identity_is_structured_failure() {
    // 同一 surface（同一依赖组）内两个声明携带同一自举身份：不得择一登记。
    let groups = [
        (
            "core",
            false,
            vec![
                file("source/a.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure i32 { }"),
                file("source/b.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")]\nstructure shadow { }"),
            ],
        ),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return 0 }")]),
    ];
    let error = try_plan_with_visibility(&groups, &[&[], &["core"]]).expect_err("duplicate primitive identity must fail");
    match error {
        SemanticClosureError::DuplicatePrimitiveIdentity { identity, declarations } => {
            assert_eq!(identity, "std::primitive::i32");
            assert_eq!(declarations.len(), 2);
        }
        other => panic!("expected DuplicatePrimitiveIdentity, got {other}"),
    }
}

#[test]
fn malformed_primitive_identity_is_structured_failure() {
    // 身份字符串必须至少是 `::` 限定名：裸短名没有可校验的自举形态。
    let groups = [
        ("core", false, vec![file("source/a.v", "namespace std.primitive;\n[primitive(\"i32\")]\nstructure i32 { }")]),
        ("app", true, vec![file("solution.v", "[export] micro run() -> i32 { return 0 }")]),
    ];
    let error = try_plan_with_visibility(&groups, &[&[], &["core"]]).expect_err("malformed primitive identity must fail");
    match error {
        SemanticClosureError::MalformedPrimitiveIdentity { declaration, identity } => {
            assert_eq!(identity, "i32");
            assert!(declaration.contains("::structure::"), "失败必须指名携带注解的声明: {declaration}");
        }
        other => panic!("expected MalformedPrimitiveIdentity, got {other}"),
    }
}

#[test]
fn member_owner_link_is_gated_by_owner_group_visibility() {
    let groups = [
        ("core", false, vec![file("source/primitive/i64.v", "namespace std.primitive;\nstructure i64 { }")]),
        ("std", false, vec![file("source/hash.v", "namespace std.collection;\nimply i64: Hash { micro hash(self): usize { return 0 } }")]),
        ("app", true, vec![file("solution.v", "namespace app;\n[export] micro run(x: i64) -> usize { return x.hash() }")]),
    ];
    // 成员与拥有者跨依赖组：std 的 `imply i64: Hash` 与 core 的 `structure i64` 按声明身份连接。
    let visible = plan_with_visibility(&groups, &[&[], &["core"], &["core", "std"]]);
    assert!(visible.demanded_declarations.iter().any(|key| key.dependency_key == "std" && key.name == "hash"));
    // std 未声明 core 为直接依赖时，拥有者引用不得命中 core 的声明，成员因而不成立也不得提升。
    let hidden = plan_with_visibility(&groups, &[&[], &[], &["core", "std"]]);
    assert!(!hidden.demanded_declarations.iter().any(|key| key.name == "hash"));
    assert!(hidden.demanded_declarations.iter().any(|key| key.kind == DeclarationKind::Structure && key.name == "i64"));
}

/// 生成端文本投影探针：`expr.rs` 把文本位置从 `utf8(<expr>)` 改成 `<expr>.display()` 后，
/// 生成单元必须在源码里显式 `using std.text;`（`v_bundle::GENERATED_STD_TEXT_IMPORT`），
/// 该 import + 注释里的名字必须解析到 `std.text` 的具体 `imply` 成员，而不是靠隐式传播：
/// `display` 命中 `imply i32: Display` / `imply Utf8Text: Display`，`item.key == key`
/// （名义 receiver + 内建参数）命中 `Utf8Text` 的 `infix ==`。
#[test]
fn generated_display_projection_requires_explicit_std_text_import() {
    let std_files = || {
        vec![
            file("source/primitive/i32.v", "namespace std.primitive;\n[primitive(\"std::primitive::i32\")] structure i32 { }"),
            file("source/text/Utf8Text.v", "namespace std.text;\nclass Utf8Text { _repr: [u8] }"),
            file("source/text/_.v", "namespace std.text;\ntype utf8 = Utf8Text;"),
            file(
                "source/text/format.v",
                "namespace std.text;\ntrait Display { micro display(self) -> utf8 }\n\
                 imply i32: Display { micro display(self) -> utf8 { return \"0\" } }\n\
                 imply Utf8Text: Display { micro display(self) -> utf8 { return self } }",
            ),
            file(
                "source/text/Utf8TextEqual.v",
                r#"namespace std.text;
class Item { key: utf8 }
imply Utf8Text { infix `==`(self, other: utf8): bool { return true } }"#,
            ),
        ]
    };
    let app_source = |import: &str| {
        file(
            "asgard/generated.v",
            &format!(
                "{import}[export] micro probe(value: i32) -> utf8 {{ return (value).display() }}\n\
                 [export] micro probe_text(text: utf8) -> utf8 {{ return (text).display() }}\n\
                 [export] micro same(item: Item, key: utf8) -> bool {{ return item.key == key }}"
            ),
        )
    };
    let closure = plan_from_groups(&[
        ("std", false, std_files()),
        ("asgard.generated", true, vec![app_source("using std.text;\n")]),
    ]);
    let demanded = &closure.demanded_declarations;
    assert!(
        demanded
            .iter()
            .any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "display" && key.owner.as_deref() == Some("i32")),
        "`display` 必须解析到 `std.text` 的具体 `imply i32: Display` 成员: {demanded:?}"
    );
    assert!(
        demanded
            .iter()
            .any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "display" && key.owner.as_deref() == Some("Utf8Text")),
        "文本 receiver 的 `display` 必须解析到 `imply Utf8Text: Display` 成员: {demanded:?}"
    );
    assert!(
        demanded.iter().any(|key| key.kind == DeclarationKind::ImplyMethod
            && key.name == "infix =="
            && key.owner.as_deref() == Some("Utf8Text")),
        "`item.key == key` 必须解析到 `Utf8Text` 的 `infix ==` 成员: {demanded:?}"
    );
}

/// 插值捕获在调用解析前脱糖为 `<expr>.display()` 与 `infix ++` 连接，这些选择名不出现在
/// 源码里。闭包必须按同一合同登记成员需求，否则依赖组的 formatter/连接声明被裁掉，
/// 脱糖后的调用点在 HIR 无候选可解析。
#[test]
fn interpolated_capture_demands_formatter_and_concat_members() {
    let std_files = || {
        vec![
            file("source/primitive/i64.v", "namespace std.primitive;\n[primitive(\"std::primitive::i64\")] structure i64 { }"),
            file("source/text/_.v", "namespace std.text;\ntype utf8 = Utf8Text;"),
            file(
                "source/text/Utf8Text.v",
                r#"namespace std.text;
class Utf8Text { _repr: [u8] }
imply Utf8Text { infix `++`(self, other: utf8): utf8 { return self } }"#,
            ),
            file("source/text/Template.v", "namespace std.text;\ntrait Display { micro display(self): utf8 }"),
            file(
                "source/text/format.v",
                "namespace std.text;\nimply i64: Display { micro display(self): utf8 { return \"0\" } }\n\
                 imply Utf8Text: Display { micro display(self): utf8 { return self } }",
            ),
        ]
    };
    let app = file(
        "solution.v",
        r#"using std.text;
namespace app;
micro bump(counter: i64) -> i64 { return counter }
[export] micro probe(name: utf8, suffix: utf8) -> utf8 { return "{ name }{ bump(0) }{ suffix }" }"#,
    );
    let closure = plan_from_groups(&[("std", false, std_files()), ("app", true, vec![app])]);
    let demanded = &closure.demanded_declarations;
    assert!(
        demanded
            .iter()
            .any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "display" && key.owner.as_deref() == Some("Utf8Text")),
        "文本插值段必须提升 `imply Utf8Text: Display` 成员: {demanded:?}"
    );
    assert!(
        demanded
            .iter()
            .any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "display" && key.owner.as_deref() == Some("i64")),
        "调用结果插值段必须提升 `imply i64: Display` 成员: {demanded:?}"
    );
    assert!(
        demanded
            .iter()
            .any(|key| key.kind == DeclarationKind::ImplyMethod && key.name == "infix ++" && key.owner.as_deref() == Some("Utf8Text")),
        "多段插值必须提升 `Utf8Text` 的 `infix ++` 成员: {demanded:?}"
    );
    assert!(
        demanded.iter().any(|key| key.kind == DeclarationKind::Micro && key.name == "bump"),
        "插值表达式里的普通调用必须进入需求: {demanded:?}"
    );
    assert!(
        closure.groups[0].body_lowering_source_paths.contains("source/text/format.v"),
        "formatter 成员所在源文件必须进入 body lowering: {:?}",
        closure.groups[0].body_lowering_source_paths
    );
}

#[test]
fn two_test_annotations_become_two_test_entries() {
    let groups = [(
        "app",
        true,
        vec![file("suite.v", "[test] micro alpha() -> i32 { return 1 }\n[test] micro beta() -> i32 { return 2 }")],
    )];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.test_entries, vec!["alpha".to_string(), "beta".to_string()]);
}
