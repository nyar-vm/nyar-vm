//! SSG 渲染的 OVM 路径：准入门 → V 发射 → OVM 编译/执行 → 读出 → 交付序列化。
//!
//! 本模块是模板静态渲染语义的所有权迁移目标：同一模板由 V 生成代码在
//! Legion 内置 Odin VM 进程内执行（`asgard.render` 合同族），Rust 只做
//! 编排、能力声明与产物落盘，不再解释模板表达式。
//!
//! 迁移边界由 [`admission`] 显式裁决：越出已迁移子集的构造一律结构化
//! 诊断并失败闭合，不在 Rust 侧"临时再解释一遍"。

pub(crate) mod admission;
pub(crate) mod compile;
pub(crate) mod emit;
pub(crate) mod run;
pub(crate) mod serialize;

use std::path::Path;

pub use admission::{AdmissionDiagnostic, AdmissionRule, check_component};

use crate::asgard::{
    awsl::LoweredComponent,
    codegen::StaticRenderResult,
    compile::GeneratedSource,
    error::{AsgardError, AsgardResult},
};

/// 经 OVM 路径渲染组件静态页体：准入失败闭合，成功后返回与既有交付形状
/// 一致的渲染结果（含作用域包装与样式），`diagnostics` 恒为空——所有
/// 诊断都在准入阶段以错误形式结构化上抛。
pub fn render_static_ovm(
    component: &LoweredComponent,
    project_dir: &Path,
    scratch_dir: &Path,
    module_name: &str,
) -> AsgardResult<StaticRenderResult> {
    let diagnostics = admission::check_component(component);
    if !diagnostics.is_empty() {
        let details = diagnostics.iter().map(|diagnostic| diagnostic.to_string()).collect::<Vec<_>>().join("; ");
        return Err(AsgardError::msg(report!("模板越出 SSG 准入子集，拒绝产出残缺页面: {details}")));
    }
    let source = emit::emit_ssg_source(component)?;
    let bytes = compile::compile_ssg_ovm_bundle(
        project_dir,
        GeneratedSource { producer: "asgard.ssg-ovm", source: &source, extra_sources: &[] },
        scratch_dir,
        module_name,
    )?;
    let tree = run::run_ssg_render(&bytes)?;
    let body = serialize::serialize_body(&tree);
    let scope = format!("asgard-static-{}", serialize::to_kebab_case(&component.name));
    let html = format!(
        "<div class=\"{scope}\" data-asgard-static=\"{}\">\n{}\n</div>",
        serialize::escape_attr(&component.route_name),
        body.trim_end()
    );
    let css = crate::asgard::style::web::component_web_css(component);
    Ok(StaticRenderResult { html, css, scope, diagnostics: Vec::new() })
}
