//! `JVM` 后端容器入口。
//!
//! 这里按 `class / jar` 两个输出边界收口：`class` 格式模型经 [`crate::transitional`] 过渡，
//! `jar` 的写出委托 `acorn-jvm`，成员名解码由本模块的 [`JvmJarPackage`] 做平台无关归一化。

#![deny(missing_docs)]

use std::{
    io::{Cursor, Read},
    path::PathBuf,
};

use acorn_jvm::jar as acorn_jar;
use miette::{IntoDiagnostic, Result, WrapErr};
use nyar::{
    abstractions::{ArtifactFormat, BackendInputKind, BinaryArch, BinaryFlavor, BinaryTarget, TargetFamily},
    backends::{BackendDescriptor, CompilationOptions, TargetCodeGenBackend},
    packaging::{ArtifactDescriptor, ArtifactSet, TargetLane},
};

pub use crate::transitional::binary::{
    class::{
        ConstantPoolBuilder, JvmClassError, JvmClassFile, JvmCodeBody, JvmFieldSignature, JvmInstruction, JvmMethodDescriptor, JvmMethodRef,
        JvmMethodSignature, JvmTypeDescriptor, decode_instructions, encode_instructions,
    },
    jar::{JvmJarEntry, JvmJarError},
};

const MANIFEST_PATH: &str = "META-INF/MANIFEST.MF";

/// `JAR` 产物模型。
///
/// 形状与 `acorn-jvm` 的同名模型一致；写出仍委托 `acorn-jvm`，
/// 但**解码**由本模块负责：`acorn-jvm` 用 `ZipFile::enclosed_name()`（`PathBuf`）
/// 还原成员名，在 Windows 上会把 `META-INF/MANIFEST.MF` 渲染成
/// `META-INF\MANIFEST.MF`，于是清单匹配失败（`main_class` 丢失，`read_class` 也查不到类）。
/// 这里改用 `ZipFile::name()` 的原始 `ZIP` 成员名，保证跨平台一致。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JvmJarPackage {
    /// 归档文件名。
    pub file_name: String,
    /// `MANIFEST.MF` 主类。
    pub main_class: Option<String>,
    /// 归档入口列表。
    pub entries: Vec<JvmJarEntry>,
}

impl JvmJarPackage {
    /// 创建一个新的 `JAR` 产物模型。
    pub fn new(file_name: impl Into<String>) -> Self {
        Self { file_name: file_name.into(), main_class: None, entries: Vec::new() }
    }

    /// 追加一个归档入口。
    pub fn push_entry(&mut self, path: impl Into<String>, bytes: Vec<u8>) {
        self.entries.push(JvmJarEntry { path: path.into(), bytes });
    }

    /// 将 `ClassFile` 编码后放入 `JAR`。
    pub fn push_class(&mut self, class_file: &JvmClassFile) -> Result<(), JvmJarError> {
        let path = format!("{}.class", class_file.internal_name.trim_end_matches(".class"));
        self.push_entry(path, class_file.to_bytes().map_err(JvmJarError::Class)?);
        Ok(())
    }

    /// 从归档中读取指定类。
    pub fn read_class(&self, internal_name: &str) -> Result<Option<JvmClassFile>, JvmJarError> {
        let target_path = format!("{}.class", internal_name.trim_end_matches(".class"));
        let Some(entry) = self.entries.iter().find(|entry| entry.path == target_path)
        else {
            return Ok(None);
        };
        Ok(Some(JvmClassFile::from_bytes(&entry.bytes).map_err(JvmJarError::Class)?))
    }

    /// 遍历归档中所有 `.class` 入口，返回 `(路径, 字节切片)` 迭代器。
    pub fn iter_classes(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.entries.iter().filter_map(|entry| entry.path.ends_with(".class").then(|| (entry.path.as_str(), entry.bytes.as_slice())))
    }

    /// 将 `JAR` 模型编码为二进制归档。写出委托 `acorn-jvm`，保持既有产物语义不变。
    pub fn to_bytes(&self) -> Result<Vec<u8>, JvmJarError> {
        let mut package = acorn_jar::JvmJarPackage::new(self.file_name.clone());
        package.main_class = self.main_class.clone();
        for entry in &self.entries {
            package.push_entry(entry.path.clone(), entry.bytes.clone());
        }
        package.to_bytes()
    }

    /// 从二进制归档解码出 `JAR` 模型。
    pub fn from_bytes(file_name: impl Into<String>, bytes: &[u8]) -> Result<Self, JvmJarError> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(JvmJarError::Zip)?;
        let mut package = Self::new(file_name);
        for index in 0..archive.len() {
            let mut member = archive.by_index(index).map_err(JvmJarError::Zip)?;
            if member.is_dir() {
                continue;
            }
            let path = member.name().map_err(JvmJarError::Zip)?.to_string();
            let mut entry_bytes = Vec::new();
            member.read_to_end(&mut entry_bytes).map_err(JvmJarError::Io)?;
            if path.eq_ignore_ascii_case(MANIFEST_PATH) {
                package.main_class = parse_manifest_main_class(&entry_bytes);
            }
            else {
                package.push_entry(path, entry_bytes);
            }
        }
        Ok(package)
    }
}

fn parse_manifest_main_class(bytes: &[u8]) -> Option<String> {
    let manifest = String::from_utf8_lossy(bytes);
    manifest.lines().find_map(|line| line.strip_prefix("Main-Class:").map(|value| value.trim().to_string()))
}

const JVM_ACC_PUBLIC: u16 = 0x0001;
const JVM_ACC_STATIC: u16 = 0x0008;

/// `JVM` 二进制后端输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JvmBinaryBackendInput {
    /// 主类文件。
    pub class_file: JvmClassFile,
    /// 输出目录。
    pub output_dir: PathBuf,
    /// 是否同时落地裸 `class` 文件。
    pub emit_class_file: bool,
    /// Singleton holder classes emitted alongside the fragment class.
    pub companion_classes: Vec<JvmClassFile>,
}

/// `JVM` 二进制后端。
pub struct JvmBinaryBackend {
    descriptor: BackendDescriptor,
}

impl JvmBinaryBackend {
    /// 创建一个新的 `JVM` 二进制后端。
    pub fn new() -> Self {
        Self {
            descriptor: BackendDescriptor {
                name: "jvm-jar".to_string(),
                input_kind: BackendInputKind::JvmClassFile,
                supported_targets: vec![BinaryTarget::new(TargetFamily::Jvm, BinaryArch::Any, BinaryFlavor::ManagedClr)],
            },
        }
    }
}

impl Default for JvmBinaryBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl TargetCodeGenBackend for JvmBinaryBackend {
    type Input = JvmBinaryBackendInput;

    fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }

    fn validate(&self, input: &Self::Input) -> Result<()> {
        if input.class_file.internal_name.trim().is_empty() {
            return Err(miette::miette!("JVM 后端需要有效的主类内部名"));
        }
        if input.class_file.methods.is_empty() {
            return Err(miette::miette!("JVM 后端至少需要一个方法"));
        }
        Ok(())
    }

    fn compile(&self, input: Self::Input, options: &CompilationOptions) -> Result<ArtifactSet> {
        std::fs::create_dir_all(&input.output_dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("创建输出目录失败：{}", input.output_dir.display()))?;

        let mut class_file = input.class_file;
        ensure_java_launcher_entry(&mut class_file);

        let class_bytes = class_file.to_bytes().map_err(|error| miette::miette!("JVM class 编码失败：{error}"))?;

        if input.emit_class_file {
            let relative = format!("{}.class", class_file.internal_name.trim_end_matches(".class"));
            let class_path = input.output_dir.join(&relative);
            if let Some(parent) = class_path.parent() {
                std::fs::create_dir_all(parent).into_diagnostic().wrap_err_with(|| format!("创建 class 输出目录失败：{}", parent.display()))?;
            }
            std::fs::write(&class_path, &class_bytes)
                .into_diagnostic()
                .wrap_err_with(|| format!("写入 class 文件失败：{}", class_path.display()))?;
            for companion in &input.companion_classes {
                let companion_relative = format!("{}.class", companion.internal_name.trim_end_matches(".class"));
                let companion_path = input.output_dir.join(&companion_relative);
                if let Some(parent) = companion_path.parent() {
                    std::fs::create_dir_all(parent)
                        .into_diagnostic()
                        .wrap_err_with(|| format!("创建 companion class 输出目录失败：{}", parent.display()))?;
                }
                let companion_bytes = companion.to_bytes().map_err(|error| miette::miette!("JVM companion class 编码失败：{error}"))?;
                std::fs::write(&companion_path, companion_bytes)
                    .into_diagnostic()
                    .wrap_err_with(|| format!("写入 companion class 失败：{}", companion_path.display()))?;
            }
        }

        let jar_path = input.output_dir.join(format!("{}.jar", options.artifact_name));
        let mut package = JvmJarPackage::new(format!("{}.jar", options.artifact_name));
        package.main_class = Some(internal_name_to_binary_name(&class_file.internal_name));
        package.push_class(&class_file).map_err(|error| miette::miette!("JAR 打包失败：{error}"))?;
        for companion in &input.companion_classes {
            package.push_class(companion).map_err(|error| miette::miette!("JAR 打包 singleton 类失败：{error}"))?;
        }
        let jar_bytes = package.to_bytes().map_err(|error| miette::miette!("JAR 编码失败：{error}"))?;
        std::fs::write(&jar_path, jar_bytes).into_diagnostic().wrap_err_with(|| format!("写入 JAR 文件失败：{}", jar_path.display()))?;

        let mut artifacts = ArtifactSet::default();
        artifacts.push(ArtifactDescriptor {
            name: options.artifact_name.clone(),
            path: format!("{}.jar", options.artifact_name),
            kind: nyar::ArtifactKind::Executable,
            format: ArtifactFormat::RawBinary,
            target: options.target.clone(),
            lane: TargetLane::Jvm,
        });
        if input.emit_class_file {
            artifacts.push(ArtifactDescriptor {
                name: format!("{}.class", class_file.internal_name.replace('/', ".")),
                path: format!("{}.class", class_file.internal_name.replace('/', ".")),
                kind: nyar::ArtifactKind::AssemblyListing,
                format: ArtifactFormat::RawBinary,
                target: options.target.clone(),
                lane: TargetLane::Jvm,
            });
        }
        Ok(artifacts)
    }
}

/// 将 `JVM` 内部类名转换为 Java 二进制类名。
pub fn internal_name_to_binary_name(internal_name: &str) -> String {
    internal_name.trim_end_matches(".class").replace('/', ".")
}

fn ensure_java_launcher_entry(class_file: &mut JvmClassFile) {
    let launcher_descriptor = JvmMethodDescriptor::new(
        vec![JvmTypeDescriptor::array(JvmTypeDescriptor::Object("java/lang/String".to_string()))],
        JvmTypeDescriptor::Void,
    );
    if class_file.methods.iter().any(|method| method.name == "main" && method.descriptor == launcher_descriptor) {
        return;
    }

    let Some(entry_method) =
        class_file.methods.iter().find(|method| method.name == "main" && method.descriptor.parameter_types.is_empty()).cloned()
    else {
        return;
    };

    let mut instructions = vec![JvmInstruction::InvokeStatic(JvmMethodRef {
        owner: class_file.internal_name.clone(),
        name: entry_method.name.clone(),
        descriptor: entry_method.descriptor.clone(),
    })];
    if entry_method.descriptor.return_type != JvmTypeDescriptor::Void {
        instructions.push(JvmInstruction::Pop);
    }
    instructions.push(JvmInstruction::Return);

    class_file.methods.push(JvmMethodSignature {
        name: "main".to_string(),
        descriptor: launcher_descriptor,
        access_flags: JVM_ACC_PUBLIC | JVM_ACC_STATIC,
        code: Some(JvmCodeBody { max_stack: 1, max_locals: 1, instructions }),
    });
}
