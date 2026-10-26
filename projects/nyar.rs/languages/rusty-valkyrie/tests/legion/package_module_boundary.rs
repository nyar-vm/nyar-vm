//! 单包模块方向守卫。
//!
//! `rusty-valkyrie` 是一个 Cargo package，但模块仍必须保持单向事实所有权：
//! 前端本体（frontend / hir / mir / semantic_closure / type_checker / …）不得导入
//! 产品层（`asgard` / `legion` / `workspace`）。包是构建边界，不是“模块可以互指”的许可。
//!
//! 同时冻结“只有一个 package”这一事实：包根之外不得出现兄弟 Cargo package。

use std::{
    fs,
    path::{Path, PathBuf},
};

fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).canonicalize().expect("package root")
}

/// 产品层模块目录（相对 `src/`），允许引用前端本体。
const PRODUCT_MODULES: &[&str] = &["asgard", "legion", "bin"];

/// 产品层标识符：前端模块中不得出现这些路径前缀。
const PRODUCT_PATHS: &[&str] =
    &["crate::asgard", "crate::legion", "crate::legion::workspace", "rusty_valkyrie::asgard", "rusty_valkyrie::legion"];

fn is_product_module(rel: &Path) -> bool {
    let mut components = rel.components();
    // 路径相对包根，首段固定为 `src`。
    if components.next().map(|component| component.as_os_str().to_owned()) != Some("src".into()) {
        return false;
    }
    match components.next() {
        Some(std::path::Component::Normal(first)) => {
            let first = first.to_string_lossy();
            PRODUCT_MODULES.iter().any(|module| *module == first)
        }
        _ => false,
    }
}

fn walk_rs(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir)
    else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs(&path, root, out);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let rel = path.strip_prefix(root).expect("path under package root").to_path_buf();
        out.push(rel);
    }
}

#[test]
fn frontend_modules_do_not_import_product_modules() {
    let root = package_root();
    let src = root.join("src");
    let mut files = Vec::new();
    walk_rs(&src, &root, &mut files);

    let mut violations = Vec::new();
    for rel in files {
        if is_product_module(&rel) {
            continue;
        }
        let Ok(source) = fs::read_to_string(root.join(&rel))
        else {
            continue;
        };
        for line in source.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            for needle in PRODUCT_PATHS {
                if line.contains(needle) {
                    violations.push(format!("{}: {}", rel.to_string_lossy().replace('\\', "/"), line.trim()));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "前端模块不得依赖产品层（asgard / legion / workspace）。请把该依赖下沉到产品模块或装配边界：\n{}",
        violations.join("\n")
    );
}

#[test]
fn package_has_no_sibling_cargo_packages() {
    let root = package_root();
    let mut nested = Vec::new();
    collect_nested_manifests(&root, &root, &mut nested);
    assert!(nested.is_empty(), "`rusty-valkyrie` 是单一 Cargo package，包内不得再出现兄弟 package：\n{}", nested.join("\n"));
}

fn collect_nested_manifests(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir)
    else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "target" || name == "node_modules" {
                continue;
            }
            collect_nested_manifests(&path, root, out);
            continue;
        }
        if path.file_name().and_then(|s| s.to_str()) != Some("Cargo.toml") {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if rel == Path::new("Cargo.toml") {
            continue;
        }
        out.push(rel.to_string_lossy().replace('\\', "/"));
    }
}
