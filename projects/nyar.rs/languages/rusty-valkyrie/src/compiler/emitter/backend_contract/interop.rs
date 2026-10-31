use nyar::{CapabilityTag, ExternalImportLink, Identifier, QualifiedName, RuntimeRequirement};
use nyar_types::DiagnosticSpan;

use crate::compiler::types::{
    NamePath,
    hir::{HirArgument, HirAttribute, HirExprKind, HirFunction, HirLiteral, HirModule, HirStringSegment},
};

/// backend contract 对声明面互操作能力的事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendContractError {
    /// 声明使用了 WASI Preview1 host import 位置，Component Model 不支持。
    WasiPreview1ImportForbidden {
        /// 命中的禁用 locator 段。
        segments: Vec<String>,
        /// 属性参数 span。
        span: DiagnosticSpan,
    },
    /// 裸 `[intrinsic]` 抽象声明缺少显式目标绑定。
    ///
    /// 目标语义只能由声明侧的显式属性给出；backend contract 不从函数名推导
    /// 目标 operator 或宿主导入，缺显式绑定时必须失败关闭。
    IntrinsicMissingExplicitTargetBinding {
        /// 声明所有者的稳定符号名（module 路径 + 声明名）。
        owner: String,
        /// 声明所在源码路径。
        source: String,
        /// 声明 span。
        span: DiagnosticSpan,
    },
}

impl BackendContractError {
    /// 诊断主定位 span。
    pub fn span(&self) -> DiagnosticSpan {
        match self {
            Self::WasiPreview1ImportForbidden { span, .. } => span.clone(),
            Self::IntrinsicMissingExplicitTargetBinding { span, .. } => span.clone(),
        }
    }
}

impl std::fmt::Display for BackendContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WasiPreview1ImportForbidden { segments, .. } => write!(
                f,
                "WASI supports Component Model only, `{}` must be replaced with a `wasi:...` component import",
                segments.join("` / `")
            ),
            Self::IntrinsicMissingExplicitTargetBinding { owner, source, .. } => write!(
                f,
                "declaration `{owner}` in `{source}` declares a bare `[intrinsic]` without an explicit target binding; \
                 declare `[wasm_operator(\"...\")]`, `[ovm_operator(\"...\")]` or `[host_contract]` instead of deriving a target from the function name"
            ),
        }
    }
}

/// C ABI FFI 属性坝（`[c("lib", "sym")]`）。
///
/// 兝许用户通过 FFI 链接任愝 C 库（坫 libc），但标准库与编译器违行时本身
/// 丝依赖 libc 系列，平坰基础能力走 `[syscall(...)]`。
const C_BINDING_ATTRIBUTE: &str = "c";
/// 内核系统调用绑定属性坝。
const SYSCALL_BINDING_ATTRIBUTE: &str = "syscall";
/// WASM 目标 operator 绑定属性（`[wasm_operator("i32.add")]`）。
///
/// 该属性只声明目标 operator 助记符；助记符到目标计划的映射归 wasm backend 所有。
const WASM_OPERATOR_BINDING_ATTRIBUTE: &str = "wasm_operator";
/// OVM 目标 operator 绑定属性（`[ovm_operator("i32.add")]`）。
///
/// 与 `[wasm_operator]` 对称：该属性只声明目标 operator 助记符；助记符到目标
/// 计划的映射归 OVM backend 所有，声明侧不携带任何语义类别或宿主符号名。
const OVM_OPERATOR_BINDING_ATTRIBUTE: &str = "ovm_operator";

/// 当剝外部导入链接覝求的能力标签。
pub(crate) fn required_capability(_link: &ExternalImportLink) -> CapabilityTag {
    CapabilityTag::new("host-interop")
}

/// 当剝外部导入链接覝求的违行时需求。
pub(crate) fn runtime_requirement(_link: &ExternalImportLink) -> RuntimeRequirement {
    RuntimeRequirement { key: "host-interop".to_string(), value: "required".to_string() }
}

pub(crate) fn validate_interop_surface(module: &HirModule) -> Result<(), BackendContractError> {
    let source = module_source_label(module);
    let scope = module_path_label(module);
    for function in &module.functions {
        validate_function_interop_surface(function, &scope, &source)?;
    }
    for struct_decl in &module.structs {
        for method in &struct_decl.methods {
            validate_function_interop_surface(method, &scope, &source)?;
        }
    }
    for trait_decl in &module.traits {
        for method in &trait_decl.methods {
            validate_function_interop_surface(method, &scope, &source)?;
        }
        for method in &trait_decl.default_methods {
            validate_function_interop_surface(method, &scope, &source)?;
        }
    }
    for impl_block in &module.impls {
        for method in &impl_block.methods {
            validate_function_interop_surface(method, &scope, &source)?;
        }
    }
    for singleton in &module.singletons {
        for method in &singleton.methods {
            validate_function_interop_surface(method, &scope, &source)?;
        }
        if let Some(constructor) = &singleton.constructor {
            validate_function_interop_surface(constructor, &scope, &source)?;
        }
        if let Some(finalizer) = &singleton.finalizer {
            validate_function_interop_surface(finalizer, &scope, &source)?;
        }
    }
    for submodule in &module.submodules {
        validate_interop_surface(submodule)?;
    }
    Ok(())
}

/// 标注是否把该可调用声明为「实现不在源码中」的抽象声明。
///
/// 覆盖三类：目标绑定属性、显式 `[host_contract]`、以及 `[intrinsic(...)]`。
/// 三者的实现来源不同，因此与 [`function_interop_contract`] 只共享「抽象」判定，
/// 不共享外部导入合同判定：
/// - `[host_contract]` 生成空 host 链接，由 adaptor 的 host provider 填充；
/// - `[intrinsic(...)]` 由 backend intrinsic lowering 直接编码成 MIR 结构操作，
///   不生成宿主 external_call；漏掉intrinsic分流时必须表现为链接失败，
///   而不是静默降级成空体函数。
/// 无体声明被判定为抽象后不得降低成空体函数，否则调用静默落到空实现。
pub(crate) fn annotations_declare_external_import(annotations: &[HirAttribute]) -> bool {
    annotations.iter().any(|attribute| {
        attribute_to_interop_contract(attribute).is_some()
            || attribute_name(attribute).as_deref() == Some("host_contract")
            || attribute_name(attribute).as_deref() == Some("intrinsic")
    })
}

/// 从函数声明推导语言中性的外部导入链接。
#[doc(hidden)]
pub fn function_interop_contract(function: &HirFunction) -> Option<ExternalImportLink> {
    if let Some(link) = function.annotations.iter().find_map(attribute_to_interop_contract) {
        return Some(link);
    }
    if !function.is_abstract {
        return None;
    }
    // 抽象声明只剩 `[host_contract]` 一种「无显式目标绑定」的合法形态：
    // 由 adaptor 的 host_provider 提供 target-conditioned 链接，此处返回空链接，
    // 等待 host provider resolve 填充具体绑定。
    //
    // `[intrinsic(...)]` 由 backend intrinsic lowering 直接编码，不生成 external_call，
    // 因此这里不产链接；裸 `[intrinsic]` 不再从函数名推导 wasm_operator（函数名不是
    // 语义事实），缺显式绑定时由 [`validate_interop_surface`] 结构化失败关闭。
    function.annotations.iter().find_map(|attribute| {
        (attribute_name(attribute).as_deref() == Some("host_contract")).then(|| ExternalImportLink::host(None, Vec::new()))
    })
}

/// 从 `[host_provider(X)]` 属性中杝坖目标契约符坷。
///
/// 该函数返回 provider 所实现的 host_contract 的陝定坝。
/// 例如 `[host_provider(std::console::write_line)]` 返回 `std::console::write_line`。
pub(crate) fn function_host_provider_target(function: &HirFunction) -> Option<QualifiedName> {
    function.annotations.iter().find_map(|attribute| {
        let name = attribute_name(attribute)?;
        if name != "host_provider" {
            return None;
        }
        attribute.arguments.iter().find_map(|argument| argument_qualified_name(argument))
    })
}

/// 模块名的扁平淡化标签，用于诊断 owner 路径。
fn module_path_label(module: &HirModule) -> String {
    module.name.parts().iter().map(|part| part.as_str()).collect::<Vec<_>>().join("::")
}

/// 模块源码路径标签；缺 `source_path` 时退回模块名，避免诊断丢失 owner 定位。
fn module_source_label(module: &HirModule) -> String {
    if !module.source_path.is_empty() {
        return module.source_path.clone();
    }
    let path = module_path_label(module);
    if path.is_empty() { "<unknown>".to_string() } else { path }
}

fn validate_function_interop_surface(function: &HirFunction, scope: &str, source: &str) -> Result<(), BackendContractError> {
    for attribute in &function.annotations {
        validate_interop_attribute(attribute)?;
    }
    validate_intrinsic_target_binding(function, scope, source)
}

/// 裸 `[intrinsic]` 抽象声明必须由显式属性声明目标绑定。
///
/// 目标语义（Wasm operator / OVM operator / 宿主导入）只能来自声明侧显式属性；
/// backend contract 不从函数名推导，也不静默返回「无链接」。
fn validate_intrinsic_target_binding(function: &HirFunction, scope: &str, source: &str) -> Result<(), BackendContractError> {
    let bare_intrinsic = function
        .annotations
        .iter()
        .any(|attribute| attribute_name(attribute).as_deref() == Some("intrinsic") && attribute.arguments.is_empty());
    if !bare_intrinsic || !function.is_abstract {
        return Ok(());
    }

    let explicit_binding = function
        .annotations
        .iter()
        .any(|attribute| attribute_to_interop_contract(attribute).is_some() || attribute_name(attribute).as_deref() == Some("host_contract"));
    if explicit_binding {
        return Ok(());
    }

    let owner = if scope.is_empty() { function.name.as_str().to_string() } else { format!("{scope}::{}", function.name.as_str()) };
    Err(BackendContractError::IntrinsicMissingExplicitTargetBinding {
        owner,
        source: source.to_string(),
        span: DiagnosticSpan {
            start: usize::try_from(function.span.get_start()).unwrap_or(0),
            end: usize::try_from(function.span.get_end()).unwrap_or(0),
        },
    })
}

fn validate_interop_attribute(attribute: &HirAttribute) -> Result<(), BackendContractError> {
    let Some(name) = attribute_name(attribute)
    else {
        return Ok(());
    };

    if name != "wasi" {
        return Ok(());
    }

    let locator_segments = attribute.arguments.iter().filter_map(argument_string_literal).collect::<Vec<_>>();
    let forbidden_segments = locator_segments
        .iter()
        .filter(|segment| matches!(segment.as_str(), "wasi_snapshot_preview1" | "fd_write" | "fd_read"))
        .cloned()
        .collect::<Vec<_>>();
    if forbidden_segments.is_empty() {
        return Ok(());
    }

    let span = attribute_argument_span(attribute);
    Err(BackendContractError::WasiPreview1ImportForbidden {
        segments: forbidden_segments,
        span: DiagnosticSpan { start: span.start, end: span.end },
    })
}

fn attribute_argument_span(attribute: &HirAttribute) -> std::ops::Range<usize> {
    attribute
        .arguments
        .first()
        .map(|argument| {
            let start = usize::try_from(argument.value.span.get_start()).unwrap_or(0);
            let end = usize::try_from(argument.value.span.get_end()).unwrap_or(start);
            start..end
        })
        .unwrap_or(0..0)
}

fn attribute_to_interop_contract(attribute: &HirAttribute) -> Option<ExternalImportLink> {
    let Some(name) = attribute_name(attribute)
    else {
        return None;
    };

    if name == SYSCALL_BINDING_ATTRIBUTE {
        return syscall_attribute_to_interop_contract(attribute);
    }

    let locator_segments = attribute.arguments.iter().filter_map(argument_string_literal).collect::<Vec<_>>();
    if locator_segments.len() != attribute.arguments.len() {
        return None;
    }

    if name == WASM_OPERATOR_BINDING_ATTRIBUTE {
        // 目标 operator 助记符必须恰好一段；空段或多段不产链接，交由声明侧修正。
        if locator_segments.len() != 1 {
            return None;
        }
        return Some(ExternalImportLink::host(Some(Identifier::new("wasm-operator")), locator_segments));
    }

    if name == OVM_OPERATOR_BINDING_ATTRIBUTE {
        // 与 `[wasm_operator]` 同形：目标 operator 助记符必须恰好一段。
        if locator_segments.len() != 1 {
            return None;
        }
        return Some(ExternalImportLink::host(Some(Identifier::new("ovm-operator")), locator_segments));
    }

    if name == C_BINDING_ATTRIBUTE {
        let library = locator_segments.first()?;
        let platform = c_binding_platform(library)?;
        return Some(ExternalImportLink::host(Some(Identifier::new(platform)), locator_segments));
    }

    if is_host_import_surface_tag(name) {
        // Preserve the surface tag (`wasm` / `wasi` / `clr` / …) so backends can
        // filter host imports by target instead of treating every host link as WASI.
        return Some(ExternalImportLink::host(Some(Identifier::new(name)), locator_segments));
    }

    None
}

fn syscall_attribute_to_interop_contract(attribute: &HirAttribute) -> Option<ExternalImportLink> {
    let mut locator_segments = vec!["syscall".to_string()];
    for argument in &attribute.arguments {
        locator_segments.push(argument_locator_token(argument)?);
    }
    if locator_segments.len() < 2 {
        return None;
    }
    Some(ExternalImportLink::host(None, locator_segments))
}

fn argument_locator_token(argument: &HirArgument) -> Option<String> {
    match &argument.value.kind {
        HirExprKind::Literal(HirLiteral::String(_)) => argument_string_literal(argument),
        HirExprKind::Literal(HirLiteral::Integer64(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn c_binding_platform(library: &str) -> Option<&'static str> {
    match library {
        // Windows DLL / NT
        "kernel32" | "user32" | "ole32" | "ws2_32" | "ntdll" => Some("win32"),
        // Apple frameworks / ObjC runtime（非 C 违行时库，但是平坰 FFI）
        "Foundation" | "CoreFoundation" | "CoreGraphics" | "ApplicationServices" | "objc" => Some("darwin"),
        // POSIX C 违行时（用户 FFI 兝许；std / 编译器自身丝依赖）
        "libc" | "libpthread" | "libm" | "libdl" | "librt" | "linux" => Some("linux-gnu"),
        // Darwin C 违行时（用户 FFI 兝许）
        "libSystem" => Some("darwin"),
        _ => None,
    }
}

fn is_host_import_surface_tag(name: &str) -> bool {
    // `js` / `js_builtin` / `ovm`：抽象宿主绑定，须进入 external import 合同，
    // 否则 Invoke(ItemInstanceId) 在 link 阶段报「没有定义或显式导入合同」。
    matches!(name, "clr" | "wasm" | "wasi" | "jvm" | "com" | "js" | "js_builtin" | "ovm")
}

fn attribute_name(attribute: &HirAttribute) -> Option<&str> {
    attribute.name.parts().first().map(|name| name.as_str())
}

fn argument_string_literal(argument: &HirArgument) -> Option<String> {
    let HirExprKind::Literal(HirLiteral::String(literal)) = &argument.value.kind
    else {
        return None;
    };

    let mut rendered = String::new();
    for segment in &literal.segments {
        let HirStringSegment::Text(text) = segment
        else {
            return None;
        };
        rendered.push_str(text);
    }
    Some(rendered)
}

/// 从属性坂数中杝坖陝定坝，支挝路径表达弝和字符串字面針两秝形弝。
fn argument_qualified_name(argument: &HirArgument) -> Option<QualifiedName> {
    match &argument.value.kind {
        HirExprKind::Path(path) => Some(qualified_name_from_path(path)),
        HirExprKind::Literal(HirLiteral::String(literal)) => {
            let mut rendered = String::new();
            for segment in &literal.segments {
                let HirStringSegment::Text(text) = segment
                else {
                    return None;
                };
                rendered.push_str(text);
            }
            let parts = rendered.split("::").map(Identifier::new).collect::<Vec<_>>();
            Some(QualifiedName::new(parts))
        }
        _ => None,
    }
}

fn qualified_name_from_path(path: &NamePath) -> QualifiedName {
    QualifiedName::new(path.parts().to_vec())
}
