//! 正式编译 lane 的宿主扩展产物降低（Node-API / CPython）。
//!
//! 与残差产品复用同一条目标执行链：计划事实 → 文本 LIR（[`super::x86_64`]）→
//! 唯一程序装配器（[`super::x64_program`]）→ 承载服务（[`super::carrier`]）→
//! 共享库容器（[`super::container`]）。本模块只负责把**语言适配器已冻结的导出
//! 事实**（`export_names` 与函数签名）映射成宿主边界合同，不要求任何特定函数名、
//! 不合成 `main`/`value` 属性，也不重新决定产物用途。
//!
//! 宿主边界能力在这里闭合，未闭合的组合在写出前结构化拒绝：
//!
//! - 导出实参必须是完整 `i64` 整数（宿主按 `napi_get_value_int64` /
//!   `PyArg_ParseTuple("n")` 编组；窄整数没有无损载体，拒绝而不是静默扩展）；
//! - 导出返回只闭合 `Integer64 → Int64` 与 `Boolean → Bool`；字符串需要镜像
//!   描述符事实，正式 lane 尚未降低字符串常量，同样拒绝；
//! - 宿主扩展不接受进程入口：程序有入口与程序公开若干函数是两个独立事实；
//! - Windows CPython 的导入 DLL 名必须由构建环境提供，版本相关名不能由编译器猜。

use std::collections::BTreeMap;

use crate::{
    FragmentSubmission, Result, backend_plan_views::NyarType, lowering::sanitize_symbol,
    nyar_backend_machine::HostProperty,
};
use nyar_types::ItemInstanceId;

use super::{
    HostMethod, HostValueKind, HostModule, X64Platform, X64ProductTarget,
    any_returns_bool, append_text_arena, compile_shared_container, is_c_identifier, node_addon_registration, node_method_wrapper, node_module_data,
    python_method_wrapper, python_module_data, python_module_exec, python_module_registration,
    x86_64::{collect_reachable_functions, lower_function_to_text_program},
    x64_program::encode_x64_program_with_dependencies,
};

/// 宿主扩展降低结果：完整共享库镜像与宿主装载事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X64LoweredExtension {
    /// 共享库镜像字节（`PE32+` DLL 或 `ELF64 ET_DYN`，由平台身份决定）。
    pub executable: Vec<u8>,
    /// 宿主装载入口符号（`napi_register_module_v1` / `PyInit_<module>`）。
    pub entry_symbol: String,
    /// 宿主可见导出名（按冻结导出面顺序）。
    pub host_exports: Vec<String>,
    /// 产物扩展名（含点）：`.node` / `.pyd` / `.so`。
    pub artifact_extension: String,
}

/// 把已冻结的目标计划降低为宿主扩展共享库。
///
/// `target` 必须来自统一目标解析服务的宿主合同；`module_name` 是 CPython 的
/// 导入名（Node-API 不使用模块名，但仍参与产物身份命名）。
pub(crate) fn lower_fragment_to_machine_extension(
    submission: &FragmentSubmission,
    target: &X64ProductTarget,
    module_name: &str,
) -> Result<X64LoweredExtension> {
    let (platform, host) = match target {
        X64ProductTarget::Extension { platform, module, .. } => (*platform, *module),
        X64ProductTarget::Library(_) => return Err("目标不是宿主扩展合同；普通共享库交付尚未接线".to_string()),
    };
    let abi = platform.abi();

    // 宿主扩展不接受进程入口：入口是应用交付事实，与导出面互相独立。
    if let Some(entry) = submission.backend_plan.entry_operation() {
        return Err(format!("宿主扩展不接受进程入口（`{entry:?}`）；导出面与入口是两个独立事实，应用交付请改用 native 可执行目标"));
    }

    let reachable = collect_reachable_functions(submission);
    if reachable.is_empty() {
        return Err(format!("宿主扩展 `{}` 没有可达函数：冻结导出面为空，拒绝产出无导出的扩展", target.name()));
    }

    // 冻结导出面 → 类型化宿主边界事实。
    let methods = frozen_host_methods(submission, target, &reachable)?;
    if methods.is_empty() {
        return Err(format!(
            "宿主扩展 `{}` 的导出面为空：宿主扩展必须至少公开一个已冻结的导出函数",
            target.name()
        ));
    }

    // 符号名：导出函数取冻结导出名的合法符号，内部闭包函数取实例身份名。
    let names = extension_symbol_names(submission, &reachable, &methods)?;
    let mut functions = Vec::with_capacity(reachable.len() + methods.len() + 2);
    for instance in &reachable {
        let view = submission
            .backend_plan
            .get_function(instance)
            .ok_or_else(|| format!("宿主扩展降低：可达操作 `{instance:?}` 缺少函数体"))?;
        functions.push(lower_function_to_text_program(instance, submission, &view.function, &reachable, &names, abi)?);
    }

    let host_library = host_import_library(target, platform)?;
    // `ELF` 没有 IAT：宿主库以 `DT_NEEDED` 显式声明（名字来自冻结的宿主 ABI 配置）。
    let host_dependencies: Vec<String> = match target.cpython_import_library() {
        Some(library) if !platform.is_windows() && matches!(target.host_module(), Some(HostModule::Python)) => vec![library.to_string()],
        _ => Vec::new(),
    };
    let properties: &[HostProperty] = &[];

    let (image, entry_symbol, artifact_extension) = match host {
        HostModule::Node => {
            let (node_data, property_name_offsets, method_name_offsets, error_message_offset) = node_module_data(properties, &methods);
            let mut data = node_data;
            let arena_offset = Some(append_text_arena(&mut data));
            let bool_exports = any_returns_bool(properties, &methods);
            for method in &methods {
                functions.push(node_method_wrapper(method, error_message_offset, platform, &host_library, arena_offset));
            }
            functions.push(node_addon_registration(properties, &property_name_offsets, methods.as_slice(), &method_name_offsets, platform, &host_library, arena_offset));
            let imports = node_imports(&methods, bool_exports, &host_library);
            let image = encode_x64_program_with_dependencies(&functions, &imports, abi, &data, &[], &host_dependencies)?;
            (image, "napi_register_module_v1".to_string(), ".node".to_string())
        }
        HostModule::Python => {
            if !is_c_identifier(module_name) {
                return Err(format!(
                    "CPython 扩展模块名 `{module_name}` 不是合法 C 标识符：`PyInit_<module>` 必须能在 C 中声明"
                ));
            }
            let (definition, relocations, property_name_offsets, format_offsets) = python_module_data(module_name, properties, &methods);
            let mut data = definition;
            let arena_offset = Some(append_text_arena(&mut data));
            let bool_exports = any_returns_bool(properties, &methods);
            for (method, format_offset) in methods.iter().zip(format_offsets) {
                functions.push(python_method_wrapper(method, format_offset, &host_library, arena_offset));
            }
            functions.push(python_module_exec(properties, &property_name_offsets, &host_library, arena_offset));
            functions.push(python_module_registration(module_name, &host_library));
            let imports = python_imports(&methods, bool_exports, &host_library);
            let image = encode_x64_program_with_dependencies(&functions, &imports, abi, &data, &relocations, &host_dependencies)?;
            let entry_symbol = format!("PyInit_{module_name}");
            // 扩展后缀优先取解析服务冻结/探测的宿主 ABI 事实；无配置时才按平台
            // 规范名回落（`.pyd` / `.so`），产物身份差异由 ResolvedTarget::identity 承载。
            let extension = target
                .cpython_module_suffix()
                .map(|suffix| if suffix.starts_with('.') { suffix.to_string() } else { format!(".{suffix}") })
                .unwrap_or_else(|| if platform.is_windows() { ".pyd".to_string() } else { ".so".to_string() });
            (image, entry_symbol, extension)
        }
    };

    let executable = compile_shared_container(platform, &image)?;
    verify_shared_image(&executable, platform)?;

    Ok(X64LoweredExtension {
        executable,
        entry_symbol,
        host_exports: methods.iter().map(|method| method.name.clone()).collect(),
        artifact_extension,
    })
}

/// 从冻结导出面构造类型化宿主方法事实；未闭合的载体在写出前拒绝。
fn frozen_host_methods(submission: &FragmentSubmission, target: &X64ProductTarget, reachable: &[ItemInstanceId]) -> Result<Vec<HostMethod>> {
    let limit = target.method_parameter_limit();
    let mut methods = Vec::new();
    for (instance, export_name) in submission.backend_plan.export_names() {
        if !reachable.contains(instance) {
            return Err(format!("宿主扩展：导出实例 `{instance:?}`（`{export_name}`）不在可达函数集中"));
        }
        let view = submission
            .backend_plan
            .get_function(instance)
            .ok_or_else(|| format!("宿主扩展：导出实例 `{instance:?}`（`{export_name}`）缺少函数体"))?;
        let function = &view.function;
        if function.param_types.len() > limit {
            return Err(format!(
                "宿主扩展方法 `{export_name}` 有 {} 个形参，超出 `{}` 的整数实参上限 {limit}",
                function.param_types.len(),
                target.name()
            ));
        }
        for parameter in &function.param_types {
            if !matches!(parameter, NyarType::Integer64 { signed: true }) {
                return Err(format!(
                    "宿主边界只闭合完整 `i64` 整数实参：`{export_name}` 的形参类型 `{parameter}` 没有无损宿主载体，拒绝写出"
                ));
            }
        }
        let returns = match &function.return_type {
            NyarType::Integer64 { .. } => HostValueKind::Int64,
            NyarType::Boolean => HostValueKind::Bool,
            other => {
                return Err(format!(
                    "宿主边界尚未闭合返回载体 `{other}`：`{export_name}` 只接受 `i64` 或 `bool` 返回，拒绝写出"
                ));
            }
        };
        methods.push(HostMethod {
            name: export_name.clone(),
            symbol: sanitize_symbol(export_name),
            parameters: function.param_types.len(),
            returns,
        });
    }
    methods.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(methods)
}

/// 镜像符号名表：导出函数按冻结导出名，内部函数按实例身份。
fn extension_symbol_names(
    submission: &FragmentSubmission,
    reachable: &[ItemInstanceId],
    methods: &[HostMethod],
) -> Result<BTreeMap<ItemInstanceId, String>> {
    let mut names = BTreeMap::new();
    let mut used: BTreeMap<&str, &str> = BTreeMap::new();
    for method in methods {
        if let Some(previous) = used.insert(method.symbol.as_str(), method.name.as_str()) {
            return Err(format!(
                "宿主扩展导出名 `{}` 与 `{previous}` 规约成同一镜像符号：拒绝产出互相覆盖的导出面",
                method.name
            ));
        }
    }
    for instance in reachable {
        let name = match submission.backend_plan.export_names().get_key_value(instance) {
            Some((_, export_name)) => {
                let method = methods.iter().find(|method| method.name == *export_name).expect("导出实例已构造方法事实");
                method.symbol.clone()
            }
            None => format!("__nyar_fn_{}", instance.index()),
        };
        if names.insert(*instance, name).is_some() {
            return Err(format!("宿主扩展：可达操作 `{instance:?}` 被重复登记"));
        }
    }
    Ok(names)
}

/// Windows 宿主导入模块名；Linux 的 `CallImport` 模块名必须为空。
fn host_import_library(target: &X64ProductTarget, platform: X64Platform) -> Result<String> {
    let Some(module) = target.host_module() else {
        return Err("目标不是宿主扩展合同".to_string());
    };
    if !platform.is_windows() {
        return Ok(String::new());
    }
    match module {
        HostModule::Node => Ok("node.exe".to_string()),
        HostModule::Python => target.cpython_import_library().map(str::to_string).ok_or_else(|| {
            "Windows CPython 扩展必须提供宿主导入 DLL 名（例如 `python312.dll`）：版本相关名不能由编译器猜".to_string()
        }),
    }
}

/// Node-API 导入面：注册与方法回调实际引用的符号。
fn node_imports(methods: &[HostMethod], bool_exports: bool, dll: &str) -> Vec<(String, String)> {
    let mut imports = vec![
        (dll.to_string(), "napi_set_named_property".to_string()),
        (dll.to_string(), "napi_create_int64".to_string()),
    ];
    if !methods.is_empty() {
        for symbol in ["napi_create_function", "napi_get_cb_info", "napi_get_value_int64", "napi_throw_type_error"] {
            imports.push((dll.to_string(), symbol.to_string()));
        }
    }
    if bool_exports {
        imports.push((dll.to_string(), "napi_get_boolean".to_string()));
    }
    imports
}

/// CPython 导入面：多阶段初始化与方法回调实际引用的符号。
fn python_imports(methods: &[HostMethod], bool_exports: bool, dll: &str) -> Vec<(String, String)> {
    let mut imports = vec![
        (dll.to_string(), "PyModuleDef_Init".to_string()),
        (dll.to_string(), "PyLong_FromLongLong".to_string()),
        (dll.to_string(), "PyModule_AddObject".to_string()),
    ];
    if !methods.is_empty() {
        imports.push((dll.to_string(), "PyArg_ParseTuple".to_string()));
    }
    if bool_exports {
        imports.push((dll.to_string(), "PyBool_FromLong".to_string()));
    }
    imports
}

/// 自检：产物必须是与平台身份一致的共享库镜像。
fn verify_shared_image(executable: &[u8], platform: X64Platform) -> Result<()> {
    use crate::nyar_backend_machine::{MachineImageKind, classify_machine_image};
    let facts = classify_machine_image(executable).map_err(|error| format!("宿主扩展产物自检失败：{error}"))?;
    if !facts.shared {
        return Err("宿主扩展产物自检失败：镜像不是共享库".to_string());
    }
    let expected = if platform.is_windows() { MachineImageKind::Pe } else { MachineImageKind::ElfShared };
    if facts.kind != expected {
        return Err(format!("宿主扩展产物自检失败：平台 `{}` 期望 {expected:?}，实际 {facts:?}", platform.name()));
    }
    Ok(())
}
