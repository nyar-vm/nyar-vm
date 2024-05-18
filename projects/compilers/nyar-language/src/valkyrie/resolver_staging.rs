//! 过渡对照：旧 `tgrammar` 文本展开仅供迁移测试，不是生产 API。

#[doc(hidden)]
pub fn expand_target_templates_for_arch(source: &str, arch: &str) -> String {
    crate::transitional::tgrammar::preprocess_target_templates(source, arch)
}
