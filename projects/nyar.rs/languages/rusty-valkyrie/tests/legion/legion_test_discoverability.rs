//! 测试可发现性守卫。
//!
//! `tests/legion/cache/` 曾整目录未接线，长期不参与 `cargo test` 也无人发现——「没接线的测试 = 永远绿的测试」。
//! 本守卫遍历 `tests/**/*.rs`，要求每个文件都能从 `Cargo.toml` 声明的 harness `[[test]]` 根经 `mod` 声明到达。

use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

/// 属性文本前缀用片段拼接，避免守卫源自身命中扫描。
const IGNORE_ATTRIBUTE_PREFIX: &str = concat!("#", "[ignore");
const CFG_ATTRIBUTE_PREFIX: &str = concat!("#", "[cfg");

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 去掉行尾注释，避免把 `// mod foo;` 这类说明当成接线声明。
fn uncommented(line: &str) -> &str {
    match line.find("//") {
        Some(index) => &line[..index],
        None => line,
    }
}

/// 提取 `#[path = "..."]` 中引号内的路径。
fn path_attribute(line: &str) -> Option<&str> {
    let rest = &line[line.find("#[path")?..];
    let rest = &rest[rest.find('"')? + 1..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// 提取 `mod NAME;` 或 `mod NAME {`，返回 `(名称, 是否为 inline 模块)`。
fn module_declaration(line: &str) -> Option<(&str, bool)> {
    let bytes = line.as_bytes();
    let mut search = 0;
    while let Some(offset) = line[search..].find("mod ") {
        let index = search + offset;
        let boundary_ok = index == 0 || bytes[index - 1].is_ascii_whitespace();
        if !boundary_ok {
            search = index + 4;
            continue;
        }
        let rest = line[index + 4..].trim_start();
        let ident_len = rest.find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')).unwrap_or(rest.len());
        if ident_len == 0 {
            return None;
        }
        let name = &rest[..ident_len];
        match rest[ident_len..].trim_start().chars().next() {
            Some(';') => return Some((name, false)),
            Some('{') => return Some((name, true)),
            _ => return None,
        }
    }
    None
}

/// 解析 `Cargo.toml` 的显式 `[[test]] path`，得到 harness 根。
fn harness_roots() -> Vec<PathBuf> {
    let dir = crate_dir();
    let manifest = fs::read_to_string(dir.join("Cargo.toml")).expect("读取 crate Cargo.toml");
    assert!(
        manifest.lines().any(|line| line.trim() == "autotests = false"),
        "测试树守卫依赖 `autotests = false` + 显式 `[[test]]` 接线；改变自动发现会绕过本守卫"
    );

    let mut roots = Vec::new();
    let mut in_test_section = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_test_section = line == "[[test]]";
            continue;
        }
        if !in_test_section {
            continue;
        }
        if let Some(rest) = line.strip_prefix("path") {
            if let Some(rest) = rest.trim_start().strip_prefix('=') {
                let value = rest.trim().trim_end_matches(',').trim_matches('"');
                roots.push(dir.join(value));
            }
        }
    }
    assert!(!roots.is_empty(), "Cargo.toml 必须显式声明 `[[test]] path`");
    for root in &roots {
        assert!(root.is_file(), "`[[test]]` 根 {} 不存在", root.display());
    }
    roots
}

/// 模块的子模块搜索目录：harness 根与 `mod.rs` 用所在目录，其它文件用同名子目录。
fn child_directory(file: &Path, is_harness_root: bool) -> PathBuf {
    let parent = file.parent().expect("模块文件必须有父目录").to_path_buf();
    if is_harness_root {
        return parent;
    }
    match file.file_stem().and_then(|stem| stem.to_str()) {
        Some("mod") => parent,
        Some(stem) => parent.join(stem),
        None => parent,
    }
}

fn collect_reachable(roots: &[PathBuf]) -> Result<BTreeSet<PathBuf>, String> {
    let mut reachable = BTreeSet::new();
    let mut queue: VecDeque<(PathBuf, PathBuf)> = VecDeque::new();
    for root in roots {
        let parent = root.parent().expect("harness 根必须有父目录").to_path_buf();
        queue.push_back((root.clone(), parent));
    }

    while let Some((file, child_dir)) = queue.pop_front() {
        if !reachable.insert(file.clone()) {
            continue;
        }
        let source = fs::read_to_string(&file).map_err(|error| format!("读取 {} 失败：{error}", file.display()))?;
        let mut pending_path: Option<String> = None;
        for (index, raw) in source.lines().enumerate() {
            let line = uncommented(raw);
            if let Some(path) = path_attribute(line) {
                pending_path = Some(path.to_string());
                continue;
            }
            let Some((name, inline)) = module_declaration(line) else {
                continue;
            };
            if inline {
                pending_path = None;
                continue;
            }
            // `#[path]` 相对当前文件所在目录；裸 `mod` 相对模块子目录。
            let target = match pending_path.take() {
                Some(path) => file.parent().expect("模块文件必须有父目录").join(path),
                None => {
                    let flat = child_dir.join(format!("{name}.rs"));
                    let nested = child_dir.join(name).join("mod.rs");
                    if flat.is_file() {
                        flat
                    } else if nested.is_file() {
                        nested
                    } else {
                        return Err(format!(
                            "{}:{}: `mod {name};` 在 {} 下找不到 `{name}.rs` 或 `{name}/mod.rs`",
                            file.display(),
                            index + 1,
                            child_dir.display()
                        ));
                    }
                }
            };
            if !target.is_file() {
                return Err(format!("{}:{}: 声明指向的 {} 不存在", file.display(), index + 1, target.display()));
            }
            queue.push_back((target.clone(), child_directory(&target, false)));
        }
    }
    Ok(reachable)
}

fn collect_test_sources(directory: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory).unwrap_or_else(|error| panic!("读取 {} 失败：{error}", directory.display()));
    for entry in entries {
        let path = entry.expect("读取测试目录项").path();
        if path.is_dir() {
            collect_test_sources(&path, out);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}


#[test]
fn every_test_source_is_wired_into_a_harness() {
    let tests_dir = crate_dir().join("tests");
    let roots = harness_roots();
    let reachable = collect_reachable(&roots).unwrap_or_else(|error| panic!("{error}"));

    let mut sources = Vec::new();
    collect_test_sources(&tests_dir, &mut sources);
    assert!(sources.len() > 100, "守卫应遍历整个 `tests` 树，实际只看到 {} 个文件", sources.len());

    let mut unreachable: Vec<String> = sources
        .iter()
        .filter(|path| !reachable.contains(*path))
        .map(|path| path.strip_prefix(&tests_dir).unwrap_or(path).display().to_string())
        .collect();
    unreachable.sort();

    assert!(
        unreachable.is_empty(),
        "以下 `tests/**/*.rs` 没有任何 harness `mod` 声明可达（未接线 = 永远不运行）：\n  {}\n共 {} 个；请接线，或按 `pending-tests/readme.md` 隔离",
        unreachable.join("\n  "),
        unreachable.len()
    );
}


/// 递归收集 crate 内 `tests/` 与 `src/` 之外含测试声明的 `.rs`。
fn collect_stray_test_sources(current: &Path, tests_dir: &Path, out: &mut Vec<String>) {
    let entries = fs::read_dir(current).unwrap_or_else(|error| panic!("读取 {} 失败：{error}", current.display()));
    for entry in entries {
        let path = entry.expect("读取目录项").path();
        if path.is_dir() {
            if path == tests_dir || path.file_name().and_then(|name| name.to_str()) == Some("src") {
                continue;
            }
            if path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with('.')) {
                continue;
            }
            collect_stray_test_sources(&path, tests_dir, out);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            let source = fs::read_to_string(&path).expect("读取 Rust 源");
            let is_test_source = source.lines().any(|line| {
                let line = uncommented(line);
                line.contains("#[test") || line.contains("cfg(test)") || line.contains("cfg_attr")
            });
            if is_test_source {
                out.push(path.display().to_string());
            }
        }
    }
}

#[test]
fn no_test_sources_outside_tests() {
    let dir = crate_dir();
    assert!(
        !dir.join("pending-tests").exists(),
        "不允许把测试源挪出 `tests/` 伪造接线状态；`pending-tests/` 必须不存在"
    );

    let tests_dir = dir.join("tests");
    let mut stray = Vec::new();
    collect_stray_test_sources(&dir, &tests_dir, &mut stray);
    stray.sort();
    assert!(
        stray.is_empty(),
        "以下 `tests/` 之外（且不在 `src/`）的 `.rs` 含测试声明，无法被任何 harness 执行（未接线 = 永远不运行）：\n  {}\n请迁入 `tests/` 并接线，或删除",
        stray.join("\n  ")
    );
}

#[test]
fn discoverability_guard_is_not_gated() {
    let source = include_str!("legion_test_discoverability.rs");
    for (index, raw) in source.lines().enumerate() {
        let line = uncommented(raw);
        assert!(!line.contains(IGNORE_ATTRIBUTE_PREFIX), "守卫自身不得被 ignore 属性遮蔽（第 {} 行）", index + 1);
        assert!(!line.contains(CFG_ATTRIBUTE_PREFIX), "守卫自身不得被条件编译特性门遮蔽（第 {} 行）", index + 1);
    }
}
