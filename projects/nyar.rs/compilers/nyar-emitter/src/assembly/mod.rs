//! 语言 → 平台的目标输入接入协议。
//!
//! 编排层只统一分区身份、能力、来源与“目标输入准备”的位置；语言语义闭合在
//! 语言自己的 MIR 中，平台承载闭合在各平台自己的计划与 LIR 中，二者经本模块
//! 的适配边界相接。这里不定义通用 operation 枚举，也不存在“共享语义子集”的
//! 特殊成功载荷：所有语言都必须通过自己的适配器提交目标输入。

use std::path::Path;

use nyar::{ArtifactPartition, HostProjectionBoundary, TargetBackendFamily, TheoryBundle};

use crate::{LoweredBackendInput, Result};

/// 语言提交给编排层的一个分区装配结果。
///
/// 携带的是分区身份、优化理论与适配器；编排层不读取语言语义。
pub struct AssembledFragment {
    /// 编排层已经确认的分区身份。
    pub partition: ArtifactPartition,
    /// 该分区选定的优化理论。
    pub theory_bundle: TheoryBundle,
    /// 语言→平台适配器。
    pub input: Box<dyn AdaptedFragmentInput>,
}

impl std::fmt::Debug for AssembledFragment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssembledFragment")
            .field("partition", &self.partition)
            .field("theory_bundle", &self.theory_bundle)
            .finish()
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
    fn prepare(&self, request: &TargetInputRequest<'_>) -> Result<LoweredBackendInput>;
}

/// 按声明能力把分区装配结果准备成驱动层输入。
///
/// 编排层只核对分区身份；目标输入的构造与降低完整留在适配器内。
pub(crate) fn prepare_assembled_fragment(payload: AssembledFragment, request: &TargetInputRequest<'_>) -> Result<LoweredBackendInput> {
    if payload.input.partition() != &payload.partition {
        return Err(format!(
            "适配器声明的分区 `{}` 与编排层分区 `{}` 不一致：拒绝用另一分区的目标输入冒充本分区产物",
            payload.input.partition().name,
            payload.partition.name
        ));
    }
    payload.input.prepare(request)
}
