//! 测试夹具的唯一 primitive 声明前导。
//!
//! 真实构建里 `i32`/`i64` 等 primitive 身份来自 std 包
//! （`valkyrie.v/projects/std/source/primitive/*.v`）的 `[primitive]` 声明，
//! 由依赖组的语义导出注入消费组（见 `hir/lowering/mod.rs` 的
//! `HirDependencySemanticExport` 装配）。测试夹具没有 std 包，因此由本模块统一
//! 注入同一份声明；各测试文件禁止再抄写 primitive 声明文本。
//!
//! 验证「未声明 primitive 必须结构化失败」的用例显式走原始
//! [`rusty_valkyrie::ValkyrieCompiler::compile_source`]，不要经过
//! [`SourceFixtureCompile`]：这就是 fail-closed 用例的 opt-out。

use std::collections::{BTreeMap, BTreeSet};

use rusty_valkyrie::{CompilerSourceFile, CompilerSourceGroup, HirError, ValkyrieCompiler, types::hir::HirModule};

/// 与 std 包一致的 primitive 声明集合；测试侧唯一声明来源。
pub(crate) const PRIMITIVES: &str = include_str!("primitives.v");

/// 从一行声明里取出被声明的短名（`structure` 或 `type` 别名）。
fn declaration_name(line: &str) -> Option<&str> {
    if let Some(rest) = line.trim_start().strip_prefix("type ") {
        return rest.split('=').next().map(str::trim).filter(|name| !name.is_empty());
    }
    let rest = line.split("structure ").nth(1)?;
    Some(rest.split_whitespace().next()?.trim_matches('`'))
}

/// 一段源码里已经声明过的拼写（`structure` 名与 `type` 别名）。
fn declared_names(source: &str) -> BTreeSet<String> {
    source.lines().filter_map(declaration_name).map(str::to_owned).collect()
}

/// 源码里按标识符边界出现的 token。
fn source_tokens(source: &str) -> BTreeSet<String> {
    let mut tokens = BTreeSet::new();
    let mut current = String::new();
    for ch in source.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            tokens.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.insert(current);
    }
    tokens
}

/// `type` 别名到目标的映射（`usize = u64`、`isize = i64`）。
fn alias_targets() -> BTreeMap<String, String> {
    PRIMITIVES
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix("type ")?;
            let (name, target) = rest.split_once('=')?;
            Some((name.trim().to_owned(), target.trim().trim_end_matches(';').trim().to_owned()))
        })
        .collect()
}

/// 夹具实际引用到的 primitive 拼写，含别名展开后的目标（别名目标同样必须已声明）。
///
/// 只注入用到的拼写：把整份前导塞进每个夹具会给入口模块引入夹具没提过的声明
/// （例如无 carrier 的 `f128` 会作为完整名义聚合进入 canonical，污染实例计数）。
fn referenced_spellings(source: &str) -> BTreeSet<String> {
    let targets = alias_targets();
    let mut wanted: BTreeSet<String> = source_tokens(source).into_iter().filter(|token| primitive_spellings().contains(token)).collect();
    let mut pending: Vec<String> = wanted.iter().cloned().collect();
    while let Some(name) = pending.pop() {
        if let Some(target) = targets.get(&name) {
            if wanted.insert(target.clone()) {
                pending.push(target.clone());
            }
        }
    }
    wanted
}

/// 前导覆盖的全部 primitive 拼写。
fn primitive_spellings() -> BTreeSet<String> {
    declared_names(PRIMITIVES)
}

/// 只为缺失的拼写生成声明前导；全部已可用时返回空串。
///
/// 声明必须落在各自身份的规范位置（`std::primitive::*` 在 `namespace std.primitive;`、
/// `std::text::char` 在 `namespace std.text;`），闭包身份来源门才认得出可验证来源；
/// 前导按块跟踪当前 namespace，切换时输出对应的 `namespace` 头。
fn prelude_for(missing: &BTreeSet<String>) -> String {
    let mut pieces: Vec<(&str, &str)> = Vec::new();
    let mut active_namespace = "std.primitive";
    for block in PRIMITIVES.split("\n\n") {
        if let Some(header) = block.lines().next().and_then(|line| line.trim_start().strip_prefix("namespace ")) {
            active_namespace = header.trim().trim_end_matches(';').trim();
            // 纯 namespace 头块：只切换位置，不产出声明。
            if block.lines().filter_map(declaration_name).next().is_none() {
                continue;
            }
        }
        let names: Vec<&str> = block.lines().filter_map(declaration_name).collect();
        if names.is_empty() {
            continue;
        }
        if names.len() == block.lines().count() {
            // 每行一条 `type` 别名，按行补齐。
            pieces.extend(block.lines().filter(|line| declaration_name(line).is_some_and(|name| missing.contains(name))).map(|line| (active_namespace, line)));
        }
        else if missing.contains(names[0]) {
            pieces.push((active_namespace, block));
        }
    }
    if pieces.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut emitted_namespace: Option<&str> = None;
    for (namespace, piece) in pieces {
        if emitted_namespace != Some(namespace) {
            out.push_str(&format!("namespace {namespace};\n\n"));
            emitted_namespace = Some(namespace);
        }
        out.push_str(piece);
        out.push_str("\n\n");
    }
    out
}

/// 一组源文件里已经声明过的拼写。
fn declared_names_sources(sources: &[CompilerSourceFile]) -> BTreeSet<String> {
    sources.iter().flat_map(|file| declared_names(&file.source)).collect()
}

/// 一组源文件里引用到的 primitive 拼写。
fn referenced_spellings_sources(sources: &[CompilerSourceFile]) -> BTreeSet<String> {
    sources.iter().flat_map(|file| referenced_spellings(&file.source)).collect()
}

/// 只补夹具引用到、且尚未声明的 primitive 声明；无需补时返回空串。
pub(crate) fn primitive_prelude(existing: &str) -> String {
    let declared = declared_names(existing);
    let missing: BTreeSet<String> = referenced_spellings(existing).into_iter().filter(|name| !declared.contains(name)).collect();
    prelude_for(&missing)
}

/// 单个源文件夹具：在末尾追加缺失的 primitive 声明。
///
/// 追加而不是前置，避免首个 `namespace` 之前的内容改变夹具原有命名空间归属。
pub(crate) fn with_primitives(source: &str) -> String {
    let prelude = primitive_prelude(source);
    if prelude.is_empty() { source.to_string() } else { format!("{source}\n{prelude}") }
}

/// 多组夹具：只给拿不到 primitive 声明的组补它缺失的拼写（作为独立源文件）。
///
/// 消费组通过依赖组的语义导出拿到 primitive 声明；若再给消费组注入同名声明，
/// 同一可见面会出现两个同名候选（`nominal `i64` is ambiguous`）。因此只有
/// 直接依赖不落在本次输入里的组才补——它没有可继承的声明来源。
///
/// 同一依赖组内重复登记同一身份是结构化错误；跨依赖组携带同一身份时，每个携带者的
/// `::` 身份引用必须在自身可见面内按绝对限定位置解析出来源声明：来源恰为自身的是
/// 规范 owner，来源为 owner 的是别名载体（同一声明的投影）。来源落空、位置歧义或
/// 并列自命中（独立 owner 冒领）都结构化拒绝（见 `closure/primitive_identity.rs`），
/// 因此注入的声明必须落在身份的规范 namespace，消费组必须能看到规范声明的依赖组。
pub(crate) fn groups_with_primitives(mut groups: Vec<CompilerSourceGroup>) -> Vec<CompilerSourceGroup> {
    // 每组注入后实际持有的拼写：本地声明 ∪ 本地引用（引用缺失的会被注入补齐）。
    let declared: BTreeMap<String, BTreeSet<String>> = groups
        .iter()
        .map(|group| {
            let mut names = declared_names_sources(&group.sources);
            names.extend(referenced_spellings_sources(&group.sources));
            (group.dependency_key.clone(), names)
        })
        .collect();
    let dependencies: BTreeMap<String, Vec<String>> =
        groups.iter().map(|group| (group.dependency_key.clone(), group.direct_dependencies.clone())).collect();
    let own: BTreeMap<String, BTreeSet<String>> =
        groups.iter().map(|group| (group.dependency_key.clone(), declared_names_sources(&group.sources))).collect();
    for group in &mut groups {
        // 可见面 = 本地声明 ∪ 依赖组（含传递）会持有的声明。
        let mut available = own.get(&group.dependency_key).cloned().unwrap_or_default();
        let mut memo = BTreeMap::new();
        let mut visiting = BTreeSet::new();
        for dependency in &group.direct_dependencies {
            available.extend(reachable_declared(dependency, &dependencies, &declared, &mut memo, &mut visiting));
        }
        let referenced = referenced_spellings_sources(&group.sources);
        let missing: BTreeSet<String> = referenced.into_iter().filter(|name| !available.contains(name)).collect();
        let prelude = prelude_for(&missing);
        if prelude.is_empty() {
            continue;
        }
        group.sources.push(CompilerSourceFile { path: "test_primitives.v".into(), source: prelude });
    }
    groups
}

/// 依赖组在自身补声明后对消费组可见的拼写集合（按依赖传递，带环保护）。
fn reachable_declared(
    key: &str,
    dependencies: &BTreeMap<String, Vec<String>>,
    declared: &BTreeMap<String, BTreeSet<String>>,
    memo: &mut BTreeMap<String, BTreeSet<String>>,
    visiting: &mut BTreeSet<String>,
) -> BTreeSet<String> {
    if let Some(cached) = memo.get(key) {
        return cached.clone();
    }
    let Some(own) = declared.get(key) else {
        // 依赖不在本次输入内：无法保证它补，按「未覆盖」处理。
        return BTreeSet::new();
    };
    if !visiting.insert(key.to_string()) {
        return own.clone();
    }
    let mut visible = own.clone();
    for dependency in dependencies.get(key).into_iter().flatten() {
        visible.extend(reachable_declared(dependency, dependencies, declared, memo, visiting));
    }
    visiting.remove(key);
    memo.insert(key.to_string(), visible.clone());
    visible
}

/// 夹具源码编译入口：普通用例都从这里进入，自动带上共享 primitive 前导。
pub(crate) trait SourceFixtureCompile {
    fn compile_fixture_source(&self, source: &str) -> Result<HirModule, HirError>;
}

impl SourceFixtureCompile for ValkyrieCompiler {
    fn compile_fixture_source(&self, source: &str) -> Result<HirModule, HirError> {
        self.compile_source(&with_primitives(source))
    }
}
