use nyar_types::{CanonicalCallee, CanonicalOperation, ItemInstanceId};
use rusty_valkyrie::compiler::{
    emitter::pipeline::{CompilerHostProviderBinding, resolve_host_provider_bindings},
    hir::lowering::{CompilerSourceGroup, ValkyrieCompiler},
    semantic::closure::{DeclarationKey, DeclarationKind, micro_declaration_key},
};

const TEST_ARCH: &str = "native";

/// 组模式下 `i32` 必须由依赖组声明为 primitive 结构；否则残留 `Named("i32")`
/// 会以 CAN003 拒绝进入 canonical 类型表。
const CORE_PRIMITIVE_SOURCE: &str = "namespace core::primitive; [primitive(\"core::primitive::i32\")] structure i32 { }";

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
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, TEST_ARCH)
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
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, TEST_ARCH)
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
fn bodyless_js_provider_carries_js_import_contract() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source("adaptor", "adaptor", "[host_provider(demo::rand)] [js(\"random\")] micro js_rand(seed: i32): i32")
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
        foreign_bindings: vec![r#"js("random")"#.into()],
    }];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, TEST_ARCH)
        .expect("bodyless [js] provider must close binding chain");
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
    assert!(imports[0].link.matches_platform_tag("js"), "link={:?}", imports[0].link);
    assert_eq!(imports[0].link.locator_segments(), &["random"]);
    let main_instance = *canonical.linked.entries.keys().next().expect("main entry");
    let call = invoke_callee(canonical, main_instance).expect("main must call rand");
    assert_eq!(call, CanonicalCallee::Item(imports[0].callee));
}

#[test]
fn same_source_file_unselected_js_provider_is_pruned_from_linked_imports() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source(
            "adaptor",
            "adaptor",
            "namespace std.adaptor.js;\n\
             [host_provider(demo::fetch)] [js(\"fetch\")] micro js_fetch(x: i32): i32\n\
             [host_provider(demo::random)] [js(\"random\")] micro js_random(x: i32): i32",
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
        foreign_bindings: vec![r#"js("random")"#.into()],
    }];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, TEST_ARCH)
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
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, TEST_ARCH)
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
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], TEST_ARCH)
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
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], TEST_ARCH)
        .expect("无体合同存在但没有任何已解析调用时不得触发编译边界门禁");
}

#[test]
fn unreached_dependency_host_contract_does_not_block_compilation() {
    // 编译边界门禁只裁决 linked consumer 中的已解析调用：
    // 依赖组里声明的无体合同（11 题 `core::primitive::__u16_add` 形态）
    // 若没有被可达入口沿 Call 触达，不得按闭包保守超集判失败。
    let core = CompilerSourceGroup::single_source(
        "core",
        "core",
        "namespace core::primitive;\n\
         [primitive(\"core::primitive::i32\")] structure i32 { }\n\
         [host_contract] micro __u16_add(lhs: i32, rhs: i32): i32\n\
         micro wrapper(lhs: i32, rhs: i32): i32 { return __u16_add(lhs, rhs) }",
    );
    let app = CompilerSourceGroup::single_source("app", "demo", "[main] micro main() -> i32 { return 0 }")
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]);
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&[core, app], &[], TEST_ARCH)
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
            "namespace std.text;\n\
             structure Utf8Text { _repr: i32 }\n\
             imply Utf8Text {\n\
             \x20\x20\x20[host_contract] micro length(self) -> i32 { return self._repr }\n\
             }",
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
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, TEST_ARCH)
        .expect("ImplyMethod 合同绑定 provider 后必须完整编译");
    let canonical = program.canonical();
    assert!(!canonical.mir.functions.contains_key(&resolved[0].contract_instance), "合同体必须随重映射退役，不得留在编译产物");
    assert!(canonical.mir.functions.contains_key(&resolved[0].provider_instance), "provider 实现必须进入编译产物");
}
