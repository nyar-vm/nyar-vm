use nyar_package_manager::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Neutral defaults that must always come from [`ProjectLayout`], never be hard-coded in `src`.
///
/// `layout.rs` is the only place allowed to spell them out.
fn neutral_layout_literals() -> Vec<String> {
    let layout = ProjectLayout::neutral();
    vec![
        layout.package_manifest.to_string(),
        layout.workspace_manifest.to_string(),
        layout.ignore_file.to_string(),
        layout.lockfile.to_string(),
        layout.home_dirname.to_string(),
        layout.home_env.to_string(),
    ]
}

/// Ecosystem package-manager CLIs must not be spawned from the neutral core.
/// (`cmd` / `sh` in ScriptRunner and `git` in publish helpers are allowed.)
const FORBIDDEN_SHELL_TOOLS: &[&str] = &[
    "npm", "pnpm", "yarn", "bun", "npx", "vite", "biome", "eslint", "prettier", "tsc", "webpack", "esbuild", "uv", "pip", "pip3", "poetry",
    "conda", "mamba", "ruff", "black", "flake8", "mypy", "pytest", "hatch", "pdm", "pipenv",
];

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if !dir.is_dir() {
        return;
    }
    for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display())) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        }
        else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn strip_rust_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut in_block = false;
    while i < bytes.len() {
        if in_block {
            if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                in_block = false;
                i += 2;
                continue;
            }
            out.push(' ');
            i += 1;
            continue;
        }
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            in_block = true;
            out.push(' ');
            out.push(' ');
            i += 2;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn scan_source(path: &Path) -> String {
    let raw = fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    strip_rust_comments(&raw)
}

#[test]
fn neutral_layout_uses_nyar_defaults() {
    let layout = ProjectLayout::neutral();
    assert_eq!(layout.package_manifest, "package.von");
    assert_eq!(layout.workspace_manifest, "workspace.von");
    assert_eq!(layout.ignore_file, ".packageignore");
    assert_eq!(layout.lockfile, "package-lock.von");
    assert_eq!(layout.home_dirname, ".nyar");
    assert_eq!(layout.home_env, "NYAR_HOME");
    assert!(layout.token_env_vars.is_empty());
}

#[test]
fn package_manager_src_does_not_hardcode_layout_defaults() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&root, &mut files);
    assert!(!files.is_empty());
    let needles = neutral_layout_literals();
    for path in files {
        if path.file_name().and_then(|name| name.to_str()) == Some("layout.rs") {
            continue;
        }
        let source = scan_source(&path);
        for token in &needles {
            let quoted = format!("\"{token}\"");
            assert!(!source.contains(&quoted), "{} must not hard-code layout identity {quoted}; read it from `ProjectLayout`", path.display());
        }
    }
}

#[test]
fn package_manager_src_does_not_shell_foreign_toolchains() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&root, &mut files);
    for path in files {
        let source = scan_source(&path);
        for tool in FORBIDDEN_SHELL_TOOLS {
            for quote in ['"', '\''] {
                let needle = format!("Command::new({quote}{tool}{quote})");
                assert!(
                    !source.contains(&needle),
                    "{} must not spawn foreign toolchain via {needle} (ScriptRunner may use cmd/sh; products own ecosystem CLIs)",
                    path.display()
                );
            }
        }
    }
}
