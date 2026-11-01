//! `swap_generation` 整代替换的定向语义：成功路径换入新一代并清理上一代，
//! 第二次重命名失败时回滚上一代，保证任何失败路径都可继续服务旧页面。

use std::fs;

use rusty_valkyrie::asgard::cli::swap_generation;
use tempfile::TempDir;

/// 在目录里写入一个标记页面，返回目录路径。
fn write_generation(root: &std::path::Path, name: &str, marker: &str) -> std::path::PathBuf {
    let directory = root.join(name);
    fs::create_dir_all(&directory).expect("创建产物代目录失败");
    fs::write(directory.join("index.html"), format!("<!DOCTYPE html><title>{marker}</title>")).expect("写入标记页面失败");
    directory
}

/// 读取目录内标记页面全文。
fn read_marker(directory: &std::path::Path) -> String {
    fs::read_to_string(directory.join("index.html")).expect("标记页面应可读")
}

#[test]
fn swap_replaces_the_current_generation_and_cleans_up() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let output = write_generation(temp.path(), "dist", "previous page");
    let staging = write_generation(temp.path(), "dist.staging", "next page");

    swap_generation(&staging, &output).expect("整代替换应成功");

    assert!(read_marker(&output).contains("next page"), "新一代必须落到产物目录");
    assert!(!staging.exists(), "staging 不得残留");
    assert!(!temp.path().join("dist.previous").exists(), "上一代必须在成功后清理");
}

#[test]
fn swap_without_previous_generation_installs_staging() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let output = temp.path().join("dist");
    let staging = write_generation(temp.path(), "dist.staging", "first page");

    swap_generation(&staging, &output).expect("无上一代时替换应直接成功");

    assert!(read_marker(&output).contains("first page"), "首代产物必须就位");
    assert!(!staging.exists(), "staging 不得残留");
}

#[test]
fn swap_failure_rolls_back_the_previous_generation() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let output = write_generation(temp.path(), "dist", "previous page");
    // staging 缺失使第二次重命名确定性失败：回滚必须把上一代搬回原位。
    let staging = temp.path().join("dist.staging");

    let error = swap_generation(&staging, &output).expect_err("staging 缺失必须失败");
    assert!(error.to_string().contains("替换产物失败"), "诊断应标明失败环节: {error}");

    assert!(read_marker(&output).contains("previous page"), "回滚后旧页面必须仍在原位可服务");
    assert!(!temp.path().join("dist.previous").exists(), "回滚完成后不得遗留 .previous 目录");
}
