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
    /// 声明携带已退役的 `[intrinsic(...)]` 注解。
    ///
    /// builtin 机制与 intrinsic 同源清退后，`[intrinsic]` 不再是任何实现来源；
    /// 目标语义只能由显式属性（`[wasm_operator]` / `[ovm_operator]` / `[host_contract]`
    /// 等）声明，过期注解必须显式拒绝，不静默忽略。
    IntrinsicAnnotationRetired {
        /// 声明所有者的稳定符号名（module 路径 + 声明名）。
        owner: String,
        /// 声明所在源码路径。
        source: String,
        /// 声明 span。
        span: DiagnosticSpan,
    },
    /// 类型级目标映射属性的形态非法（参数个数不符、参数不是字符串字面量、或存在空串）。
    ///
    /// 类型身份是声明级合同：`[jvm]` 恰好一段非空字符串（内部类名）、`[clr]` 恰好两段
    /// （assembly 与全名）、`[js]` 一到两段（模块与全局名）、`[js_builtin]` 恰好一段；
    /// 畸形声明显式拒绝，不静默忽略、不按名字猜测。函数级同名属性表达调用约定，
    /// 走另一条入口，不受本合同约束。
    MalformedTypeTargetAttribute {
        /// 属性名（`jvm` / `clr` / `js` / `js_builtin`）。
        attribute: String,
        /// 携带该属性的类型聚合全名（module 路径 + 类型名）。
        owner: String,
        /// 期望的稳定形态描述。
        expected: &'static str,
        /// 实际解析出的字符串字面量段数。
        actual: usize,
        /// 类型所在源码路径。
        source: String,
        /// 属性参数 span。
        span: DiagnosticSpan,
    },
}

impl BackendContractError {
    /// 诊断主定位 span。
    pub fn span(&self) -> DiagnosticSpan {
        match self {
            Self::WasiPreview1ImportForbidden { span, .. } => span.clone(),
            Self::IntrinsicAnnotationRetired { span, .. } => span.clone(),
            Self::MalformedTypeTargetAttribute { span, .. } => span.clone(),
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
            Self::IntrinsicAnnotationRetired { owner, source, .. } => write!(
                f,
                "declaration `{owner}` in `{source}` carries the retired `[intrinsic]` annotation; \
                 the builtin/intrinsic bypass is removed — declare `[wasm_operator(\"...\")]`, `[ovm_operator(\"...\")]`, `[host_contract]` \
                 or a regular body instead"
            ),
            Self::MalformedTypeTargetAttribute { attribute, owner, expected, actual, .. } => write!(
                f,
                "type-level target mapping attribute `[{attribute}]` on `{owner}` requires {expected}, but resolved {actual} string segment(s)"
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
/// JVM 目标 operator 绑定属性（`[jvm_operator("ldiv")]`）。
///
/// 与 `[wasm_operator]` 对称：该属性只声明 JVM 规范助记符；助记符到冻结配方的
/// 映射归 JVM backend 所有，声明侧不携带任何语义类别或宿主符号名。
const JVM_OPERATOR_BINDING_ATTRIBUTE: &str = "jvm_operator";
/// CLR 目标 operator 绑定属性（`[clr_operator("i64.div")]`）。
///
/// 与 `[wasm_operator]` 对称：该属性只声明 MSIL 目标 operator 助记符；助记符到
/// 冻结配方的映射归 CLR backend 所有，声明侧不携带任何语义类别或宿主符号名。
const CLR_OPERATOR_BINDING_ATTRIBUTE: &str = "clr_operator";

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
        let owner = if scope.is_empty() { struct_decl.name.as_str().to_string() } else { format!("{scope}::{}", struct_decl.name.as_str()) };
        for attribute in &struct_decl.annotations {
            validate_type_target_attribute(attribute, &owner, &source)?;
        }
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
/// 覆盖两类：目标绑定属性与显式 `[host_contract]`。两者的实现来源不同，
/// 因此与 [`function_interop_contract`] 只共享「抽象」判定，不共享外部导入合同判定：
/// - `[host_contract]` 生成空 host 链接，由 adaptor 的 host provider 填充；
/// - 目标绑定属性直接生成宿主 external_call。
/// 无体声明被判定为抽象后不得降低成空体函数，否则调用静默落到空实现。
/// `[intrinsic(...)]` 已退役，不再使声明成为抽象，由 [`validate_interop_surface`] 显式拒绝。
pub(crate) fn annotations_declare_external_import(annotations: &[HirAttribute]) -> bool {
    annotations
        .iter()
        .any(|attribute| attribute_to_interop_contract(attribute).is_some() || attribute_name(attribute).as_deref() == Some("host_contract"))
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
    // `[intrinsic(...)]` 已随 builtin 旁路整条清退，不再是实现来源；携带该注解的
    // 声明由 [`validate_interop_surface`] 显式拒绝，不静默忽略。
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
    validate_intrinsic_annotation_retired(function, scope, source)
}

/// `[intrinsic(...)]` 注解已随 builtin 旁路整条清退，任何携带该注解的声明
/// 显式拒绝（不区分抽象/有体、带参/裸注解），不静默忽略、不降级成空体函数。
fn validate_intrinsic_annotation_retired(function: &HirFunction, scope: &str, source: &str) -> Result<(), BackendContractError> {
    if !function.annotations.iter().any(|attribute| attribute_name(attribute).as_deref() == Some("intrinsic")) {
        return Ok(());
    }

    let owner = if scope.is_empty() { function.name.as_str().to_string() } else { format!("{scope}::{}", function.name.as_str()) };
    Err(BackendContractError::IntrinsicAnnotationRetired {
        owner,
        source: source.to_string(),
        span: DiagnosticSpan {
            start: usize::try_from(function.span.get_start()).unwrap_or(0),
            end: usize::try_from(function.span.get_end()).unwrap_or(0),
        },
    })
}

/// 类型级目标类型映射属性的形态合同。
///
/// 只检查形状（段数与非空字符串字面量）；类型身份的解析与绑定归后续阶段，
/// 此层不做平台语义解释。未列出的属性名不是类型映射属性，原样放行；
/// 函数级同名属性表达调用约定，不经由此入口校验。
fn validate_type_target_attribute(attribute: &HirAttribute, owner: &str, source: &str) -> Result<(), BackendContractError> {
    let Some(name) = attribute_name(attribute)
    else {
        return Ok(());
    };
    let (min_segments, max_segments, expected): (usize, usize, &'static str) = match name {
        "jvm" => (1, 1, "exactly one non-empty string segment (internal class name)"),
        "clr" => (2, 2, "exactly two non-empty string segments (assembly, full type name)"),
        "js_builtin" => (1, 1, "exactly one non-empty string segment (global path)"),
        _ => return Ok(()),
    };
    let segments = attribute.arguments.iter().filter_map(argument_string_literal).collect::<Vec<_>>();
    let malformed = segments.len() != attribute.arguments.len()
        || segments.len() < min_segments
        || segments.len() > max_segments
        || segments.iter().any(|segment| segment.trim().is_empty());
    if !malformed {
        return Ok(());
    }
    let span = attribute_argument_span(attribute);
    Err(BackendContractError::MalformedTypeTargetAttribute {
        attribute: name.to_string(),
        owner: owner.to_string(),
        expected,
        actual: segments.len(),
        source: source.to_string(),
        span: DiagnosticSpan { start: span.start, end: span.end },
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

    if name == JVM_OPERATOR_BINDING_ATTRIBUTE {
        // 与 `[wasm_operator]` 同形：JVM 规范助记符必须恰好一段；
        // 助记符到冻结配方的映射归 JVM backend 私有表所有。
        if locator_segments.len() != 1 {
            return None;
        }
        return Some(ExternalImportLink::host(Some(Identifier::new("jvm-operator")), locator_segments));
    }

    if name == CLR_OPERATOR_BINDING_ATTRIBUTE {
        // 与 `[wasm_operator]` 同形：MSIL 目标 operator 助记符必须恰好一段；
        // 助记符到冻结配方的映射归 CLR backend 私有表所有。
        if locator_segments.len() != 1 {
            return None;
        }
        return Some(ExternalImportLink::host(Some(Identifier::new("clr-operator")), locator_segments));
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
    // `js_builtin` / `ovm`：抽象宿主绑定，须进入 external import 合同，
    // 否则 Invoke(ItemInstanceId) 在 link 阶段报「没有定义或显式导入合同」。
    matches!(name, "clr" | "wasm" | "wasi" | "jvm" | "com" | "js_builtin" | "ovm")
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
