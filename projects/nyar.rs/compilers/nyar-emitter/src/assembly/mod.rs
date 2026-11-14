//! 语言 → 平台的目标输入接入协议。
//!
//! 编排层只统一分区身份、能力、来源与“目标输入准备”的位置；语言语义闭合在
//! 语言自己的 MIR 中，平台承载闭合在各平台自己的计划与 LIR 中，二者经本模块
//! 的适配边界相接。共享 SSA 语义子集只是**可选复用**的一种接入能力，不是
//! 所有语言唯一允许的成功载荷。

use std::path::Path;

use nyar::{ArtifactPartition, HostProjectionBoundary, TargetBackendFamily, TheoryBundle};
use nyar_types::CompiledProgram;

use crate::{FragmentSubmission, LoweredBackendInput, TargetExecutionPlan};

/// 语言提交给编排层的一个分区装配结果。
///
/// 携带的是分区身份与接入能力声明，不再强制携带完整公共 MIR。
pub struct AssembledFragment {
    /// 编排层已经确认的分区身份。
    pub partition: ArtifactPartition,
    /// 该分区选定的优化理论。
    pub theory_bundle: TheoryBundle,
    /// 语言声明的目标输入接入能力。
    pub input: FragmentTargetInput,
}

impl std::fmt::Debug for AssembledFragment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssembledFragment")
            .field("partition", &self.partition)
            .field("theory_bundle", &self.theory_bundle)
            .field("capability", &self.input.capability_name())
            .finish()
    }
}

/// 语言接入编排层的方式。
///
/// 这里刻意不定义通用 operation 枚举：共享子集变体只复用既有语义载荷；适配器
/// 变体把目标输入的构造与降低完整留在语言/平台组合的适配器内。
pub enum FragmentTargetInput {
    /// 复用共享 SSA 语义子集（当前 Valkyrie 路径）。
    SharedSemanticSubset(CompiledProgram),
    /// 语言→平台适配器自带的目标输入。
    Adapted(Box<dyn AdaptedFragmentInput>),
}

impl FragmentTargetInput {
    /// 返回接入能力名，用于诊断与编排层核对。
    pub fn capability_name(&self) -> &'static str {
        match self {
            Self::SharedSemanticSubset(_) => "shared-semantic-subset",
            Self::Adapted(_) => "adapted-target-input",
        }
    }
}

/// 编排层提供给语言→平台适配器的目标输入准备协议。
///
/// 只包含承载位置与目标能力标识；不携带语义模型，也不替适配器决定实现方式。
pub struct TargetInputRequest<'a> {
    /// 分区目标所属的后端家族。
    pub backend_family: TargetBackendFamily,
    /// 分区的宿主投影边界。
    pub host_boundary: HostProjectionBoundary,
    /// 产物目录。
    pub output_dir: &'a Path,
    /// 目标 profile 的 host flavor。
    pub host_flavor: &'a str,
    /// Wasm 打包形态。
    pub wasm_package_kind: crate::nyar_backend_wasi::WasmPackageKind,
}

/// 语言 → 平台适配器。
///
/// 适配器拥有“语言操作如何映射到平台能力”的决定权；编排层只核对分区身份，
/// 不解释其中的语言语义，也不要求适配器先转成共享 IR。
pub trait AdaptedFragmentInput {
    /// 适配器声明其所服务的分区身份，必须与编排层计划一致。
    fn partition(&self) -> &ArtifactPartition;

    /// 在目标能力内准备后端输入。
    fn prepare(&self, request: &TargetInputRequest<'_>) -> crate::Result<LoweredBackendInput>;
}

/// 按声明能力把分区装配结果准备成驱动层输入。
///
/// 共享子集仍走既有降低链；适配器路径只核对身份，不读取或改写其中的语义。
pub(crate) fn prepare_assembled_fragment(payload: AssembledFragment, request: &TargetInputRequest<'_>) -> crate::Result<LoweredBackendInput> {
    match payload.input {
        FragmentTargetInput::SharedSemanticSubset(program) => {
            let submission = fragment_submission_from_shared_subset(&program, &payload.partition, payload.theory_bundle)?;
            LoweredBackendInput::from_fragment_submission(
                &submission,
                request.backend_family,
                request.host_boundary,
                request.output_dir,
                request.host_flavor,
                request.wasm_package_kind,
            )
        }
        FragmentTargetInput::Adapted(adapter) => {
            if adapter.partition() != &payload.partition {
                return Err(format!(
                    "适配器声明的分区 `{}` 与编排层分区 `{}` 不一致：拒绝用另一分区的目标输入冒充本分区产物",
                    adapter.partition().name,
                    payload.partition.name
                ));
            }
            adapter.prepare(request)
        }
    }
}

/// 从共享语义子集生成后端提交视图。
fn fragment_submission_from_shared_subset(
    program: &CompiledProgram,
    partition: &ArtifactPartition,
    theory_bundle: TheoryBundle,
) -> crate::Result<FragmentSubmission> {
    let linked = &program.canonical().linked;
    let fragment = linked.fragments.get(&partition.fragment).ok_or_else(|| format!("Canonical 片段 `{}` 不存在", partition.fragment))?;
    if partition.exported_operations != fragment.exported_operations || partition.entry_operation != fragment.entry_operation {
        return Err(format!("分区 `{}` 的根与 Canonical 片段不一致", partition.name));
    }
    let mut roots = partition.exported_operations.clone();
    if let Some(entry) = partition.entry_operation {
        if !roots.contains(&entry) {
            roots.push(entry);
        }
    }
    let backend_plan = TargetExecutionPlan::from_compiled_program(program, &partition.fragment, theory_bundle, &roots)
        .map_err(|error| format!("目标计划构建失败: {error}"))?;

    Ok(FragmentSubmission { backend_plan: std::sync::Arc::new(backend_plan) })
}
