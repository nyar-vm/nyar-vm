//! `valkyrie.v` 真实 std 源码与真实 AWSL 生成单元的同批全构建。
//!
//! 原始阻断是 `asgard.generated` 携带的 `i32`/`bool` 与 std 的 `[primitive]` 声明
//! 同身份，闭包建表直接把真实 asgard 构建判死。这里不复述闭包夹具，而是把真实 std
//! 包注册进测试工作区，走生产静态站构建链路（AWSL 降级、生成组挂接、完整编译），
//! 同时用同一批源码重建闭包，确认身份表 owner 落在 std 组。

use std::fs;
use std::path::PathBuf;

use oak_awsl::AwslParser;
use rusty_valkyrie::{
    asgard::{
        awsl::{LoweringOptions, lower_component},
        codegen::{GeneratedVSource, ScalarBootstrap, build_awsl_wasm_source},
        static_site::{StaticSiteOptions, build_static_site},
    },
    legion::CanonicalTarget,
    plan_semantic_closure, ClosureGroupInput,
};
use tempfile::TempDir;

const GENERATED_GROUP_KEY: &str = "asgard.generated";

/// 含 `<style>` 块与 script 字段绑定的根组件；widget 名与入口文件 stem（`index`）一致。
const FIXTURE: &str = r#"<widget index>
<Column class="hero">
    <Text class="hero-title">{greeting}</Text>
</Column>
</widget>
<script>
let greeting: utf8 = "你好，真实 std"
</script>
<style>
root { color: @brand }
</style>
"#;

/// 与 legion 测试一致的 `valkyrie.v` 定位：优先 `VALKYRIE_V_ROOT`，其次兄弟检出。
fn valkyrie_v_root() -> Option<PathBuf> {
    let candidates = std::env::var("VALKYRIE_V_ROOT").ok().map(PathBuf::from).into_iter().chain(std::iter::once(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../valkyrie.v"),
    ));
    candidates.into_iter().find(|root| root.join("projects").join("std").join("legion.von").is_file())
}

fn package_path(root: &std::path::Path, parts: &[&str]) -> String {
    let path = parts.iter().fold(root.to_path_buf(), |path, part| path.join(part));
    path.to_string_lossy().replace('\\', "/")
}

/// 工作区同时注册渲染合同族与真实 std（含其 `unicode.types` 依赖所在的嵌套工作区）。
fn workspace_manifest(root: &std::path::Path, name: &str) -> String {
    format!(
        r#"{{
    name: "{name}-ws",
    members: [
        "{}",
        "{}",
        "{}",
        "{}"
    ]
}}
"#,
        package_path(root, &["projects", "asgard._", "projects", "asgard.render"]),
        package_path(root, &["projects", "asgard._", "projects", "asgard.render.host.ovm"]),
        package_path(root, &["projects", "std"]),
        package_path(root, &["projects", "unicode._"]),
    )
}

/// 项目清单：渲染合同族与真实 std 都按 workspace 成员解析。
fn legion_manifest(name: &str) -> String {
    format!(
        r#"{{
    name: "{name}",
    version: "0.1.0",
    dependencies: {{
        "asgard.render": {{ version: "workspace" }},
        "asgard.render.host.ovm": {{ version: "workspace" }},
        "std": {{ version: "workspace" }}
    }},
    build: [
        {{ target: "wasm32-unknown-browser-wasm" }},
        {{ target: "ovm" }}
    ]
}}
"#
    )
}

fn write_project(project: &std::path::Path, root: &std::path::Path) {
    fs::create_dir_all(project.join("source")).expect("创建 source/ 目录失败");
    fs::write(project.join("legions.von"), workspace_manifest(root, "primitive-identity-e2e")).expect("写入 legions.von 失败");
    fs::write(project.join("legion.von"), legion_manifest("primitive-identity-e2e")).expect("写入 legion.von 失败");
    fs::write(project.join("source").join("index.awsl"), FIXTURE).expect("写入入口文件失败");
}

fn generated_source() -> GeneratedVSource {
    let root = AwslParser::parse_root(FIXTURE).expect("parse awsl");
    let lowered = lower_component(&root, "index", "source/index.awsl", &LoweringOptions::default()).expect("lower awsl component");
    build_awsl_wasm_source(&[lowered], ScalarBootstrap::StdVisible).expect("generate awsl wasm source")
}

#[test]
fn std_sources_and_generated_unit_compile_together_with_std_identity_owner() {
    let Some(root) = valkyrie_v_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let temp = TempDir::new().expect("创建临时目录失败");
    let project = temp.path();
    write_project(project, &root);
    let target = CanonicalTarget::browser();
    let generated = generated_source();
    assert!(!generated.source.contains("structure bool"), "生成单元不得重声明 `bool`：{}", generated.source);
    assert!(generated.source.contains("using std.primitive;"), "std 可见时生成单元必须只导入标量声明：{}", generated.source);
    assert!(!generated.source.contains("[primitive("), "std 可见时生成单元不得再造标量载体：{}", generated.source);
    assert!(generated.extra_sources.is_empty(), "std 可见时生成组不得附带标量载体文件");

    // 身份表 owner：真实 std 组的声明必须是规范 owner，生成组只是别名载体。
    let resolved = rusty_valkyrie::asgard::compile::resolve_project_source_groups(project, &target).expect("解析真实项目源码组");
    let files = resolved
        .groups
        .iter()
        .map(|group| group.sources.iter().map(|file| (file.path.clone(), file.source.clone())).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let dependency_keys = resolved
        .groups
        .iter()
        .map(|group| group.direct_dependencies.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    // 生成组显式依赖 std：标量声明只经 `using std.primitive;` 消费，规范 owner 唯一落在 std 组。
    let mut generated_files = vec![("asgard/generated.v".to_string(), generated.source.clone())];
    generated_files.extend(generated.extra_sources.iter().map(|file| (file.path.clone(), file.source.clone())));
    let mut generated_dependencies: Vec<String> =
        resolved.groups.iter().filter(|group| group.is_entry).map(|group| group.dependency_key.clone()).collect();
    generated_dependencies.extend(resolved.groups.iter().filter(|group| group.name == "std").map(|group| group.dependency_key.clone()));
    let generated_dependency_refs: Vec<&str> = generated_dependencies.iter().map(String::as_str).collect();
    let mut inputs = resolved
        .groups
        .iter()
        .zip(&files)
        .zip(&dependency_keys)
        .map(|((group, files), direct_dependencies)| ClosureGroupInput {
            // 静态站交付走 `GeneratedEntrySurface::GeneratedOnly`：项目组不承担入口，
            // 唯一入口是生成单元；闭包镜像必须同口径。
            dependency_key: &group.dependency_key,
            module_name: &group.name,
            direct_dependencies,
            is_entry: false,
            files: files.as_slice(),
        })
        .collect::<Vec<_>>();
    inputs.push(ClosureGroupInput {
        dependency_key: GENERATED_GROUP_KEY,
        module_name: GENERATED_GROUP_KEY,
        direct_dependencies: generated_dependency_refs.as_slice(),
        is_entry: true,
        files: generated_files.as_slice(),
    });
    let closure = plan_semantic_closure(&inputs, "wasm32").expect("真实 std + 生成组必须能组成闭包");
    let std_group = resolved
        .groups
        .iter()
        .find(|group| group.sources.iter().any(|file| file.path.replace('\\', "/").ends_with("primitive/i32.v")))
        .expect("真实 std 包必须包含 primitive 源码");
    for identity in ["std::primitive::i32", "std::primitive::bool"] {
        let owner = closure.primitive_identities.get(identity).unwrap_or_else(|| panic!("`{identity}` 必须登记"));
        assert_eq!(owner.declaration.dependency_key, std_group.dependency_key, "`{identity}` 的规范 owner 必须是 std 组: {:?}", owner.declaration);
    }

    // 生产静态站链路：AWSL 降级、生成组挂接、完整编译与页面产物一次成型。
    let report = build_static_site(&StaticSiteOptions {
        project_dir: project.to_path_buf(),
        entry: PathBuf::from("source/index.awsl"),
        output_dir: None,
        base: "/".into(),
        assets_dir: None,
        title: Some("primitive identity e2e".into()),
    })
    .expect("真实 std 源码与生成单元必须同批构建成功");
    assert_eq!(report.component_name, "index");
    let html = fs::read_to_string(&report.page_path).expect("读取页面失败");
    assert!(html.contains("你好，真实 std"), "页面必须来自同一批构建:\n{html}");
}
