//! 首轮 static 配置的严格模型：把配置求值产物校验并规范化为不可变的 `ResolvedAsgardConfig`。
//!
//! 本模块的输入是结构化值树 [`ConfigValue`]，不是 V 源码文本、不是 VON 文本，也不是正则或
//! `legion.von`。它不解析任何源码，只做结构校验、默认值补全和路径规范化；除入口文件与
//! 资源目录的存在性检查外不做任何文件系统读取，也绝不探测 manifest。
//!
//! 值树本身不携带源位置：本模块的诊断只带字段路径（如 `artifact.entry`）。真实的源文件与
//! span 由未来的配置求值器在其求值产物中携带；在求值器接入之前，不得在此伪造行号。
//!
//! `build` 中的 `minify` / `source_map` 保持 [`Option<bool>`]，不在求值阶段套用 production /
//! development 默认：模式（及其默认）属于构建命令在配置求值之后决定的工具执行策略。

use crate::asgard::error::{AsgardError, AsgardResult};
use std::{
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
};

/// 根层允许的字段名。
const ROOT_FIELDS: &[&str] = &["artifact", "assets", "build", "serve", "dev"];
/// `artifact` 层允许的字段名。
const ARTIFACT_FIELDS: &[&str] = &["target", "entry", "output", "base", "chunks"];
/// `artifact.chunks` 层允许的字段名。
const CHUNKS_FIELDS: &[&str] = &["boundaries", "budgets"];
/// 单条 chunk 边界允许的字段名。
const BOUNDARY_FIELDS: &[&str] = &["component", "preload", "prefetch"];
/// `artifact.chunks.budgets` 层允许的字段名。
const BUDGETS_FIELDS: &[&str] = &["entry_bytes", "chunk_bytes"];
/// `build` 层允许的字段名。
const BUILD_FIELDS: &[&str] = &["minify", "source_map"];
/// `serve` / `dev` 层允许的字段名。
const ENDPOINT_FIELDS: &[&str] = &["host", "port"];

/// 缺省监听主机：只监听本机。
const DEFAULT_HOST: &str = "127.0.0.1";
/// 缺省监听端口。
const DEFAULT_PORT: u16 = 3000;
/// 缺省输出目录（相对项目根）。
const DEFAULT_OUTPUT: &str = "dist";
/// 缺省 URL 基址（目录形）。
const DEFAULT_BASE: &str = "/";
/// `artifact.target` 首轮唯一取值。
const STATIC_TARGET: &str = "static";
/// 入口文件接受的后缀；两种后缀同格式同语义，不做优先级猜测。
const ENTRY_SUFFIXES: &[&str] = &["w", "awsl"];

/// 结构化配置值：配置求值器与严格模型之间的边界类型（不是 VON，也不是文本）。
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    /// 对象；字段按出现顺序保留，重复字段由校验阶段拒绝。
    Object(Vec<(String, ConfigValue)>),
    /// 列表。
    List(Vec<ConfigValue>),
    /// 文本。
    Text(String),
    /// 布尔值。
    Bool(bool),
    /// 整数。
    Int(i64),
    /// 浮点数。
    Float(f64),
    /// 空值。
    Null,
}

/// 配置源身份：配置求值前的定位结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigOrigin {
    /// 选中的 `asgard.config.v` 路径。
    pub config_path: PathBuf,
    /// 项目根；配置内的相对路径一律相对此根解析。
    pub project_root: PathBuf,
}

/// 首轮唯一产物类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticArtifactTarget {
    /// 只需静态 HTTP 托管的浏览器静态站点。
    Static,
}

/// 一个可分割的组件 / 路由边界。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkBoundary {
    /// 边界对应的组件标识；必须是非空文本。
    pub component: String,
    /// 入口 chunk 是否预加载。
    pub preload: bool,
    /// 已知下一步是否预取。
    pub prefetch: bool,
}

/// 分包体积预算；未配置的维度保持 `None`，不在求值阶段臆造数值。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChunkBudgets {
    /// 首屏入口 chunk 的压缩后字节预算。
    pub entry_bytes: Option<u64>,
    /// 单个 chunk 的压缩后字节预算。
    pub chunk_bytes: Option<u64>,
}

/// 分包计划：只描述允许的边界与预算，不生成任何文件名或 import 路径。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChunkPlan {
    /// 显式声明的切分边界；未配置时为不填充任何伪边界的空列表。
    pub boundaries: Vec<ChunkBoundary>,
    /// 体积预算。
    pub budgets: ChunkBudgets,
}

/// 构建选项；`None` 表示作者未声明，构建命令在求值后决定实际模式与默认。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildOptions {
    /// 是否压缩；`None` 表示由构建命令按模式决定。
    pub minify: Option<bool>,
    /// 是否生成 source map；`None` 表示由构建命令按模式决定。
    pub source_map: Option<bool>,
}

/// `serve` 的监听设置（工具运行配置，不改产物种类 / 入口 / 输出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeOptions {
    /// 监听主机。
    pub host: String,
    /// 监听端口，范围 `1..=65535`。
    pub port: u16,
}

/// `dev` 的监听设置（工具运行配置，不改产物种类 / 入口 / 输出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevOptions {
    /// 监听主机。
    pub host: String,
    /// 监听端口，范围 `1..=65535`。
    pub port: u16,
}

/// 阶段一校验后的不可变配置；此时还没有应用包图，因此不携带包身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAsgardConfig {
    /// 配置源身份。
    pub origin: ConfigOrigin,
    /// 产物类型。
    pub target: StaticArtifactTarget,
    /// 组件入口的绝对路径，保证位于项目根内且是存在的普通文件。
    pub entry: PathBuf,
    /// 输出目录的绝对路径，保证位于项目根内；缺省 `<root>/dist`。
    pub output: PathBuf,
    /// 目录形 URL 基址；缺省 `/`。
    pub base: String,
    /// 分包计划。
    pub chunks: ChunkPlan,
    /// 静态资源目录的绝对路径；存在时保证是目录。
    pub assets: Option<PathBuf>,
    /// 构建选项。
    pub build: BuildOptions,
    /// `serve` 监听设置。
    pub serve: ServeOptions,
    /// `dev` 监听设置。
    pub dev: DevOptions,
}

/// 校验并规范化首轮 static 配置。
///
/// 语义固定为设计中的首轮静态站点字段集合：未知字段、重复字段、类型不符、非 `static` 目标、
/// 越界路径、错误后缀、端口越界和缺失入口文件一律拒绝，且不回退默认配置。默认值只补充
/// 合法配置中的可选项（`target`、`output`、`base`、`serve`/`dev` 端点）。
pub fn resolve_static_config(value: ConfigValue, origin: ConfigOrigin) -> AsgardResult<ResolvedAsgardConfig> {
    let fields = as_object(&value, "")?;
    validate_fields(fields, "", ROOT_FIELDS)?;

    let artifact_value = look_up(fields, "artifact").ok_or_else(|| config_error("", "缺少必需字段 `artifact`"))?;
    let artifact_fields = as_object(artifact_value, "artifact")?;
    validate_fields(artifact_fields, "artifact", ARTIFACT_FIELDS)?;

    let target = resolve_target(look_up(artifact_fields, "target"))?;

    let entry_value = look_up(artifact_fields, "entry").ok_or_else(|| config_error("artifact.entry", "缺少必需字段 `entry`"))?;
    let entry_relative = normalize_relative(expect_non_empty_text(entry_value, "artifact.entry")?, "artifact.entry")?;
    validate_entry_suffix(&entry_relative, "artifact.entry")?;
    let entry = origin.project_root.join(&entry_relative);
    let metadata =
        fs::metadata(&entry).map_err(|error| config_error("artifact.entry", format!("`{}` 不存在或不可访问：{error}", entry.display())))?;
    if !metadata.is_file() {
        return Err(config_error("artifact.entry", format!("`{}` 不是普通文件", entry.display())));
    }

    let output_relative = match look_up(artifact_fields, "output") {
        Some(value) => normalize_relative(expect_non_empty_text(value, "artifact.output")?, "artifact.output")?,
        None => PathBuf::from(DEFAULT_OUTPUT),
    };
    if output_relative.as_os_str().is_empty() {
        return Err(config_error("artifact.output", "输出目录不能是项目根"));
    }
    if entry_relative.starts_with(&output_relative) {
        return Err(config_error(
            "artifact.output",
            format!("输出目录 `{}` 是入口 `{}` 的祖先目录，构建会覆盖输入", output_relative.display(), entry_relative.display()),
        ));
    }
    let output = origin.project_root.join(&output_relative);

    let base = match look_up(artifact_fields, "base") {
        Some(value) => normalize_base(expect_text(value, "artifact.base")?, "artifact.base")?,
        None => DEFAULT_BASE.to_owned(),
    };

    let chunks = resolve_chunks(look_up(artifact_fields, "chunks"))?;

    let assets = resolve_assets(look_up(fields, "assets"), &origin)?;
    let build = resolve_build(look_up(fields, "build"))?;
    let (serve_host, serve_port) = resolve_endpoint(look_up(fields, "serve"), "serve")?;
    let (dev_host, dev_port) = resolve_endpoint(look_up(fields, "dev"), "dev")?;

    Ok(ResolvedAsgardConfig {
        origin,
        target,
        entry,
        output,
        base,
        chunks,
        assets,
        build,
        serve: ServeOptions { host: serve_host, port: serve_port },
        dev: DevOptions { host: dev_host, port: dev_port },
    })
}

/// 构造带字段路径的诊断。
fn config_error(path: &str, detail: impl std::fmt::Display) -> AsgardError {
    let label = if path.is_empty() { "配置根" } else { path };
    AsgardError::msg(format!("asgard 配置校验失败（{label}）：{detail}"))
}

/// 拼接字段路径；根层字段直接返回字段名。
fn field_path(parent: &str, name: &str) -> String {
    if parent.is_empty() { name.to_owned() } else { format!("{parent}.{name}") }
}

/// 值树的类型名，用于类型不符诊断。
fn describe(value: &ConfigValue) -> &'static str {
    match value {
        ConfigValue::Object(_) => "对象",
        ConfigValue::List(_) => "列表",
        ConfigValue::Text(_) => "文本",
        ConfigValue::Bool(_) => "布尔值",
        ConfigValue::Int(_) => "整数",
        ConfigValue::Float(_) => "浮点数",
        ConfigValue::Null => "null",
    }
}

/// 要求对象并返回其字段表。
fn as_object<'a>(value: &'a ConfigValue, path: &str) -> AsgardResult<&'a [(String, ConfigValue)]> {
    match value {
        ConfigValue::Object(fields) => Ok(fields),
        other => Err(config_error(path, format!("期望对象，实际为 {}", describe(other)))),
    }
}

/// 要求文本。
fn expect_text<'a>(value: &'a ConfigValue, path: &str) -> AsgardResult<&'a str> {
    match value {
        ConfigValue::Text(text) => Ok(text),
        other => Err(config_error(path, format!("期望文本，实际为 {}", describe(other)))),
    }
}

/// 要求非空文本。
fn expect_non_empty_text<'a>(value: &'a ConfigValue, path: &str) -> AsgardResult<&'a str> {
    let text = expect_text(value, path)?;
    if text.trim().is_empty() {
        return Err(config_error(path, "不能为空文本"));
    }
    Ok(text)
}

/// 要求布尔值。
fn expect_bool(value: &ConfigValue, path: &str) -> AsgardResult<bool> {
    match value {
        ConfigValue::Bool(flag) => Ok(*flag),
        other => Err(config_error(path, format!("期望布尔值，实际为 {}", describe(other)))),
    }
}

/// 要求整数。
fn expect_int(value: &ConfigValue, path: &str) -> AsgardResult<i64> {
    match value {
        ConfigValue::Int(number) => Ok(*number),
        other => Err(config_error(path, format!("期望整数，实际为 {}", describe(other)))),
    }
}

/// 要求非负字节数。
fn expect_bytes(value: &ConfigValue, path: &str) -> AsgardResult<u64> {
    let number = expect_int(value, path)?;
    if number < 0 {
        return Err(config_error(path, format!("必须是非负整数，收到 {number}")));
    }
    u64::try_from(number).map_err(|_| config_error(path, format!("数值超出可表示范围：{number}")))
}

/// 拒绝未知字段与重复字段，并列出该层允许的字段名。
fn validate_fields(fields: &[(String, ConfigValue)], path: &str, allowed: &[&str]) -> AsgardResult<()> {
    let mut seen: Vec<&str> = Vec::new();
    for (name, _) in fields {
        if seen.contains(&name.as_str()) {
            return Err(config_error(path, format!("重复字段 `{name}`")));
        }
        seen.push(name.as_str());
        if !allowed.contains(&name.as_str()) {
            let names = allowed.join("、");
            return Err(config_error(path, format!("未知字段 `{name}`；该层允许的字段为：{names}")));
        }
    }
    Ok(())
}

/// 按字段名查找字段值；重复字段已在校验阶段拒绝。
fn look_up<'a>(fields: &'a [(String, ConfigValue)], name: &str) -> Option<&'a ConfigValue> {
    fields.iter().find(|(field, _)| field == name).map(|(_, value)| value)
}

/// 解析 `artifact.target`：缺省 `static`，其它取值一律拒绝。
fn resolve_target(value: Option<&ConfigValue>) -> AsgardResult<StaticArtifactTarget> {
    let Some(value) = value
    else {
        return Ok(StaticArtifactTarget::Static);
    };
    match value {
        ConfigValue::Text(text) if text == STATIC_TARGET => Ok(StaticArtifactTarget::Static),
        ConfigValue::Text(text) => Err(config_error("artifact.target", format!("首轮只接受 `{STATIC_TARGET}`，收到 `{text}`"))),
        other => Err(config_error("artifact.target", format!("期望文本 `{STATIC_TARGET}`，实际为 {}", describe(other)))),
    }
}

/// 词法规范化项目根内相对路径：拒绝绝对路径，拒绝越过项目根的 `..`。
fn normalize_relative(text: &str, path: &str) -> AsgardResult<PathBuf> {
    let raw = Path::new(text);
    let mut components: Vec<OsString> = Vec::new();
    for component in raw.components() {
        match component {
            Component::Normal(value) => components.push(value.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                if components.pop().is_none() {
                    return Err(config_error(path, format!("`{text}` 越过项目根，拒绝")));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(config_error(path, format!("`{text}` 必须是相对项目根的路径，不接受绝对路径")));
            }
        }
    }
    let mut normalized = PathBuf::new();
    for component in &components {
        normalized.push(component);
    }
    Ok(normalized)
}

/// 校验入口后缀：只接受 `.w` / `.awsl`，不做优先级猜测。
fn validate_entry_suffix(relative: &Path, path: &str) -> AsgardResult<()> {
    let suffix = relative.extension().and_then(|extension| extension.to_str());
    match suffix {
        Some(suffix) if ENTRY_SUFFIXES.contains(&suffix) => Ok(()),
        _ => Err(config_error(path, format!("入口 `{}` 只接受 `.w` / `.awsl` 后缀（两者同格式同语义）", relative.display()))),
    }
}

/// 目录形 URL 基址规范化：接受 `/`、`/app`、`/app/`，拒绝空串、根外段与缺少前导 `/` 的写法。
fn normalize_base(text: &str, path: &str) -> AsgardResult<String> {
    if text.is_empty() {
        return Err(config_error(path, "不能为空文本"));
    }
    if !text.starts_with('/') {
        return Err(config_error(path, format!("`{text}` 必须是目录形 URL 基址，以 `/` 开头")));
    }
    if text.split('/').any(|segment| segment == "..") {
        return Err(config_error(path, format!("`{text}` 不允许包含 `..` 段")));
    }
    let trimmed = text.trim_end_matches('/');
    Ok(if trimmed.is_empty() { DEFAULT_BASE.to_owned() } else { trimmed.to_owned() })
}

/// 解析 `artifact.chunks`；未配置时返回不填充任何伪边界的空计划。
fn resolve_chunks(value: Option<&ConfigValue>) -> AsgardResult<ChunkPlan> {
    let Some(value) = value
    else {
        return Ok(ChunkPlan::default());
    };
    let fields = as_object(value, "artifact.chunks")?;
    validate_fields(fields, "artifact.chunks", CHUNKS_FIELDS)?;

    let boundaries = match look_up(fields, "boundaries") {
        None => Vec::new(),
        Some(ConfigValue::List(items)) => {
            items.iter().enumerate().map(|(index, item)| resolve_boundary(item, index)).collect::<AsgardResult<Vec<_>>>()?
        }
        Some(other) => return Err(config_error("artifact.chunks.boundaries", format!("期望列表，实际为 {}", describe(other)))),
    };
    let budgets = match look_up(fields, "budgets") {
        None => ChunkBudgets::default(),
        Some(value) => resolve_budgets(value)?,
    };
    Ok(ChunkPlan { boundaries, budgets })
}

/// 解析单条切分边界。
fn resolve_boundary(value: &ConfigValue, index: usize) -> AsgardResult<ChunkBoundary> {
    let path = format!("artifact.chunks.boundaries[{index}]");
    let fields = as_object(value, &path)?;
    validate_fields(fields, &path, BOUNDARY_FIELDS)?;

    let component_value = look_up(fields, "component").ok_or_else(|| config_error(&path, "缺少必需字段 `component`"))?;
    let component = expect_non_empty_text(component_value, &field_path(&path, "component"))?.to_owned();
    let preload = match look_up(fields, "preload") {
        Some(value) => expect_bool(value, &field_path(&path, "preload"))?,
        None => false,
    };
    let prefetch = match look_up(fields, "prefetch") {
        Some(value) => expect_bool(value, &field_path(&path, "prefetch"))?,
        None => false,
    };
    Ok(ChunkBoundary { component, preload, prefetch })
}

/// 解析分包体积预算。
fn resolve_budgets(value: &ConfigValue) -> AsgardResult<ChunkBudgets> {
    let path = "artifact.chunks.budgets";
    let fields = as_object(value, path)?;
    validate_fields(fields, path, BUDGETS_FIELDS)?;
    let entry_bytes = match look_up(fields, "entry_bytes") {
        Some(value) => Some(expect_bytes(value, &field_path(path, "entry_bytes"))?),
        None => None,
    };
    let chunk_bytes = match look_up(fields, "chunk_bytes") {
        Some(value) => Some(expect_bytes(value, &field_path(path, "chunk_bytes"))?),
        None => None,
    };
    Ok(ChunkBudgets { entry_bytes, chunk_bytes })
}

/// 解析可选资源目录：相对项目根、必须存在且是目录。
fn resolve_assets(value: Option<&ConfigValue>, origin: &ConfigOrigin) -> AsgardResult<Option<PathBuf>> {
    let Some(value) = value
    else {
        return Ok(None);
    };
    let relative = normalize_relative(expect_non_empty_text(value, "assets")?, "assets")?;
    let path = origin.project_root.join(&relative);
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => Ok(Some(path)),
        Ok(_) => Err(config_error("assets", format!("`{}` 不是目录", path.display()))),
        Err(error) => Err(config_error("assets", format!("`{}` 不存在或不可访问：{error}", path.display()))),
    }
}

/// 解析 `build`；不在此套用 production / development 默认，保持 `Option<bool>`。
fn resolve_build(value: Option<&ConfigValue>) -> AsgardResult<BuildOptions> {
    let Some(value) = value
    else {
        return Ok(BuildOptions::default());
    };
    let fields = as_object(value, "build")?;
    validate_fields(fields, "build", BUILD_FIELDS)?;
    let minify = match look_up(fields, "minify") {
        Some(value) => Some(expect_bool(value, &field_path("build", "minify"))?),
        None => None,
    };
    let source_map = match look_up(fields, "source_map") {
        Some(value) => Some(expect_bool(value, &field_path("build", "source_map"))?),
        None => None,
    };
    Ok(BuildOptions { minify, source_map })
}

/// 解析 `serve` / `dev` 监听设置。
fn resolve_endpoint(value: Option<&ConfigValue>, path: &str) -> AsgardResult<(String, u16)> {
    let Some(value) = value
    else {
        return Ok((DEFAULT_HOST.to_owned(), DEFAULT_PORT));
    };
    let fields = as_object(value, path)?;
    validate_fields(fields, path, ENDPOINT_FIELDS)?;

    let host = match look_up(fields, "host") {
        Some(value) => expect_text(value, &field_path(path, "host"))?.to_owned(),
        None => DEFAULT_HOST.to_owned(),
    };
    let port = match look_up(fields, "port") {
        Some(value) => {
            let number = expect_int(value, &field_path(path, "port"))?;
            if !(1..=65535).contains(&number) {
                return Err(config_error(&field_path(path, "port"), format!("端口必须在 1..=65535 范围内，收到 {number}")));
            }
            u16::try_from(number).map_err(|_| config_error(&field_path(path, "port"), format!("端口超出可表示范围：{number}")))?
        }
        None => DEFAULT_PORT,
    };
    Ok((host, port))
}
