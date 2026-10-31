//! `asgard.config.v` 发现的回归测试：显式优先、最近祖先命中、不搜子目录。

use std::{
    fs,
    path::{Path, PathBuf},
};

use rusty_valkyrie::asgard::config_discovery::{ASGARD_CONFIG_FILENAME, ConfigDiscoveryRequest, ConfigLocation, discover_asgard_config};
use tempfile::TempDir;

fn write_config(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("创建配置目录失败");
    let path = dir.join(ASGARD_CONFIG_FILENAME);
    fs::write(&path, "{\n    artifact: { target: \"static\" }\n}\n").expect("写入配置文件失败");
    path
}

fn search(start_dir: &Path, explicit_config: Option<PathBuf>) -> Result<ConfigLocation, String> {
    discover_asgard_config(ConfigDiscoveryRequest { start_dir: start_dir.to_path_buf(), explicit_config }).map_err(|error| error.to_string())
}

#[test]
fn missing_config_lists_checked_paths() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let start = temp.path().join("project");
    fs::create_dir_all(&start).expect("创建起始目录失败");

    let message = search(&start, None).expect_err("缺少配置应报错");
    assert!(message.contains(&start.display().to_string()), "诊断应列出已检查的起始目录: {message}");
    assert!(message.contains(ASGARD_CONFIG_FILENAME), "诊断应指明配置文件名: {message}");
    assert!(message.contains("已检查"), "诊断应说明检查过哪些位置: {message}");
}

#[test]
fn nearest_ancestor_config_wins() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let root = temp.path().join("app");
    let config = write_config(&root);
    let start = root.join("source").join("pages");
    fs::create_dir_all(&start).expect("创建起始目录失败");

    let location = search(&start, None).expect("应向上找到最近的配置");
    assert_eq!(location.config_path, config, "应选中中间层的配置文件");
    assert_eq!(location.project_root, root, "项目根应为配置文件所在目录");
}

#[test]
fn explicit_config_takes_priority() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let start = temp.path().join("app");
    let default_config = write_config(&start);
    let explicit_root = temp.path().join("elsewhere");
    let explicit_config = write_config(&explicit_root);

    let location = search(&start, Some(explicit_config.clone())).expect("显式路径应被采用");
    assert_eq!(location.config_path, explicit_config, "显式配置应优先于向上搜索");
    assert_eq!(location.project_root, explicit_root, "项目根应为显式配置所在目录");
    assert_ne!(location.config_path, default_config, "不应选回向上搜索到的配置");
}

#[test]
fn missing_explicit_config_fails_without_search() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let start = temp.path().join("app");
    write_config(&start);
    let missing = temp.path().join("nope").join(ASGARD_CONFIG_FILENAME);

    let message = search(&start, Some(missing.clone())).expect_err("显式缺失路径应报错");
    assert!(message.contains(&missing.display().to_string()), "诊断应指明显式路径: {message}");
}

#[test]
fn child_directory_config_is_not_searched() {
    let temp = TempDir::new().expect("创建临时目录失败");
    let start = temp.path().join("app");
    fs::create_dir_all(&start).expect("创建起始目录失败");
    write_config(&start.join("child"));

    let message = search(&start, None).expect_err("不应向下搜索子目录");
    assert!(message.contains(&start.display().to_string()), "诊断应列出起始目录: {message}");
}
