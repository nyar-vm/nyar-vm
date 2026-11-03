//! native / 自渲染后端的布局桩接口。
//!
//! 阶段三/四尚未实现：桩只声明接口与能力，任何真实降级请求都返回结构化
//! 「阶段未实现」错误（fail-closed），不伪造平台行为。能力声明见
//! [`crate::asgard::style::matrix`]。

use super::{LayoutBinding, capability::BackendStage};
use crate::asgard::error::{AsgardError, AsgardResult};

/// native 后端布局 sink 桩：阶段三接入平台控件或自有布局引擎时实现本接口。
pub trait NativeLayoutSink {
    /// 应用一个布局绑定；桩实现必须 fail-closed。
    fn apply(&self, binding: &LayoutBinding) -> AsgardResult<()>;
}

/// 原型桩：记录阶段身份，任何应用请求都结构化失败。
#[derive(Debug, Clone, Copy)]
pub struct UnimplementedNativeSink {
    /// 桩所代表的后端阶段。
    pub stage: BackendStage,
}

impl NativeLayoutSink for UnimplementedNativeSink {
    fn apply(&self, binding: &LayoutBinding) -> AsgardResult<()> {
        Err(AsgardError::msg(format!(
            "布局合同在 {:?} 阶段尚无实现（桩接口）；绑定 {:?} 被 fail-closed 拒绝，不做静默降级",
            self.stage,
            binding_class(binding)
        )))
    }
}

fn binding_class(binding: &LayoutBinding) -> &str {
    match binding {
        LayoutBinding::Container { class, .. } | LayoutBinding::MeasuredText { class, .. } | LayoutBinding::MeasureRejected { class, .. } => class,
    }
}
