use nyar_types::{CanonicalCallee, CanonicalOperation, ItemInstanceId};
use rusty_valkyrie::compiler::{
    emitter::pipeline::{CompilerHostProviderBinding, resolve_host_provider_bindings},
    hir::lowering::{CompilerSourceGroup, ValkyrieCompiler},
    semantic::closure::{DeclarationKey, DeclarationKind, micro_declaration_key},
};

const TEST_ARCH: &str = "native";

/// 组模式下 numeric primitive 必须由依赖组声明为 primitive 结构；否则残留
/// `Named("i32")` 会以 CAN003 拒绝进入 canonical 类型表。声明文本统一来自测试侧
/// 共享前导，不在此处另抄一份。
const CORE_PRIMITIVE_SOURCE: &str = crate::primitives::PRIMITIVES;

fn core_group() -> CompilerSourceGroup {
    CompilerSourceGroup::single_source("core", "core", CORE_PRIMITIVE_SOURCE)
}

fn user_linked_import_count(canonical: &nyar_types::CanonicalProgram) -> usize {
    canonical
        .linked
        .imports
        .values()
        .filter(|import| {
            let label = import.capability.to_string();
            !label.ends_with("::begin_phase") && !label.ends_with("::end_phase")
        })
        .count()
}

fn invoke_callee(canonical: &nyar_types::CanonicalProgram, instance: ItemInstanceId) -> Option<CanonicalCallee> {
    canonical.mir.functions.get(&instance)?.blocks.values().flat_map(|block| block.instructions.iter()).find_map(|instruction| {
        match &instruction.operation {
            CanonicalOperation::Invoke { callee, .. } => Some(*callee),
            _ => None,
        }
    })
}

#[test]
fn selected_host_provider_rewrites_contract_calls_in_full_source_closure() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "[host_provider(demo::write)] micro write(value: i32) -> i32 { return value }",
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro write(value: i32) -> i32 { } \
                [main] micro main() -> i32 { return write(1) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &[], "write"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &[], "write"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("host provider binding must close across dependency groups");
    let canonical = program.canonical();
    assert_eq!(user_linked_import_count(canonical), 0, "contract import must be rewritten to provider callable");
    let main_instance = *canonical.linked.entries.keys().next().expect("main entry");
    let call = invoke_callee(canonical, main_instance).expect("main must call host write");
    let provider_instance = match call {
        CanonicalCallee::Item(instance) => instance,
        other => panic!("host binding must rewrite to local provider callable, got {other:?}"),
    };
    assert_ne!(provider_instance, main_instance);
    assert!(canonical.mir.functions.contains_key(&provider_instance));
}

#[test]
fn bodyless_target_bound_provider_carries_its_own_import_contract() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "[host_provider(demo::add)] [wasm_operator(\"i32.add\")] micro i32_add(a: i32, b: i32): i32",
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro add(a: i32, b: i32): i32 [main] micro main() -> i32 { return add(1, 2) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &[], "add"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &[], "i32_add"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("无体目标绑定 provider 必须闭合绑定链");
    let canonical = program.canonical();
    let imports = canonical
        .linked
        .imports
        .values()
        .filter(|import| {
            let label = import.capability.to_string();
            !label.ends_with("::begin_phase") && !label.ends_with("::end_phase")
        })
        .collect::<Vec<_>>();
    assert_eq!(imports.len(), 1, "provider 的目标绑定必须取代原合同导入");
    assert!(imports[0].link.matches_platform_tag("wasm-operator"), "link={:?}", imports[0].link);
    assert_eq!(imports[0].link.locator_segments(), &["i32.add"]);
    let main_instance = *canonical.linked.entries.keys().next().expect("main entry");
    let call = invoke_callee(canonical, main_instance).expect("main must call add");
    assert_eq!(call, CanonicalCallee::Item(imports[0].callee));
}

#[test]
fn bodyless_js_builtin_provider_carries_js_import_contract() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "[host_provider(demo::rand)] [js_builtin(\"random\")] micro js_rand(seed: i32): i32",
        )
            .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro rand(seed: i32): i32 [main] micro main() -> i32 { return rand(1) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &[], "rand"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &[], "js_rand"),
        required_capabilities: vec!["js.random".into()],
        foreign_bindings: vec![r#"js_builtin("random")"#.into()],
    }];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("bodyless [js_builtin] provider must close binding chain");
    let canonical = program.canonical();
    let imports = canonical
        .linked
        .imports
        .values()
        .filter(|import| {
            let label = import.capability.to_string();
            !label.ends_with("::begin_phase") && !label.ends_with("::end_phase")
        })
        .collect::<Vec<_>>();
    assert_eq!(imports.len(), 1, "provider js binding must become external import");
    assert!(imports[0].link.matches_platform_tag("js_builtin"), "link={:?}", imports[0].link);
    assert_eq!(imports[0].link.locator_segments(), &["random"]);
    let main_instance = *canonical.linked.entries.keys().next().expect("main entry");
    let call = invoke_callee(canonical, main_instance).expect("main must call rand");
    assert_eq!(call, CanonicalCallee::Item(imports[0].callee));
}

#[test]
fn same_source_file_unselected_js_builtin_provider_is_pruned_from_linked_imports() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "namespace std.adaptor.js;\n\
             [host_provider(demo::fetch)] [js_builtin(\"fetch\")] micro js_fetch(x: i32): i32\n\
             [host_provider(demo::random)] [js_builtin(\"random\")] micro js_random(x: i32): i32",
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro fetch(x: i32): i32\n\
             [host_contract] micro random(x: i32): i32\n\
             [main] micro main() -> i32 { return random(1) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let js_ns = ["std".to_string(), "adaptor".to_string(), "js".to_string()];
    let bindings = vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &[], "random"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &js_ns, "js_random"),
        required_capabilities: vec!["js.random".into()],
        foreign_bindings: vec![r#"js_builtin("random")"#.into()],
    }];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("unselected sibling provider in same file must not link");
    let canonical = program.canonical();
    let imports = canonical
        .linked
        .imports
        .values()
        .filter(|import| {
            let label = import.capability.to_string();
            !label.ends_with("::begin_phase") && !label.ends_with("::end_phase")
        })
        .collect::<Vec<_>>();
    assert_eq!(imports.len(), 1);
    assert_eq!(imports[0].link.locator_segments(), &["random"]);
}

#[test]
fn selected_host_binding_does_not_link_sibling_provider_import() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "[host_provider(demo::fetch)] [wasm_operator(\"i32.sub\")] micro wasm_fetch(x: i32): i32\n\
             [host_provider(demo::random)] [wasm_operator(\"i32.mul\")] micro wasm_random(x: i32): i32",
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro fetch(x: i32): i32\n\
             [host_contract] micro random(x: i32): i32\n\
             [main] micro main() -> i32 { return random(1) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &[], "random"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &[], "wasm_random"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("only selected provider import must link");
    let canonical = program.canonical();
    let imports = canonical
        .linked
        .imports
        .values()
        .filter(|import| {
            let label = import.capability.to_string();
            !label.ends_with("::begin_phase") && !label.ends_with("::end_phase")
        })
        .collect::<Vec<_>>();
    assert_eq!(imports.len(), 1, "sibling provider import must not enter canonical surface");
    assert_eq!(imports[0].link.locator_segments(), &["i32.mul"]);
}

#[test]
fn resolved_call_to_bodyless_host_contract_without_provider_fails_at_boundary() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "[host_provider(demo::write)] micro write(value: i32) -> i32 { return value }",
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro write(value: i32) -> i32 { } \
                [main] micro main() -> i32 { return write(1) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let error = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], TEST_ARCH)
        .expect_err("已解析调用指向未绑定 provider 的无体合同必须 fail-closed");
    let message = error.to_string();
    assert!(message.contains("no host provider is bound"), "expected compile-boundary gate failure, got: {message}");
    assert!(message.contains("write"), "failure must name the contract identity, got: {message}");
}

#[test]
fn unreferenced_bodyless_host_contract_import_does_not_block_compilation() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro write(value: i32) -> i32 { } \
                [main] micro main() -> i32 { return 0 }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], TEST_ARCH)
        .expect("无体合同存在但没有任何已解析调用时不得触发编译边界门禁");
}

#[test]
fn unreached_dependency_host_contract_does_not_block_compilation() {
    // 编译边界门禁只裁决 linked consumer 中的已解析调用：
    // 依赖组里声明的无体合同（11 题 `std::primitive::__u16_add` 形态）
    // 若没有被可达入口沿 Call 触达，不得按闭包保守超集判失败。
    let core = CompilerSourceGroup::single_source(
        "core",
        "core",
        "namespace std::primitive;\n\
         [primitive(\"std::primitive::i32\")] structure i32 { }\n\
         [host_contract] micro __u16_add(lhs: i32, rhs: i32): i32\n\
         micro wrapper(lhs: i32, rhs: i32): i32 { return __u16_add(lhs, rhs) }",
    );
    let app = CompilerSourceGroup::single_source("app", "demo", "[main] micro main() -> i32 { return 0 }")
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]);
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&[core, app], &[], &[], TEST_ARCH)
        .expect("依赖组未触达的无体合同不得触发编译边界门禁");
}

#[test]
fn imply_method_host_contract_binds_provider_and_retires_contract_body() {
    // 回归：`imply` 成员的 `[host_contract]` 目标在声明绑定阶段被冻结为 `Nominal { declaration }`，
    // 身份登记必须按冻结 ItemId 恢复聚合短名并命中选定 provider；重映射后合同体必须从
    // linked 模块退役，不得作为孤儿定义进入编译产物（合同体携带聚合字段指令）。
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "std",
            "std",
            concat!(
                "namespace std.text;\n",
                "structure Utf8Text { _repr: i32 }\n",
                "imply Utf8Text {\n",
                "   [host_contract] micro length(self) -> i32 { }\n",
                "}"
            ),
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "namespace std.adaptor.wasm.text;\n\
             using std.text.{Utf8Text};\n\
             [host_provider(std::text::Utf8Text::length)] micro host_length(value: Utf8Text): i32 { return value._repr }",
        )
        .with_direct_dependencies(vec!["core".into(), "std".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "namespace demo;\n\
             using std.text.{Utf8Text};\n\
             [main] micro main() -> i32 { let t = Utf8Text { _repr: 5 }; return t.length() }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "std".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: DeclarationKey {
            dependency_key: "std".into(),
            source_path: "std.v".into(),
            namespace: vec!["std".into(), "text".into()],
            kind: DeclarationKind::ImplyMethod,
            owner: Some("Utf8Text".into()),
            name: "length".into(),
        },
        provider: micro_declaration_key("adaptor", "adaptor.v", &["std".into(), "adaptor".into(), "wasm".into(), "text".into()], "host_length"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let hirs = ValkyrieCompiler::default()
        .resolve_source_groups_with_host_bindings(&groups, &bindings, TEST_ARCH)
        .expect("ImplyMethod 合同组必须完成 HIR 物化");
    let refs = hirs.iter().collect::<Vec<_>>();
    let resolved = resolve_host_provider_bindings(&refs, &bindings).expect("resolve 必须结构化成功");
    assert_eq!(resolved.len(), 1, "ImplyMethod 合同必须命中选定 provider");
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("ImplyMethod 合同绑定 provider 后必须完整编译");
    let canonical = program.canonical();
    assert!(
        resolved[0].contract_instances.iter().all(|instance| !canonical.mir.functions.contains_key(instance)),
        "合同体与其实例化副本必须随重映射退役，不得留在编译产物"
    );
    let bound_provider_instance = *resolved[0].provider_pairs.values().next().expect("闭合合同实例必须配到 provider 实例");
    assert!(canonical.mir.functions.contains_key(&bound_provider_instance), "provider 实现必须进入编译产物");
}

#[test]
fn unreached_contract_with_drifted_provider_signature_does_not_block_compilation() {
    // 回归：provider 已登记但与合同签名漂移，且合同从未被可达代码调用。
    // resolve 只记录 unpaired，不得让无关合同连坐整个构建。
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "std",
            "std",
            concat!(
                "namespace std.text;\n",
                "structure Utf8Text { _repr: i32 }\n",
                "imply Utf8Text {\n",
                "   [host_contract] micro length(self) -> i32 { }\n",
                "}"
            ),
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "namespace std.adaptor.wasm.text;\n\
             using std.text.{Utf8Text};\n\
             [host_provider(std::text::Utf8Text::length)] micro host_length(value: Utf8Text): i64 {\n\
                 let probe: i32 = value.length();\n\
                 return 0\n\
             }",
        )
        .with_direct_dependencies(vec!["core".into(), "std".into()]),
        CompilerSourceGroup::single_source("app", "demo", "[main] micro main() -> i32 { return 0 }")
            .as_entry()
            .with_direct_dependencies(vec!["core".into(), "std".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: DeclarationKey {
            dependency_key: "std".into(),
            source_path: "std.v".into(),
            namespace: vec!["std".into(), "text".into()],
            kind: DeclarationKind::ImplyMethod,
            owner: Some("Utf8Text".into()),
            name: "length".into(),
        },
        provider: micro_declaration_key("adaptor", "adaptor.v", &["std".into(), "adaptor".into(), "wasm".into(), "text".into()], "host_length"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let hirs = ValkyrieCompiler::default()
        .resolve_source_groups_with_host_bindings(&groups, &bindings, TEST_ARCH)
        .expect("签名漂移的 provider 组必须完成 HIR 物化");
    let refs = hirs.iter().collect::<Vec<_>>();
    let resolved = resolve_host_provider_bindings(&refs, &bindings).expect("未达合同的签名漂移不得 abort resolve");
    assert_eq!(resolved.len(), 1, "绑定必须保留但只记录 unpaired");
    assert!(resolved[0].provider_pairs.is_empty(), "漂移签名不得产生配对");
    assert_eq!(resolved[0].unpaired.len(), 1, "合同实例必须记录未配对事实");
    let unpaired = resolved[0].unpaired.values().next().expect("unpaired 必须带诊断事实");
    assert_eq!(unpaired.provider_declaration.name, "host_length");
    assert_eq!(unpaired.provider_signatures.len(), 1, "provider 闭合签名必须被记录以自证漂移");
    let unpaired_instance = *resolved[0].unpaired.keys().next().expect("unpaired 实例必须存在");
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("未被可达代码调用的合同不得连坐构建");
    assert!(
        !program.canonical().mir.functions.contains_key(&unpaired_instance),
        "未达合同实例不得进入编译产物"
    );
}

#[test]
fn resolved_call_to_drifted_provider_signature_fails_at_boundary() {
    // 反例：同一签名漂移一旦被可达调用引用，必须在链接后门禁结构化失败，
    // 且同时打出合同侧与 provider 侧签名，不得退化成静默通过或后端悬空调用。
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "std",
            "std",
            concat!(
                "namespace std.text;\n",
                "structure Utf8Text { _repr: i32 }\n",
                "imply Utf8Text {\n",
                "   [host_contract] micro length(self) -> i32 { }\n",
                "}"
            ),
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "namespace std.adaptor.wasm.text;\n\
             using std.text.{Utf8Text};\n\
             [host_provider(std::text::Utf8Text::length)] micro host_length(value: Utf8Text): i64 { return 0 }",
        )
        .with_direct_dependencies(vec!["core".into(), "std".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "namespace demo;\n\
             using std.text.{Utf8Text};\n\
             [main] micro main() -> i32 { let t = Utf8Text { _repr: 5 }; return t.length() }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "std".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: DeclarationKey {
            dependency_key: "std".into(),
            source_path: "std.v".into(),
            namespace: vec!["std".into(), "text".into()],
            kind: DeclarationKind::ImplyMethod,
            owner: Some("Utf8Text".into()),
            name: "length".into(),
        },
        provider: micro_declaration_key("adaptor", "adaptor.v", &["std".into(), "adaptor".into(), "wasm".into(), "text".into()], "host_length"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let error = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect_err("可达调用指向签名漂移的 provider 必须 fail-closed");
    let message = error.to_string();
    for needle in ["no host provider is bound", "length", "contract signature", "host_length", "-> i32", "-> i64"] {
        assert!(message.contains(needle), "missing `{needle}` in boundary failure: {message}");
    }
    assert!(!message.contains("同签名的 provider 实例"), "旧 resolve 期错误必须消失: {message}");
}

#[test]
fn overloaded_imply_methods_outside_bindings_are_each_bound_and_do_not_fail_host_scan() {
    // 回归（leetcode 矩阵 wasm 构建）：`Utf8Builder::append` 有两个合法重载（不同参数类型），
    // host binding 扫描按编码名聚合曾把它们误判为歧义并拒绝整个构建。
    // 编码名不是已解析身份：未被任何绑定引用的重载集合必须原样通过，各实例独立可调用。
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "std",
            "std",
            concat!(
                "namespace std.text;\n",
                "structure Utf8Text { _repr: i32 }\n",
                "class Utf8Builder { bytes: i32 }\n",
                "imply Utf8Builder {\n",
                "   micro append(self, c: i32) -> i32 { return c }\n",
                "   micro append(self, text: Utf8Text) -> i32 { return text._repr }\n",
                "}"
            ),
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "[host_provider(demo::write)] micro write(value: i32) -> i32 { return value }",
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            concat!(
                "namespace demo;\n",
                "using std.text.{Utf8Text, Utf8Builder};\n",
                "[host_contract] micro write(value: i32) -> i32 { }\n",
                "[main] micro main() -> i32 {\n",
                "   let b = Utf8Builder { bytes: 1 }\n",
                "   let t = Utf8Text { _repr: 2 }\n",
                "   let x = b.append(3)\n",
                "   let y = b.append(t)\n",
                "   return write(y)\n",
                "}"
            ),
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "std".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &["demo".into()], "write"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &[], "write"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("同一编码名下的合法重载集合不得被 host binding 扫描判为歧义");
    let canonical = program.canonical();
    assert_eq!(user_linked_import_count(canonical), 0, "无关合同仍必须正常绑定到 provider");
    let main_instance = *canonical.linked.entries.keys().next().expect("main entry");
    let callees = canonical
        .mir
        .functions
        .get(&main_instance)
        .expect("main in mir")
        .blocks
        .values()
        .flat_map(|block| block.instructions.iter())
        .filter_map(|instruction| match &instruction.operation {
            CanonicalOperation::Invoke { callee, .. } => Some(*callee),
            _ => None,
        })
        .collect::<Vec<_>>();
    let append_callees = callees
        .iter()
        .filter_map(|callee| match callee {
            CanonicalCallee::Item(instance) => Some(*instance),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(append_callees.len() >= 3, "两个 append 重载调用与 write 调用必须指向各自独立实例, got {append_callees:?}");
}

#[test]
fn host_contract_binding_on_overloaded_declaration_key_fails_closed() {
    // 负向（保留既有拒绝语义）：Resolver 绑定按名字键引用合同，若编码名下聚合了
    // 多个声明身份（两个同名 `[host_contract]` 重载），名字键无法区分实现目标，
    // 必须结构化拒绝，不得取第一个候选或按名字表放行。
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "std",
            "std",
            concat!(
                "namespace std.text;\n",
                "class Utf8Builder { bytes: i32 }\n",
                "imply Utf8Builder {\n",
                "   [host_contract] micro append(self, c: i32) -> i32 { return c }\n",
                "   [host_contract] micro append(self, width: i32, height: i32) -> i32 { return height }\n",
                "}"
            ),
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "namespace std.adaptor.wasm.text;\n\
             using std.text.{Utf8Builder};\n\
             [host_provider(std::text::Utf8Builder::append)] micro host_append(value: Utf8Builder): i32 { return value.bytes }",
        )
        .with_direct_dependencies(vec!["core".into(), "std".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "namespace demo;\n\
             using std.text.{Utf8Builder};\n\
             [main] micro main() -> i32 { let b = Utf8Builder { bytes: 1 }; return b.append(2) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "std".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: DeclarationKey {
            dependency_key: "std".into(),
            source_path: "std.v".into(),
            namespace: vec!["std".into(), "text".into()],
            kind: DeclarationKind::ImplyMethod,
            owner: Some("Utf8Builder".into()),
            name: "append".into(),
        },
        provider: micro_declaration_key("adaptor", "adaptor.v", &["std".into(), "adaptor".into(), "wasm".into(), "text".into()], "host_append"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let hirs = ValkyrieCompiler::default()
        .resolve_source_groups_with_host_bindings(&groups, &bindings, TEST_ARCH)
        .expect("重载合同组必须完成 HIR 物化");
    let refs = hirs.iter().collect::<Vec<_>>();
    let error = resolve_host_provider_bindings(&refs, &bindings).expect_err("编码名下多声明身份的合同绑定必须 fail-closed");
    let message = error.to_string();
    assert!(message.contains("对应多个声明身份或实例"), "expected structured ambiguity rejection, got: {message}");
    assert!(message.contains("append"), "failure must name the ambiguous declaration key, got: {message}");
}

#[test]
fn live_call_to_gated_out_provider_fails_with_contract_provider_and_feature() {
    // 活调用命中 feature gate 排除：provider 存在但 `[gate(feature = "...")]` 未启用，
    // unbound-contract 失败必须同时点名合同、被排除的 provider 与缺失的 feature，
    // 给出可操作的修复面，而不是凭空缺实现。
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "adaptor",
            "asgard.render.host.ovm",
            "namespace asgard.render.host.ovm;\n\
             [host_provider(demo::dom_create_element)] [gate(feature = \"asgard.ssg.dom\")] micro host_dom_create_element(tag: i32): i32 { return tag }",
        )
        .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro dom_create_element(tag: i32): i32 \
                [main] micro main() -> i32 { return dom_create_element(1) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let gated_out = vec![rusty_valkyrie::compiler::emitter::pipeline::CompilerGatedOutHostProvider {
        contract: "demo::dom_create_element".into(),
        provider: "asgard::render::host::ovm::host_dom_create_element".into(),
        feature: "asgard.ssg.dom".into(),
    }];
    let error = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &gated_out, TEST_ARCH)
        .expect_err("被 feature gate 排除的 provider 不得静默补位，必须 fail-closed");
    let message = error.to_string();
    assert!(message.contains("no host provider is bound"), "必须保持 unbound-contract 门禁失败: {message}");
    assert!(message.contains("demo::dom_create_element"), "失败必须点名合同限定名: {message}");
    assert!(message.contains("asgard::render::host::ovm::host_dom_create_element"), "失败必须点名被排除的 provider: {message}");
    assert!(message.contains("asgard.ssg.dom"), "失败必须点名缺失的 feature: {message}");
}

#[test]
fn dead_contract_with_gated_out_provider_does_not_block_compilation() {
    // 死调用不受影响：合同存在、gated-out 事实存在，但没有任何已解析调用触达合同，
    // 编译边界门禁不得失败（链接 BFS 不会把不可达合同并入 consumer）。
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro dom_create_element(tag: i32): i32 \
                [main] micro main() -> i32 { return 0 }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ];
    let gated_out = vec![rusty_valkyrie::compiler::emitter::pipeline::CompilerGatedOutHostProvider {
        contract: "demo::dom_create_element".into(),
        provider: "asgard::render::host::ovm::host_dom_create_element".into(),
        feature: "asgard.ssg.dom".into(),
    }];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &gated_out, TEST_ARCH)
        .expect("gated-out 事实不改变死合同的可达性语义：无已解析调用即不触发门禁");
}

#[test]
fn unbound_contract_error_stays_structured_and_does_not_degrade() {
    // 结构化错误不退化：有无 gated-out 事实，失败都保持 `HirError::FrontendRejected`
    // 包裹 `ParseError::Invalid` 的同一结构；无事实时消息保持基线形态、不编造 feature。
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            "[host_contract] micro dom_create_element(tag: i32): i32 \
                [main] micro main() -> i32 { return dom_create_element(1) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ];

    let without_facts = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], TEST_ARCH)
        .expect_err("无 provider 事实时仍必须 fail-closed");
    let rusty_valkyrie::HirError::FrontendRejected(rusty_valkyrie::compiler::syntax::oak::ParseError::Invalid { .. }) = &without_facts
    else {
        panic!("错误必须保持 FrontendRejected(ParseError::Invalid) 结构，实际: {without_facts:?}");
    };
    let base_message = without_facts.to_string();
    assert!(base_message.contains("no host provider is bound"), "基线消息必须保持 unbound 语义: {base_message}");
    assert!(!base_message.contains("is gated out"), "无事实时不得编造 feature 提示: {base_message}");

    let gated_out = vec![rusty_valkyrie::compiler::emitter::pipeline::CompilerGatedOutHostProvider {
        contract: "demo::dom_create_element".into(),
        provider: "asgard::render::host::ovm::host_dom_create_element".into(),
        feature: "asgard.ssg.dom".into(),
    }];
    let with_facts = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &gated_out, TEST_ARCH)
        .expect_err("有 provider 事实时同样 fail-closed");
    let rusty_valkyrie::HirError::FrontendRejected(rusty_valkyrie::compiler::syntax::oak::ParseError::Invalid { .. }) = &with_facts
    else {
        panic!("错误必须保持 FrontendRejected(ParseError::Invalid) 结构，实际: {with_facts:?}");
    };
    assert!(with_facts.to_string().contains("is gated out"), "有事实时必须追加 gate 提示: {with_facts}");
}

#[test]
fn provider_forwarding_to_dependency_body_rewrites_nested_contract_call() {
    // 回归（`Utf8Case` 形态）：provider 体转发到依赖组内**有体普通函数**，该普通函数
    // 再调用无体 `[host_contract]`。首轮链接看不到被转发实现，绑定阶段的调用改写也只
    // 覆盖消费模块内函数；被转发实现只能由二次链接迁入。链接器必须用规范化后的副本
    // 扫边与入栈，否则未改写的合同调用会以空 `host()` 链接进入 canonical，后端拒绝编码。
    let library = "namespace demo;\n\
        [host_contract] micro lower(value: i32): i32\n\
        [host_contract] micro shift(a: i32, b: i32): i32\n\
        micro passthrough(value: i32) -> i32 { return shift(value, 1) }\n\
        [host_provider(demo::lower)] micro host_lower(value: i32) -> i32 { return passthrough(value) }\n\
        [host_provider(demo::shift)] [wasm_operator(\"i32.add\")] micro i32_shift(a: i32, b: i32): i32";
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source("lib", "demo", library).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "app",
            "namespace app;\nusing demo::{lower, shift};\n[main] micro main() -> i32 { return shift(lower(1), 2) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "lib".into()]),
    ];
    let namespace = ["demo".to_string()];
    let bindings = vec![
        CompilerHostProviderBinding {
            contract: micro_declaration_key("lib", "demo.v", &namespace, "lower"),
            provider: micro_declaration_key("lib", "demo.v", &namespace, "host_lower"),
            required_capabilities: vec![],
            foreign_bindings: vec![],
        },
        CompilerHostProviderBinding {
            contract: micro_declaration_key("lib", "demo.v", &namespace, "shift"),
            provider: micro_declaration_key("lib", "demo.v", &namespace, "i32_shift"),
            required_capabilities: vec![],
            foreign_bindings: vec![],
        },
    ];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("provider 转发体内的无体合同必须经二次链接改写，不得残留空 host 链接");
    let canonical = program.canonical();
    assert_eq!(user_linked_import_count(canonical), 1, "二次链接只应保留 provider 自身的目标绑定导入");
    let user_import = canonical
        .linked
        .imports
        .values()
        .find(|import| {
            let label = import.capability.to_string();
            !label.ends_with("::begin_phase") && !label.ends_with("::end_phase")
        })
        .expect("provider 目标绑定导入必须存在");
    assert!(user_import.link.matches_platform_tag("wasm-operator"), "link={:?}", user_import.link);
    assert_eq!(user_import.link.locator_segments(), &["i32.add"]);
    assert!(
        !canonical
            .linked
            .imports
            .values()
            .any(|import| import.link.matches_boundary("host")
                && import.link.platform_tag.is_none()
                && import.link.locator_segments.is_empty()),
        "二次链接不得把被转发实现里的无体合同残留为空 host 链接"
    );
}

#[test]
fn nested_contract_reached_only_by_relink_without_provider_fails_closed() {
    // 反例（`Utf8Case` 漏绑定）：被转发实现里的无体合同在第一次链接时不可达，
    // 绑定阶段门禁看不到它，只有二次链接把它带入消费模块。此时必须在链接后门禁
    // 以带合同名与实例号的结构化错误失败，而不是把空 `host()` 链接留给后端 panic。
    let library = "namespace demo;\n\
        [host_contract] micro lower(value: i32): i32\n\
        [host_contract] micro shift(a: i32, b: i32): i32\n\
        micro passthrough(value: i32) -> i32 { return shift(value, 1) }\n\
        [host_provider(demo::lower)] micro host_lower(value: i32) -> i32 { return passthrough(value) }";
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source("lib", "demo", library).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source(
            "app",
            "app",
            "namespace app;\nusing demo::lower;\n[main] micro main() -> i32 { return lower(1) }",
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "lib".into()]),
    ];
    let namespace = ["demo".to_string()];
    let bindings = vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("lib", "demo.v", &namespace, "lower"),
        provider: micro_declaration_key("lib", "demo.v", &namespace, "host_lower"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let error = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect_err("二次链接引入的无体合同调用缺少 provider 时必须 fail-closed");
    let message = error.to_string();
    assert!(message.contains("host contract `demo::shift`"), "失败必须点名合同限定名: {message}");
    assert!(message.contains("no host provider is bound"), "必须保持 unbound-contract 门禁语义: {message}");
    assert!(message.contains("instance `ItemInstanceId("), "失败必须带冻结实例号: {message}");
}
