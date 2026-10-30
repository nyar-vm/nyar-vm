//! 项目源码发现。
//!
//! `.w` 是 `.awsl` 的缩写：同格式、同语义、同工具链，后缀只影响文件发现。
//! 同一父目录下出现同名不同后缀（`.w` 与 `.awsl`）时不建立覆盖关系，
//! 该身份碰撞作为错误诊断输出。
use crate::asgard::error::{AsgardError, AsgardResult};
use std::path::{Path, PathBuf};
/// 发现的 AWSL 组件文件。
#[derive(Debug, Clone)]
pub struct DiscoveredAwslFile {
    /// 绝对路径。
    pub path: PathBuf,
    /// 路由/组件名（来自文件名）。
    pub component_name: String,
    /// 相对 source 目录的路径。
    pub relative_path: String,
}

/// 发现的 AWS（Tailwind）样式入口。
#[derive(Debug, Clone)]
pub struct DiscoveredAwsFile {
    /// 绝对路径。
    pub path: PathBuf,
    /// 相对 source 目录的路径。
    pub relative_path: String,
}

/// 项目源码扫描结果。
#[derive(Debug, Clone)]
pub struct DiscoveredSources {
    /// AWSL 文件。
    pub awsl_files: Vec<DiscoveredAwslFile>,
    /// AWS Tailwind 入口文件。
    pub aws_files: Vec<DiscoveredAwsFile>,
}

/// 扫描项目 `source/` 目录。
pub fn discover_sources(project_dir: &Path) -> AsgardResult<DiscoveredSources> {
    let source_dir = project_dir.join("source");
    if !source_dir.exists() {
        return Err(AsgardError::msg(report!("未找到 source/ 目录: {}", source_dir.display())));
    }

    let mut awsl_files = Vec::new();
    let mut aws_files = Vec::new();
    collect_files(&source_dir, &source_dir, &mut awsl_files, &mut aws_files)?;
    awsl_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    aws_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    check_awsl_identity_collisions(&awsl_files)?;
    Ok(DiscoveredSources { awsl_files, aws_files })
}

/// 检查 AWSL 源码身份碰撞：同一父目录下同名不同后缀（`.w` 与 `.awsl`）。
///
/// 两者语义相同，不建立任何覆盖或择优关系；发现即报错，由用户消除歧义。
fn check_awsl_identity_collisions(awsl_files: &[DiscoveredAwslFile]) -> AsgardResult<()> {
    let mut seen: std::collections::HashMap<(Option<&Path>, &str), &DiscoveredAwslFile> = std::collections::HashMap::new();
    for file in awsl_files {
        let key = (file.path.parent(), file.component_name.as_str());
        if let Some(prev) = seen.insert(key, file) {
            return Err(AsgardError::msg(report!(
                "AWSL 源码身份碰撞: {} 与 {} 在同一目录下同名不同后缀（.w 与 .awsl 同语义），不建立覆盖关系",
                prev.relative_path,
                file.relative_path
            )));
        }
    }
    Ok(())
}

fn collect_files(
    root: &Path,
    current: &Path,
    awsl_files: &mut Vec<DiscoveredAwslFile>,
    aws_files: &mut Vec<DiscoveredAwsFile>,
) -> AsgardResult<()> {
    for entry in std::fs::read_dir(current).map_err(|e| AsgardError::msg(format!("读取目录失败: {}: {e}", current.display())))? {
        let entry = entry.map_err(|e| AsgardError::msg(format!("{e}: {}", "读取目录项失败")))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, awsl_files, aws_files)?;
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str())
        else {
            continue;
        };
        match ext {
            "awsl" | "w" => {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                let component_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("component").to_string();
                awsl_files.push(DiscoveredAwslFile { path, component_name, relative_path: relative });
            }
            "aws" => {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                aws_files.push(DiscoveredAwsFile { path, relative_path: relative });
            }
            _ => {}
        }
    }
    Ok(())
}
