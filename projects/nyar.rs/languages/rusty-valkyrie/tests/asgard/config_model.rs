//! 首轮 static 配置严格模型的回归测试：默认值、拒绝路径与规范化。

use std::{fs, path::Path};

use rusty_valkyrie::asgard::config_model::{
    BuildOptions, ChunkBoundary, ChunkBudgets, ChunkPlan, ConfigOrigin, ConfigValue, ResolvedAsgardConfig, StaticArtifactTarget,
    resolve_static_config,
};
use tempfile::TempDir;

/// 构造带入口文件的项目根与配置源身份。
fn project_origin(temp: &Path, entry_relative: &str) -> ConfigOrigin {
    let entry = temp.join(entry_relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(entry.parent().expect("入口应有父目录")).expect("创建入口目录失败");
    fs::write(&entry, "<widget></widget>\n").expect("写入入口文件失败");
    ConfigOrigin { config_path: temp.join("asgard.config.v"), project_root: temp.to_path_buf() }
}

fn obj(fields: Vec<(&str, ConfigValue)>) -> ConfigValue {
    ConfigValue::Object(fields.into_iter().map(|(name, value)| (name.to_owned(), value)).collect())
}

fn text(value: &str) -> ConfigValue {
    ConfigValue::Text(value.to_owned())
}

/// 最小合法配置：只声明入口。
fn minimal(entry: &str) -> ConfigValue {
    obj(vec![("artifact", obj(vec![("entry", text(entry))]))])
}

fn resolve(temp: &Path, entry: &str, value: ConfigValue) -> Result<ResolvedAsgardConfig, String> {
    resolve_static_config(value, project_origin(temp, entry)).map_err(|error| error.to_string())
}

fn expect_error(result: Result<ResolvedAsgardConfig, String>, needle: &str) -> String {
    let message = result.expect_err(&format!("应拒绝并包含 `{needle}`"));
    assert!(message.contains(needle), "诊断应包含 `{needle}`，实际：{message}");
    message
}

#[test]
fn minimal_config_fills_documented_defaults() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let config = resolve(temp.path(), "source/App.w", minimal("source/App.w")).expect("最小配置应通过校验");

    assert_eq!(config.target, StaticArtifactTarget::Static);
    assert_eq!(config.entry, temp.path().join("source").join("App.w"));
    assert_eq!(config.output, temp.path().join("dist"));
    assert_eq!(config.base, "/");
    assert_eq!(config.chunks, ChunkPlan::default());
    assert_eq!(config.assets, None);
    assert_eq!(config.build, BuildOptions::default());
    assert_eq!(config.serve.host, "127.0.0.1");
    assert_eq!(config.serve.port, 3000);
    assert_eq!(config.dev.host, "127.0.0.1");
    assert_eq!(config.dev.port, 3000);
}

#[test]
fn full_config_resolves_every_section() {
    let temp = TempDir::new().expect("创建临时目录失败");
    fs::create_dir_all(temp.path().join("static")).expect("创建资源目录失败");
    let value = obj(vec![
        (
            "artifact",
            obj(vec![
                ("target", text("static")),
                ("entry", text("source/App.awsl")),
                ("output", text("build/site")),
                ("base", text("/app/")),
                (
                    "chunks",
                    obj(vec![
                        ("boundaries", ConfigValue::List(vec![obj(vec![("component", text("Admin")), ("preload", ConfigValue::Bool(true))])])),
                        ("budgets", obj(vec![("entry_bytes", ConfigValue::Int(40_000)), ("chunk_bytes", ConfigValue::Int(20_000))])),
                    ]),
                ),
            ]),
        ),
        ("assets", text("static")),
        ("build", obj(vec![("minify", ConfigValue::Bool(false)), ("source_map", ConfigValue::Bool(true))])),
        ("serve", obj(vec![("host", text("0.0.0.0")), ("port", ConfigValue::Int(8080))])),
        ("dev", obj(vec![("port", ConfigValue::Int(5173))])),
    ]);

    let config = resolve(temp.path(), "source/App.awsl", value).expect("完整配置应通过校验");
    assert_eq!(config.output, temp.path().join("build").join("site"));
    assert_eq!(config.base, "/app", "目录形基址应去掉尾斜杠");
    assert_eq!(
        config.chunks,
        ChunkPlan {
            boundaries: vec![ChunkBoundary { component: "Admin".into(), preload: true, prefetch: false }],
            budgets: ChunkBudgets { entry_bytes: Some(40_000), chunk_bytes: Some(20_000) },
        }
    );
    assert_eq!(config.assets, Some(temp.path().join("static")));
    assert_eq!(config.build, BuildOptions { minify: Some(false), source_map: Some(true) });
    assert_eq!(config.serve.port, 8080);
    assert_eq!(config.dev.port, 5173);
}

#[test]
fn unknown_root_field_is_rejected() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w"))])), ("platform", text("browser"))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "未知字段 `platform`");
}

#[test]
fn unknown_artifact_field_is_rejected() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w")), ("target_triple", text("wasm32"))]))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "未知字段 `target_triple`");
}

#[test]
fn duplicate_field_is_rejected() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w"))])), ("artifact", obj(vec![("entry", text("source/B.w"))]))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "重复字段 `artifact`");
}

#[test]
fn missing_artifact_and_entry_are_rejected() {
    let temp = TempDir::new().expect("创建临时目录失败");
    expect_error(resolve(temp.path(), "source/App.w", obj(vec![])), "缺少必需字段 `artifact`");
    let value = obj(vec![("artifact", obj(vec![]))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "缺少必需字段 `entry`");
}

#[test]
fn entry_type_and_suffix_are_enforced() {
    let temp = TempDir::new().expect("创建临时目录失败");
    expect_error(resolve(temp.path(), "source/App.w", ConfigValue::Int(1)), "期望对象");
    expect_error(resolve(temp.path(), "source/App.w", obj(vec![("artifact", obj(vec![("entry", ConfigValue::Int(7))]))])), "artifact.entry");
    expect_error(resolve(temp.path(), "source/App.v", obj(vec![("artifact", obj(vec![("entry", text("source/App.v"))]))])), "`.w` / `.awsl`");
}

#[test]
fn entry_must_exist_and_stay_in_root() {
    let temp = TempDir::new().expect("创建临时目录失败");
    // 不创建入口文件：存在性检查必须失败闭合。
    let origin = ConfigOrigin { config_path: temp.path().join("asgard.config.v"), project_root: temp.path().to_path_buf() };
    let message =
        resolve_static_config(minimal("source/App.w"), origin).map_err(|error| error.to_string()).expect_err("缺失入口应被拒绝").to_string();
    assert!(message.contains("不存在或不可访问"), "诊断应指出入口缺失：{message}");

    expect_error(resolve(temp.path(), "source/App.w", minimal("../outside.w")), "越过项目根");
}

#[test]
fn non_static_target_is_rejected() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let value = obj(vec![("artifact", obj(vec![("target", text("server")), ("entry", text("source/App.w"))]))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "首轮只接受 `static`");

    let value = obj(vec![("artifact", obj(vec![("target", ConfigValue::Bool(true)), ("entry", text("source/App.w"))]))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "期望文本 `static`");
}

#[test]
fn output_must_not_escape_root_or_swallow_entry() {
    let temp = TempDir::new().expect("创建临时目录失败");
    expect_error(
        resolve(temp.path(), "source/App.w", obj(vec![("artifact", obj(vec![("entry", text("source/App.w")), ("output", text("../out"))]))])),
        "越过项目根",
    );
    expect_error(
        resolve(temp.path(), "source/App.w", obj(vec![("artifact", obj(vec![("entry", text("source/App.w")), ("output", text("."))]))])),
        "不能是项目根",
    );

    let value = obj(vec![("artifact", obj(vec![("entry", text("dist/App.w")), ("output", text("dist"))]))]);
    expect_error(resolve(temp.path(), "dist/App.w", value), "覆盖输入");
}

#[test]
fn base_normalization_and_rejections() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let with_base = |base: &str| obj(vec![("artifact", obj(vec![("entry", text("source/App.w")), ("base", text(base))]))]);

    let message = expect_error(resolve(temp.path(), "source/App.w", with_base("app")), "以 `/` 开头");
    assert!(message.contains("artifact.base"), "诊断应带字段路径：{message}");
    expect_error(resolve(temp.path(), "source/App.w", with_base("")), "不能为空文本");
    expect_error(resolve(temp.path(), "source/App.w", with_base("/a/../b")), "不允许包含 `..` 段");

    let config = resolve(temp.path(), "source/App.w", with_base("/sub/")).expect("合法 base 应通过");
    assert_eq!(config.base, "/sub");
}

#[test]
fn assets_must_be_existing_directory() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w"))])), ("assets", text("missing"))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "不存在或不可访问");

    let file = temp.path().join("not-a-dir.txt");
    fs::write(&file, "x").expect("写入文件失败");
    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w"))])), ("assets", text("not-a-dir.txt"))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "不是目录");
}

#[test]
fn chunks_reject_unknown_fields_and_empty_component() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let with_chunks = |chunks: ConfigValue| obj(vec![("artifact", obj(vec![("entry", text("source/App.w")), ("chunks", chunks)]))]);

    let value =
        with_chunks(obj(vec![("boundaries", ConfigValue::List(vec![obj(vec![("component", text("A")), ("lazy", ConfigValue::Bool(true))])]))]));
    expect_error(resolve(temp.path(), "source/App.w", value), "未知字段 `lazy`");

    let value = with_chunks(obj(vec![("boundaries", ConfigValue::List(vec![obj(vec![("component", text(""))])]))]));
    expect_error(resolve(temp.path(), "source/App.w", value), "不能为空文本");

    let value = with_chunks(obj(vec![("boundaries", text("A"))]));
    expect_error(resolve(temp.path(), "source/App.w", value), "期望列表");

    let value = with_chunks(obj(vec![("budgets", obj(vec![("entry_bytes", ConfigValue::Int(-1))]))]));
    expect_error(resolve(temp.path(), "source/App.w", value), "非负整数");
}

#[test]
fn build_and_endpoints_are_strict() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w"))])), ("build", obj(vec![("mode", text("prod"))]))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "未知字段 `mode`");

    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w"))])), ("serve", obj(vec![("port", ConfigValue::Int(70_000))]))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "1..=65535");

    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w"))])), ("dev", obj(vec![("port", ConfigValue::Int(0))]))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "1..=65535");

    let value = obj(vec![("artifact", obj(vec![("entry", text("source/App.w"))])), ("dev", obj(vec![("port", text("3000"))]))]);
    expect_error(resolve(temp.path(), "source/App.w", value), "期望整数");
}
