//! `JVM` 后端容器入口。
//!
//! 这里按 `class / jar` 两个输出边界收口：`class` 与 `jar` 格式模型统一由
//! `acorn-jvm` 提供，本模块只负责后端编排。

#![deny(missing_docs)]

use std::path::PathBuf;

use crate::{DriverRunContract, EmitterError, ManagedAbiContract, Result};
use nyar::{
    abstractions::{ArtifactFormat, BackendInputKind, BinaryArch, BinaryFlavor, BinaryTarget, TargetFamily},
    backends::{BackendDescriptor, CompilationOptions, TargetCodeGenBackend},
    packaging::{ArtifactDescriptor, ArtifactSet, TargetLane},
};

pub use crate::transitional::binary::{
    class::{
        ConstantPoolBuilder, JvmClassError, JvmClassFile, JvmCodeBody, JvmFieldRef, JvmFieldSignature, JvmInstruction, JvmMethodDescriptor,
        JvmMethodRef, JvmMethodSignature, JvmTypeDescriptor, decode_instructions, encode_instructions,
    },
    jar::{JvmJarEntry, JvmJarError, JvmJarPackage},
};

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
    /// 导出入口的 managed ABI 契约（若有）。
    pub managed_abi: Option<ManagedAbiContract>,
    /// 程序入口（`[main]`）的 argv launcher 内部类名。
    ///
    /// 由 lowering 按计划的入口身份生成；驱动层据此产出 `java -cp ... <launcher>`
    /// 运行契约，禁止扫描主类里是否恰好存在名为 `main` 的方法。
    pub entry_launcher: Option<String>,
    /// 后端已冻结的运行契约；驱动层按契约原样上报。
    pub run_contracts: Vec<DriverRunContract>,
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

impl JvmBinaryBackend {
    fn validate_artifact(&self, input: &JvmBinaryBackendInput) -> Result<()> {
        if input.class_file.internal_name.trim().is_empty() {
            return Err(format!("JVM 后端需要有效的主类内部名"));
        }
        if input.class_file.methods.is_empty() {
            return Err(format!("JVM 后端至少需要一个方法"));
        }
        // manifest 与运行合同消费的冻结 launcher 必须真实存在于镜像内，
        // 禁止指向不存在的类再靠扫描补造。
        if let Some(launcher) = &input.entry_launcher {
            let present = input.class_file.internal_name == launcher.as_str()
                || input.companion_classes.iter().any(|class| class.internal_name == *launcher);
            if !present {
                return Err(format!("JVM 冻结入口 launcher 类 `{launcher}` 不在主类或 companion 类中"));
            }
        }
        Ok(())
    }

    fn compile_artifact(&self, input: JvmBinaryBackendInput, options: &CompilationOptions) -> Result<ArtifactSet> {
        std::fs::create_dir_all(&input.output_dir).map_err(|e| format!("创建输出目录失败：{}: {e}", input.output_dir.display()))?;

        let class_file = input.class_file;
        let class_bytes = class_file.to_bytes().map_err(|error| format!("JVM class 编码失败：{error}"))?;

        if input.emit_class_file {
            let relative = format!("{}.class", class_file.internal_name.trim_end_matches(".class"));
            let class_path = input.output_dir.join(&relative);
            if let Some(parent) = class_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建 class 输出目录失败：{}: {e}", parent.display()))?;
            }
            std::fs::write(&class_path, &class_bytes).map_err(|e| format!("写入 class 文件失败：{}: {e}", class_path.display()))?;
            for companion in &input.companion_classes {
                let companion_relative = format!("{}.class", companion.internal_name.trim_end_matches(".class"));
                let companion_path = input.output_dir.join(&companion_relative);
                if let Some(parent) = companion_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| format!("创建 companion class 输出目录失败：{}: {e}", parent.display()))?;
                }
                let companion_bytes = companion.to_bytes().map_err(|error| format!("JVM companion class 编码失败：{error}"))?;
                std::fs::write(&companion_path, companion_bytes)
                    .map_err(|e| format!("写入 companion class 失败：{}: {e}", companion_path.display()))?;
            }
        }

        let jar_path = input.output_dir.join(format!("{}.jar", options.artifact_name));
        let mut package = JvmJarPackage::new(format!("{}.jar", options.artifact_name));
        // `Main-Class` 是镜像属性，只能指向 lowering 冻结的程序入口 launcher；
        // 导出面不伪造入口（入口与导出是两个独立事实）。没有冻结入口的 jar
        // 不写 `Main-Class`，由宿主按 managed ABI 或装载合同消费。
        package.main_class = input.entry_launcher.as_ref().map(|launcher| internal_name_to_binary_name(launcher));
        package.push_class(&class_file).map_err(|error| format!("JAR 打包失败：{error}"))?;
        for companion in &input.companion_classes {
            package.push_class(companion).map_err(|error| format!("JAR 打包 singleton 类失败：{error}"))?;
        }
        let jar_bytes = package.to_bytes().map_err(|error| format!("JAR 编码失败：{error}"))?;
        std::fs::write(&jar_path, jar_bytes).map_err(|e| format!("写入 JAR 文件失败：{}: {e}", jar_path.display()))?;

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

impl TargetCodeGenBackend for JvmBinaryBackend {
    type Input = JvmBinaryBackendInput;

    fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }

    fn validate(&self, input: &Self::Input) -> nyar::backends::BackendResult<()> {
        self.validate_artifact(input).map_err(|error| nyar::backends::BackendError::for_backend(&self.descriptor.name, error))
    }

    fn compile(&self, input: Self::Input, options: &CompilationOptions) -> nyar::backends::BackendResult<ArtifactSet> {
        self.compile_artifact(input, options)
            .map_err(|error| nyar::backends::BackendError::for_backend(&self.descriptor.name, error))
    }
}

/// 将 `JVM` 内部类名转换为 Java 二进制类名。
pub fn internal_name_to_binary_name(internal_name: &str) -> String {
    internal_name.trim_end_matches(".class").replace('/', ".")
}
