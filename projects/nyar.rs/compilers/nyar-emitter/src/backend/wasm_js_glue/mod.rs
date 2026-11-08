//! `WASM + Node JS glue` 后端：Node 专用 JS launcher 生成。
//!
//! 仅负责为 `wasm32` + Node 运行方式生成 `.mjs` 启动壳。
//! 不承载 `.wasi` / `wasmtime` / 任何 WASI preview 模块语义，
//! 也不依赖 `backend::wasi` 子树的任何符号。

use std::{fs, path::Path};

use crate::{EmitterError, Result};
use acorn_wasm::{WasmBinaryModule, WasmExternalKind, parse_export_section};
use nyar::{
    abstractions::ArtifactFormat,
    packaging::{ArtifactDescriptor, TargetLane},
};

use crate::{
    backend::binding_builders::{BindingGenerationContext, HostBindingBuilder},
    nyar_backend_wasi::WasmPackageKind,
};

/// `WASM + JS glue` 宿主绑定生成器。
pub(crate) struct JsGlueBindingBuilder;

impl HostBindingBuilder for JsGlueBindingBuilder {
    fn build(&self, context: &BindingGenerationContext<'_>) -> Result<Vec<ArtifactDescriptor>> {
        let launcher_stem = context.artifact_name;
        let launcher_path = context.output_dir.join(format!("{launcher_stem}.mjs"));
        let wasm_path = context.output_dir.join(format!("{launcher_stem}.wasm"));
        let utf8_literals = read_wasm_utf8_literals(&wasm_path).unwrap_or_default();
        let library_mode = match context.wasm_package_kind {
            WasmPackageKind::Library => true,
            WasmPackageKind::Binary => false,
        };
        let library_invoke = if library_mode { read_library_invoke_spec(&wasm_path) } else { None };
        let launcher = build_node_launcher(launcher_stem, context.imports, &utf8_literals, library_mode, library_invoke.as_deref());
        fs::write(&launcher_path, launcher).map_err(|e| format!("写入 Node 启动壳失败：{}: {e}", launcher_path.display()))?;

        Ok(vec![ArtifactDescriptor {
            name: format!("{}.launcher", context.artifact_name),
            path: format!("{}.mjs", context.artifact_name),
            kind: nyar::ArtifactKind::AssemblyListing,
            format: ArtifactFormat::RawBinary,
            target: context.target.clone(),
            lane: TargetLane::Wasm,
        }])
    }
}

/// 为指定导入字段生成 `JS` 实现表达式。
///
/// 仅消费 `WASM` 模块真实声明的导入字段，不注入伪导入。
/// `cli_get_*` 系列字段返回安全默认值，不依赖任何 `CLI` 状态。
fn js_import_impl(field: &str) -> String {
    match field {
        "get_input" => "() => input_value".to_string(),
        "read_source_byte" => "() => { if (source_pos < source_bytes.length) { return source_bytes[source_pos++]; } return -1; }".to_string(),
        "emit_byte" => "(b) => { output_bytes.push(b & 0xFF); }".to_string(),
        "emit_i32" => "(v) => { output_bytes.push(v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF, (v >> 24) & 0xFF); }".to_string(),
        "add" => "(a, b) => (a + b) | 0".to_string(),
        "sub" => "(a, b) => (a - b) | 0".to_string(),
        "mul" => "(a, b) => (a * b) | 0".to_string(),
        "lt" => "(a, b) => a < b ? 1 : 0".to_string(),
        "gt" => "(a, b) => a > b ? 1 : 0".to_string(),
        "le" => "(a, b) => a <= b ? 1 : 0".to_string(),
        "ge" => "(a, b) => a >= b ? 1 : 0".to_string(),
        "eq" => "(a, b) => a === b ? 1 : 0".to_string(),
        "ne" => "(a, b) => a !== b ? 1 : 0".to_string(),
        "cli_get_project" => "() => hostIntern(cli_project)".to_string(),
        "cli_get_target" => "() => hostIntern(cli_target)".to_string(),
        "cli_get_output" => "() => hostIntern(cli_output)".to_string(),
        "cli_get_verbose" => "() => cli_verbose ? 1 : 0".to_string(),
        "get_current_directory" => "() => hostIntern(cwd())".to_string(),
        "file_exists" => "(path) => hostFileExists(path)".to_string(),
        "directory_exists" => "(path) => hostDirectoryExists(path)".to_string(),
        "create_directory" => "(path) => hostCreateDirectory(path)".to_string(),
        "read_file_text" => "(path) => hostReadFileText(path)".to_string(),
        "write_file_text" => "(path, content) => hostWriteFileText(path, content)".to_string(),
        "get_files" => "(path, pattern, recursive) => hostGetFiles(path, pattern, recursive)".to_string(),
        "const_utf8" => "(index) => hostIntern(hostConstUtf8(index))".to_string(),
        "utf8_concat" => "(a, b) => hostUtf8Concat(a, b)".to_string(),
        "utf8_length" => "(s) => hostUtf8Length(s)".to_string(),
        "utf8_trim" => "(s) => hostIntern(hostUtf8Trim(s))".to_string(),
        "utf8_to_lower" => "(s) => hostIntern(hostUtf8ToLower(s))".to_string(),
        "utf8_to_upper" => "(s) => hostIntern(hostUtf8ToUpper(s))".to_string(),
        "utf8_replace" => "(s, oldValue, newValue) => hostIntern(hostUtf8Replace(s, oldValue, newValue))".to_string(),
        "utf8_starts_with" => "(s, prefix) => hostUtf8StartsWith(s, prefix)".to_string(),
        "utf8_ends_with" => "(s, suffix) => hostUtf8EndsWith(s, suffix)".to_string(),
        "utf8_contains" => "(s, other) => hostUtf8Contains(s, other)".to_string(),
        "utf8_equals" => "(a, b) => hostUtf8Equals(a, b)".to_string(),
        "utf8_index_of" => "(s, other) => hostUtf8IndexOf(s, other)".to_string(),
        "utf8_slice" => "(s, start, count) => hostIntern(hostUtf8Slice(s, start, count))".to_string(),
        // `utf16` 在 JS-glue 边界与 `utf8` 共用同一张宿主句柄表；区别是索引/长度
        // 按 UTF-16 code unit 语义（原生 JS 字符串），不做 scalar 展开。
        "const_utf16" => "(index) => hostIntern(hostConstUtf16(index))".to_string(),
        "utf8_to_utf16" => "(s) => hostIntern(hostPathFromRef(s))".to_string(),
        "utf16_to_utf8" => "(s) => hostIntern(hostPathFromRef(s))".to_string(),
        "utf16_concat" => "(a, b) => hostUtf16Concat(a, b)".to_string(),
        "utf16_length" => "(s) => hostUtf16Length(s)".to_string(),
        "utf16_trim" => "(s) => hostUtf16Trim(s)".to_string(),
        "utf16_to_lower" => "(s) => hostUtf16ToLower(s)".to_string(),
        "utf16_to_upper" => "(s) => hostUtf16ToUpper(s)".to_string(),
        "utf16_replace" => "(s, oldValue, newValue) => hostUtf16Replace(s, oldValue, newValue)".to_string(),
        "utf16_starts_with" => "(s, prefix) => hostUtf16StartsWith(s, prefix)".to_string(),
        "utf16_ends_with" => "(s, suffix) => hostUtf16EndsWith(s, suffix)".to_string(),
        "utf16_contains" => "(s, other) => hostUtf16Contains(s, other)".to_string(),
        "utf16_equals" => "(a, b) => hostUtf16Equals(a, b)".to_string(),
        "utf16_index_of" => "(s, other) => hostUtf16IndexOf(s, other)".to_string(),
        "utf16_slice" => "(s, start, count) => hostUtf16Slice(s, start, count)".to_string(),
        "host_intern_utf8" => "(ptr, len) => hostInternUtf8Bytes(ptr, len)".to_string(),
        "host_utf8_extract" => "(handle) => hostUtf8Extract(handle)".to_string(),
        // 未捕获 `raise`：把 guest 的文本 payload 还原成宿主 Error，让用户代码错误
        // 以结构化异常呈现，而不是把整个调用塌缩成 `RuntimeError: unreachable`。
        "host_raise" => "(handle) => { throw new Error(hostResolve(handle)); }".to_string(),
        _ => "() => 0".to_string(),
    }
}

/// 将字段名格式化为合法的 `JS` 对象键。
fn js_object_key(field: &str) -> String {
    if field.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
        && !field.chars().next().is_some_and(|ch| ch.is_ascii_digit())
    {
        field.to_string()
    }
    else {
        format!("\"{}\"", field.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// 收集去重后的导入绑定。
///
/// 仅消费源码声明的 `[wasm(...)]` / contract 解析结果，
/// 不塞默认导入，无产品名耦合。
#[doc(hidden)]
pub fn collect_import_bindings(imports: &[(String, String)]) -> Vec<(String, String)> {
    let mut bindings = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (module, field) in imports {
        let key = (module.clone(), field.clone());
        if seen.insert(key.clone()) {
            bindings.push(key);
        }
    }
    bindings
}

/// 将导入绑定渲染为 `JS` 对象字面量字符串（集成测试专用）。
#[doc(hidden)]
pub fn render_import_object_literal(imports: &[(String, String)]) -> String {
    let bindings = collect_import_bindings(imports);
    let mut by_module: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for (module, field) in bindings {
        by_module.entry(module).or_default().push(field);
    }
    let modules: Vec<String> = by_module
        .iter()
        .map(|(module, fields)| {
            let entries: Vec<String> = fields.iter().map(|field| format!("    {}: {}", js_object_key(field), js_import_impl(field))).collect();
            format!("  {}: {{\n{}\n  }}", js_object_key(module), entries.join(",\n"))
        })
        .collect();
    format!("{{\n{}\n}}", modules.join(",\n"))
}

fn is_library_wasm_module(wasm_path: &Path) -> bool {
    let Ok(bytes) = fs::read(wasm_path)
    else {
        return false;
    };
    let Ok(module) = WasmBinaryModule::from_bytes(&bytes)
    else {
        return false;
    };
    let exports = parse_export_section(&module);
    let has_entry = exports.iter().any(|item| item.name == "main" || item.name == "_start");
    let has_named_function = exports.iter().any(|item| item.kind == WasmExternalKind::Func && item.name != "main" && item.name != "_start");
    has_named_function && !has_entry
}

fn read_library_invoke_spec(wasm_path: &Path) -> Option<String> {
    let bytes = fs::read(wasm_path).ok()?;
    let module = WasmBinaryModule::from_bytes(&bytes).ok()?;
    for section in &module.sections {
        if section.name.as_deref() == Some("nyar.library_invoke") {
            return Some(String::from_utf8_lossy(&section.bytes).to_string());
        }
    }
    None
}

fn library_invoke_marshaler_js(spec_json: &str) -> String {
    // LIBRARY_INVOKE 的 `returns` 与 marshal 目标消费结构化的 WasmAbiType JSON schema：
    // 要么是 string (旧格式 "i64")，要么是 object (新格式 {"kind": "I64", ...})。
    // `exports[*].params` 自 v2 起是参数身份对象 {index, name, type, carrier}，
    // 通过 `paramType` 取其中的 `type` 后再 marshal。
    format!(
        r#"const LIBRARY_INVOKE = {spec_json};

// ── Schema helpers ────────────────────────────────────────────────────────
// 从 param/return schema 提取 ABI kind 字符串。
// 同时兼容旧格式（直接是 "i64"）和新结构化格式（{{"kind": "I64"}}）。
function abiKind(schema) {{
    if (typeof schema === "string") return schema;
    if (schema && typeof schema.kind === "string") return schema.kind;
    return null;
}}

// 判断 schema 在 Wasm 里是否直接以 GC 引用承载（容器 / 具名聚合 / 元组 / 嵌套 Option）。
// 这些值本身就占 anyref 槽，JS 侧只能传递、不能构造；`Option` 的内层若属于此类，
// 不需要额外的 box/unbox 合同。标量内层必须由 schema 的 `box` 合同放行。
function hasWasmHandleCarrier(schema) {{
    const kind = abiKind(schema);
    return kind === "NamedAggregate" || kind === "InstanceHandle" || kind === "Array" || kind === "FixedArray" || kind === "Tuple" || kind === "Option" || kind === "Sum";
}}

// ── 引用存储名义实例：不透明宿主句柄 ─────────────────────────────────────
// `class` 实例在 ABI 位置是同一个 GC 引用，不是字段快照。宿主只能用构造函数
// 产出的句柄，不能按字段重建（那会让 `mut self` 改在副本上，跨调用丢状态）。
// 句柄用 symbol 键包装：不可 JSON 序列化，也不会被普通对象误认。
const INSTANCE_HANDLE = Symbol("nyar.instanceHandle");
function wrapInstanceHandle(schema, raw) {{
    return {{ [INSTANCE_HANDLE]: {{ name: schema.name, module_path: schema.module_path, raw }} }};
}}
function unwrapInstanceHandle(schema, value, path) {{
    if (value === null || typeof value !== "object" || !(INSTANCE_HANDLE in value)) {{
        throw new Error(
            `[wasm-js-glue]${{describePath(path)}}: Cannot marshal InstanceHandle "${{schema.name}}": expected the opaque handle returned by this module's constructor export; field objects and raw references are not accepted.`
        );
    }}
    const handle = value[INSTANCE_HANDLE];
    if (handle.name !== schema.name || handle.module_path !== schema.module_path) {{
        throw new Error(
            `[wasm-js-glue]${{describePath(path)}}: Cannot marshal InstanceHandle "${{schema.module_path}}::${{schema.name}}": the handle belongs to "${{handle.module_path}}::${{handle.name}}".`
        );
    }}
    if (handle.raw == null) {{
        throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot marshal InstanceHandle "${{schema.name}}": the handle carries no reference.`);
    }}
    return handle.raw;
}}

// 从 param 条目提取 marshal 用的类型 schema。
// v2 合同是 {{index, name, type, carrier}}；兼容旧的裸 schema / 字符串条目。
function paramType(param) {{
    if (param && typeof param === "object" && param.type !== undefined) return param.type;
    return param;
}}

function describePath(path) {{
    return path ? ` at ${{path}}` : "";
}}

// 值诊断只用于错误信息：显式区分 `null` / `undefined`，避免把缺值渲染成 "object"。
// 非预期载体一律给出结构化「载体不符」描述，绝不对 managed wasm GC 引用调用
// `String()`——那只会抛 TypeError 并把真正的载体错误掩盖掉。
function describeValue(value) {{
    if (value === null) return "null";
    if (value === undefined) return "undefined";
    if (typeof value === "string") return `string ${{JSON.stringify(value)}}`;
    if (typeof value === "number" || typeof value === "boolean" || typeof value === "bigint") {{
        return `${{typeof value}} ${{value.toString()}}`;
    }}
    if (typeof value === "symbol") return "symbol";
    if (typeof value === "function") return "function";
    if (typeof value === "object") {{
        const prototype = Object.getPrototypeOf(value);
        const shape = prototype === null ? "null-prototype object" : "object";
        return `${{shape}} (carrier mismatch: expected a primitive carrier value, not a managed wasm reference)`;
    }}
    return typeof value;
}}

function bridgeExport(name, role, path) {{
    if (typeof name !== "string" || name.length === 0) {{
        throw new Error(`[wasm-js-glue]${{describePath(path)}}: ABI schema is missing the ` + role + ` export name`);
    }}
    const handler = exports[name];
    if (typeof handler !== "function") {{
        throw new Error(`[wasm-js-glue]${{describePath(path)}}: required ` + role + ` export "${{name}}" is missing from the wasm module`);
    }}
    return handler;
}}

function marshalLibraryArg(schema, value, path) {{
    const kind = abiKind(schema);
    if (kind == null) return value;

    // ── Primitive carriers ──────────────────────────────────────────────
    // I64：Wasm JS API 只接受 BigInt。Number 一律拒绝——即使落在安全整数
    // 范围，它也表达不了完整 64 位合同，静默转换会让「传错类型」看起来像成功。
    // 十进制字符串是跨进程 JSON 的既有 i64 文本载体，按文本精确恢复。
    if (kind === "I64") {{
        if (typeof value === "bigint") return value;
        if (typeof value === "string" && /^[-+]?\d+$/.test(value.trim())) {{
            const parsed = BigInt(value.trim());
            if (parsed < -(2n ** 63n) || parsed > 2n ** 64n - 1n) {{
                throw new Error(
                    `[wasm-js-glue]${{describePath(path)}}: Cannot marshal I64 from ${{value}}: outside the 64-bit range.`
                );
            }}
            return parsed;
        }}
        throw new Error(
            `[wasm-js-glue]${{describePath(path)}}: Cannot marshal I64 from ${{typeof value}} ${{String(value)}}. I64 requires a BigInt or a decimal string; Number cannot represent the full 64-bit range.`
        );
    }}
    // 32/64 位标量一律显式校验：缺值（`undefined`）、显式 `null`、类型不符都不得
    // 被 `value ?? 0` 补成 0。错误信息必须带路径与期望类型，与本项目
    // 「缺参不补 0、`array.set` 不静默写第 0 槽」的失败闭合原则一致。
    if (kind === "I32") {{
        if (typeof value !== "number" || !Number.isInteger(value)) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot marshal I32 from ${{describeValue(value)}}: expected an integer number.`);
        }}
        if (value < -(2 ** 31) || value > 2 ** 32 - 1) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot marshal I32 from ${{value}}: outside the 32-bit range.`);
        }}
        return value;
    }}
    if (kind === "F32" || kind === "F64") {{
        if (typeof value !== "number" || !Number.isFinite(value)) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot marshal ${{kind}} from ${{describeValue(value)}}: expected a finite number.`);
        }}
        return value;
    }}
    if (kind === "Bool") {{
        if (typeof value !== "boolean") {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot marshal Bool from ${{describeValue(value)}}: expected a boolean.`);
        }}
        return value ? 1 : 0;
    }}
    if (kind === "Char") {{
        const codePoint =
            typeof value === "string" && [...value].length === 1
                ? value.codePointAt(0)
                : typeof value === "number" && Number.isInteger(value)
                  ? value
                  : null;
        if (codePoint == null || codePoint < 0 || codePoint > 0x10ffff) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot marshal Char from ${{describeValue(value)}}: expected a single Unicode code point.`);
        }}
        return codePoint;
    }}
    if (kind === "Utf8" || kind === "Utf16") {{
        // Utf8/Utf16 在 JS glue 轨是 i32 host-intern 句柄：JS 字符串入参在此登记为句柄，
        // 已是句柄的整数原样透传；缺值与类型不符一律失败闭合。
        if (typeof value === "string") return hostIntern(value);
        if (typeof value === "number" && Number.isInteger(value)) return value;
        throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot marshal ${{kind}} from ${{describeValue(value)}}: expected a string or an integer host handle.`);
    }}
    if (kind === "Unit") return undefined;

    // ── Option：wire 形状固定为 `None` → null、`Some(v)` → v ─────────────────
    // Wasm 物理降级是 niche：None 就是空引用；Some(引用) 直接透传句柄；
    // Some(标量) 必须先按 schema 的 `box` 合同装进后端已注册的 `[i32]`/`[i64]`。
    // 没有合同就没有表示——宿主不得按 kind 现场造一个。
    if (kind === "Option") {{
        if (value === null) return null;
        if (value === undefined) {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot marshal Option: undefined is not the None wire form. None must be null; Some(v) must be the payload value. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        if (hasWasmHandleCarrier(schema.inner)) return marshalLibraryArg(schema.inner, value, path);
        const contract = schema.box;
        if (!contract || typeof contract.new !== "string" || typeof contract.get !== "string") {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot marshal Option<${{abiKind(schema.inner)}}>: ABI schema carries no box/unbox contract for this payload carrier. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        const boxNew = bridgeExport(contract.new, "option box constructor", path);
        return boxNew(marshalLibraryArg(schema.inner, value, path));
    }}

    // ── Array / 序列视图：JS 数组 → VM 物理数组，必要时再装配成容器聚合 ──
    // 构造只走 wasm 导出（`array.new_default` / `array.set` / `struct.new`
    // 包装），宿主不复刻布局；schema 里的 ops/container 合同是唯一调度依据。
    if (kind === "Array") {{
        if (!Array.isArray(value)) {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot marshal ${{kind}}: expected a JS Array, got ${{typeof value}}. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        const ops = schema.ops;
        if (!ops || typeof ops.new !== "string" || typeof ops.set !== "string") {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot marshal ${{kind}}: ABI schema has no physical array ops contract. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        const allocate = bridgeExport(ops.new, "array.new", path);
        const set = bridgeExport(ops.set, "array.set", path);
        const items = allocate(value.length);
        for (let i = 0; i < value.length; i++) {{
            set(items, i, marshalLibraryArg(schema.element, value[i], `${{path}}[${{i}}]`));
        }}
        if (!schema.container) return items;
        const seq = schema.container;
        const constructor = bridgeExport(seq.constructor, "container constructor", path);
        const ctorArgs = new Array(seq.field_count).fill(null);
        ctorArgs[seq.items_field_index] = items;
        ctorArgs[seq.length_field_index] = marshalLibraryArg(
            seq.length_type,
            abiKind(seq.length_type) === "I64" ? BigInt(value.length) : value.length,
            `${{path}}.length`
        );
        return constructor(...ctorArgs);
    }}

    // ── InstanceHandle：不透明实例句柄原样透传 ─────────────────────────────
    // 引用存储名义实例的语义是共享身份；这里只校验句柄身份并取出 GC 引用，
    // 绝不按字段重建实例。
    if (kind === "InstanceHandle") {{
        return unwrapInstanceHandle(schema, value, path);
    }}

    // ── NamedAggregate：按布局派生的字段构造合同逐字段编组 ──
    // 只有非序列形状走到这里。字段名/构造器名都来自 schema；缺字段合同或
    // 缺 `struct.new` 包装导出都失败闭合，不默认回填字段。
    if (kind === "NamedAggregate") {{
        const fields = schema.fields;
        if (!Array.isArray(fields) || fields.length === 0) {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot marshal NamedAggregate "${{schema.name}}": no field construction contract in ABI schema. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        if (value == null || typeof value !== "object" || Array.isArray(value)) {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot marshal NamedAggregate "${{schema.name}}": expected a field object, got ${{Array.isArray(value) ? "array" : typeof value}}. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        const constructor = bridgeExport(schema.constructor, "aggregate constructor", path);
        const ctorArgs = fields.map((field) => marshalLibraryArg(field.type, value[field.name], `${{path}}.${{field.name}}`));
        return constructor(...ctorArgs);
    }}

    // ── Sum（零 payload unite）：只有变体身份读回合同，没有构造桥 ──
    // 入库方向的构造需要后端注册的 `struct.new` + tag 写入合同；当前冻结
    // schema 不携带该合同，因此显式失败闭合，不按变体名现场造 GC 对象。
    if (kind === "Sum") {{
        throw new Error(
            `[wasm-js-glue]${{describePath(path)}}: Cannot marshal Sum "${{schema.name}}": ABI schema carries no variant construction contract. Schema: ${{JSON.stringify(schema)}}`
        );
    }}

    // ── 未支持形状一律失败闭合 ──
    throw new Error(
        `[wasm-js-glue]${{describePath(path)}}: Cannot marshal schema kind "${{kind}}": no ABI bridge implemented. Schema: ${{JSON.stringify(schema)}}`
    );
}}

function demarshalLibraryReturn(schema, raw, path) {{
    const kind = abiKind(schema);
    if (kind == null) return raw;

    // ── Unit：无返回值 ──────────────────────────────────────────────────
    if (kind === "Unit") return undefined;

    // ── Primitive carriers ──────────────────────────────────────────────
    // I64：Wasm i64 返回值在 JS 侧天然是 BigInt，原样透出保持全 64 位精度；
    // 不允许经 Number 收窄（>2^53 静默丢精度）。返回值侧与入参侧对称：
    // 空引用或非 BigInt 一律失败闭合，不得降级成 `null`。
    if (kind === "I64") {{
        if (typeof raw !== "bigint") {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal I64 from ${{describeValue(raw)}}: expected a BigInt (the wasm i64 carrier).`);
        }}
        return raw;
    }}
    if (kind === "I32") {{
        if (typeof raw !== "number" || !Number.isInteger(raw)) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal I32 from ${{describeValue(raw)}}: expected an integer number.`);
        }}
        return raw;
    }}
    if (kind === "F32" || kind === "F64") {{
        if (typeof raw !== "number" || !Number.isFinite(raw)) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal ${{kind}} from ${{describeValue(raw)}}: expected a finite number.`);
        }}
        return raw;
    }}
    if (kind === "Bool") {{
        if (typeof raw !== "number") {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal Bool from ${{describeValue(raw)}}: expected the i32 bool carrier.`);
        }}
        return raw !== 0;
    }}
    if (kind === "Char") {{
        if (typeof raw !== "number" || !Number.isInteger(raw)) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal Char from ${{describeValue(raw)}}: expected an integer code point.`);
        }}
        return raw;
    }}
    if (kind === "Utf8" || kind === "Utf16") {{
        // 空引用/越界句柄不得降级成空串：句柄必须落在已登记的 host 字符串表内。
        if (typeof raw !== "number" || !Number.isInteger(raw) || raw < 0 || raw >= hostValues.length) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal ${{kind}} from ${{describeValue(raw)}}: expected a valid host string handle.`);
        }}
        return hostResolve(raw);
    }}

    // ── Option：与 marshal 对称；wire 形状固定为 `None` → null、`Some(v)` → v ──
    if (kind === "Option") {{
        if (raw === null) return null;
        if (raw === undefined) {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal Option: wasm produced no reference slot value (undefined); None is the null reference. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        if (hasWasmHandleCarrier(schema.inner)) return demarshalLibraryReturn(schema.inner, raw, path);
        const contract = schema.box;
        if (!contract || typeof contract.new !== "string" || typeof contract.get !== "string") {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal Option<${{abiKind(schema.inner)}}>: ABI schema carries no box/unbox contract for this payload carrier. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        const boxGet = bridgeExport(contract.get, "option box accessor", path);
        return demarshalLibraryReturn(schema.inner, boxGet(raw), path);
    }}

    // ── Array / 序列视图：物理数组 → JS 数组；序列容器先读回字段再投影 ──
    // 序列容器只经 schema 声明的 accessor/ops 读取，按冻结布局事实定位
    // backing array 与计数；不按类型名识别 ArrayList。
    if (kind === "Array") {{
        if (raw == null) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal ${{kind}}: wasm returned a null reference`);
        }}
        const ops = schema.ops;
        if (!ops || typeof ops.length !== "string" || typeof ops.get !== "string") {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal ${{kind}}: ABI schema has no physical array ops contract. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        if (schema.container) {{
            const seq = schema.container;
            const readItems = bridgeExport(seq.items_accessor, "container items accessor", path);
            const readLength = bridgeExport(seq.length_accessor, "container length accessor", path);
            const items = readItems(raw);
            if (items == null) {{
                throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal ${{kind}}: container backing array is a null reference`);
            }}
            const length = Number(demarshalLibraryReturn(seq.length_type, readLength(raw), `${{path}}.length`));
            const get = bridgeExport(ops.get, "array.get", path);
            const output = new Array(length);
            for (let i = 0; i < length; i++) {{
                output[i] = demarshalLibraryReturn(schema.element, get(items, i), `${{path}}[${{i}}]`);
            }}
            return output;
        }}
        const length = Number(bridgeExport(ops.length, "array.len", path)(raw));
        const get = bridgeExport(ops.get, "array.get", path);
        const output = new Array(length);
        for (let i = 0; i < length; i++) {{
            output[i] = demarshalLibraryReturn(schema.element, get(raw, i), `${{path}}[${{i}}]`);
        }}
        return output;
    }}

    // ── InstanceHandle：不透明实例句柄 ─────────────────────────────────────
    // 引用存储名义实例返回后保持同一 GC 引用；空引用一律失败闭合，不降级成
    // 字段对象或 `null`。
    if (kind === "InstanceHandle") {{
        if (raw == null) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal InstanceHandle "${{schema.name}}": wasm returned a null reference`);
        }}
        if (typeof raw !== "object" && typeof raw !== "function") {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal InstanceHandle "${{schema.name}}": expected a managed wasm reference, got ${{describeValue(raw)}}`);
        }}
        return wrapInstanceHandle(schema, raw);
    }}

    // ── NamedAggregate：按布局派生的字段访问合同逐字段读回 ──
    // 字段列表与 accessor 导出名来自 `nyar.library_invoke` 的 `fields` 合同
    // （冻结布局派生，非名字表）：聚合句柄经 accessor 逐字段读取，再按字段
    // schema 递归反编组。缺字段合同、空句柄或缺 accessor 导出都失败闭合，
    // 不猜测字段，不默认回填。
    if (kind === "NamedAggregate") {{
        const fields = schema.fields;
        if (!Array.isArray(fields) || fields.length === 0) {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal NamedAggregate "${{schema.name}}": no field access contract in ABI schema. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        if (raw == null) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal NamedAggregate "${{schema.name}}": wasm returned a null reference`);
        }}
        const aggregate = {{}};
        for (const field of fields) {{
            const accessor = exports[field.accessor];
            if (typeof accessor !== "function") {{
                throw new Error(
                    `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal NamedAggregate "${{schema.name}}" field "${{field.name}}": accessor export "${{field.accessor}}" is missing`
                );
            }}
            aggregate[field.name] = demarshalLibraryReturn(field.type, accessor(raw), `${{path}}.${{field.name}}`);
        }}
        return aggregate;
    }}

    // ── Sum（零 payload unite）：经 tag accessor 读出判别值，映射回变体身份 ──
    // 变体名与 tag 的对应关系来自冻结 sum 布局；tag 不在声明集合内即失败闭合，
    // 不得按声明下标猜变体。
    if (kind === "Sum") {{
        if (raw == null) {{
            throw new Error(`[wasm-js-glue]${{describePath(path)}}: Cannot demarshal Sum "${{schema.name}}": wasm returned a null reference`);
        }}
        const variants = schema.variants;
        if (!Array.isArray(variants) || variants.length === 0) {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal Sum "${{schema.name}}": no variant identity contract in ABI schema. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        const readTag = bridgeExport(schema.tag_accessor, "sum tag accessor", path);
        const tag = readTag(raw);
        if (typeof tag !== "number" || !Number.isInteger(tag)) {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal Sum "${{schema.name}}": tag accessor returned ${{describeValue(tag)}}, expected an integer tag`
            );
        }}
        const variant = variants.find((item) => item.tag === tag);
        if (!variant) {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal Sum "${{schema.name}}": tag ${{tag}} is not declared. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        // 零载荷变体只有身份；带载荷变体必须经该变体自己的 payload accessor
        // 读出只属于它的载荷，绝不把未匹配变体的载荷槽当本变体解释。
        if (!variant.payload) return variant.name;
        if (typeof variant.payload_accessor !== "string") {{
            throw new Error(
                `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal Sum "${{schema.name}}" variant "${{variant.name}}": no payload accessor contract. Schema: ${{JSON.stringify(schema)}}`
            );
        }}
        const readPayload = bridgeExport(variant.payload_accessor, "sum payload accessor", path);
        return {{
            variant: variant.name,
            payload: demarshalLibraryReturn(variant.payload, readPayload(raw), `${{path}}.payload`)
        }};
    }}

    // ── 未支持形状一律失败闭合 ──
    throw new Error(
        `[wasm-js-glue]${{describePath(path)}}: Cannot demarshal schema kind "${{kind}}": no ABI bridge implemented. Schema: ${{JSON.stringify(schema)}}`
    );
}}
"#,
        spec_json = spec_json
    )
}

/// 构建 `Node` 启动壳（`.mjs`）。
///
/// 启动壳只做三件事：读取 `WASM` 字节码、按声明的导入构造 `importObject`、
/// 调用 `exports.main ?? exports._start`。不输出 banner、不解析 CLI 子命令、
/// 不注入伪导入，确保非 CLI 产物的 `stdout` 不被污染。
#[doc(hidden)]
pub fn build_node_launcher(
    artifact_name: &str,
    imports: &[(String, String)],
    utf8_literals: &[String],
    library_mode: bool,
    library_invoke: Option<&str>,
) -> String {
    let utf8_literals_json = serde_json::to_string(utf8_literals).unwrap_or_else(|_| "[]".to_string());
    let has_imports = !imports.is_empty();
    let has_read_source = imports.iter().any(|(_, field)| field == "read_source_byte");
    let smoke_import_object_literal = render_import_object_literal(imports);
    // ── CLI 模式开关：仅基于 import 名存在性 ────────────────────────────
    // 注意：这里只看 `"cli_get_target"` / `"cli_get_project"` / `"cli_get_output"`
    // 这三个 import **名**是否同时存在，用来决定启动壳是否生成 CLI 分派代码。
    //
    // ❌ 这里**绝不**读取编译目标（`TargetExecutionPlan.target`）或 runner
    //    默认值 `"node"` —— `cli_target` 的值是 Node 启动壳内部 JS 变量，
    //    和编译目标事实是两条完全独立的语义线。
    let cli_mode = imports.iter().any(|(_, field)| field == "cli_get_project")
        && imports.iter().any(|(_, field)| field == "cli_get_target")
        && imports.iter().any(|(_, field)| field == "cli_get_output");

    let smoke_import_object = if library_mode {
        if has_imports {
            format!(
                r#"const importObject = {import_object};
let wasmInstance;
let exports;"#,
                import_object = smoke_import_object_literal
            )
        }
        else {
            r#"const importObject = {};
let wasmInstance;
let exports;"#
                .to_string()
        }
    }
    else if has_imports {
        format!(
            r#"const importObject = {import_object};
const instance = await wasmResolveInstance(wasmBytes, importObject);"#,
            import_object = smoke_import_object_literal
        )
    }
    else {
        r#"const instance = await wasmResolveInstance(wasmBytes, {});"#.to_string()
    };

    let smoke_output_logic = if has_imports {
        r#"
if (output_path && output_bytes.length > 0) {
    writeFileSync(output_path, Buffer.from(output_bytes));
    process.exit(0);
} else if (output_bytes.length > 0) {
    process.stdout.write(Buffer.from(output_bytes));
    if (typeof result === "number") {
        process.exit(result);
    }
} else if (typeof result === "number") {
    process.exit(result);
}"#
    }
    else {
        r#"
if (typeof result === "number") {
    process.exit(result);
}"#
    };

    let smoke_arg_parsing = if has_imports {
        if has_read_source {
            r#"const source_path = process.argv[2] || null;
const output_path = process.argv[3] || null;
let source_bytes = Buffer.alloc(0);
let source_pos = 0;
if (source_path) {
    try {
        source_bytes = readFileSync(source_path);
    } catch (e) {}
}
const input_value = 0;
const output_bytes = [];
"#
        }
        else {
            r#"const input_value = process.argv[2] ? parseInt(process.argv[2], 10) : 0;
const output_path = process.argv[3] || null;
const output_bytes = [];
"#
        }
    }
    else {
        ""
    };

    let cli_dispatch = if cli_mode {
        // 空 argv 仅在存在 `help` 导出时走帮助；否则回退到 `main`，避免非完整 CLI 模块被误伤。
        r#"
const args = process.argv.slice(2);
const command = args[0] ?? "";
if (command === "--version" || command === "-V") {
    if (typeof exports.version !== "function") throw new Error("wasm module does not export version");
    process.stdout.write(hostResolve(exports.version()) + "\n");
    process.exit(0);
}
if (command === "--help" || command === "-h") {
    if (typeof exports.help !== "function") throw new Error("wasm module does not export help");
    const code = exports.help();
    if (typeof code === "number" && code !== 0) process.exit(code);
    process.exit(0);
}
if (command === "" && typeof exports.help === "function") {
    const code = exports.help();
    if (typeof code === "number" && code !== 0) process.exit(code);
    process.exit(0);
}
if (command === "build") {
    const positional = [];
    for (let i = 1; i < args.length; i++) {
        if (args[i] === "--target") { cli_target = args[++i] ?? cli_target; continue; }
        if (args[i] === "-o" || args[i] === "--output") { cli_output = args[++i] ?? cli_output; continue; }
        if (args[i] === "--verbose" || args[i] === "-v") { cli_verbose = true; continue; }
        if (!args[i].startsWith("-")) positional.push(args[i]);
    }
    cli_project = positional[0] ?? "";
    if (!cli_project) throw new Error("build requires <project-dir>");
    if (!cli_output) cli_output = pathJoin(cwd(), "dist", "build");
    if (typeof exports.build !== "function") throw new Error("wasm module does not export build");
    const code = exports.build();
    if (typeof code === "number") process.exit(code);
    process.exit(0);
}
"#
    }
    else {
        ""
    };

    format!(
        r#"import {{ readFileSync, writeFileSync, readdirSync, mkdirSync, accessSync, constants as fsConstants }} from "node:fs";
import {{ resolve as pathResolve, join as pathJoin }} from "node:path";
import {{ cwd }} from "node:process";

const UTF8_LITERALS = {utf8_literals_json};
const hostValues = [""];
const hostValueIds = new Map([["", 0]]);
let cli_project = "";
// ── Node runner CLI 默认值，不是编译目标 ────────────────────────────
// 本变量仅用于 Node 启动壳内部的 `--target` CLI 参数默认值。
// WASM 模块内部通过 `cli_get_target` import 读取到的就是这个运行时值，
// 供测试矩阵选择执行宿主（node / deno / bun ...）。
//
// ⚠️  它和 `TargetExecutionPlan.target` / planner manifest 里的**编译目标事实**
//      完全无关。emitter 构建链路在 Rust 侧从不读取本变量的值——
//      `cli_mode` 开关（决定是否生成 CLI 分派代码）只看 `"cli_get_target"`
//      这个 import **名**是否存在，与默认值字符串无关。
let cli_target = "node";
let cli_output = "";
let cli_verbose = false;

function hostIntern(value) {{
    const text = String(value ?? "");
    const known = hostValueIds.get(text);
    if (known !== undefined) return known;
    const handle = hostValues.length;
    hostValues.push(text);
    hostValueIds.set(text, handle);
    return handle;
}}

function hostResolve(handle) {{
    return Number.isInteger(handle) && handle >= 0 && handle < hostValues.length ? hostValues[handle] : "";
}}

// managed 字符串 → 宿主 i32 句柄：guest 侧已把 gc struct 的 [u8] 拷贝进线性内存，
// 宿主按 (ptr, len) 读取 UTF-8 字节并登记进既有 hostIntern 句柄表。
// 每次重新读取 memory.buffer：memory.grow 之后旧 buffer 会被分离（detached）。
function hostLinearMemory() {{
    const memory = typeof exports !== "undefined" && exports ? exports.memory : undefined;
    if (!memory) {{
        throw new Error("[wasm-js-glue] managed string boundary requires the wasm module to export its linear memory as 'memory'");
    }}
    return memory;
}}

function hostInternUtf8Bytes(ptr, len) {{
    const memory = hostLinearMemory();
    const bytes = new Uint8Array(memory.buffer, ptr >>> 0, len >>> 0);
    // ignoreBOM: a leading U+FEFF is string content for `Utf8Text`, not a byte-order marker.
    return hostIntern(new TextDecoder("utf-8", {{ ignoreBOM: true }}).decode(bytes));
}}

// 库导出边界物化：把句柄字符串的 UTF-8 字节经 cabi_realloc 写入线性内存，
// 以多值 [ptr, len] 返回给 guest wrapper，由 guest 拷进 managed struct 的 [u8] 字段。
// 每次重新读取 memory.buffer：memory.grow 之后旧 buffer 会被分离（detached）。
function hostUtf8Extract(handle) {{
    const bytes = new TextEncoder().encode(hostResolve(handle));
    const ptr = Number(exports.cabi_realloc(0, 0, 1, bytes.length));
    new Uint8Array(hostLinearMemory().buffer, ptr >>> 0, bytes.length).set(bytes);
    return [ptr, bytes.length];
}}

function hostConstUtf8(index) {{
    return UTF8_LITERALS[index] ?? "";
}}

function hostPathFromRef(pathRef) {{
    return typeof pathRef === "string" ? pathRef : hostResolve(pathRef);
}}

function hostUtf8Concat(a, b) {{
    return hostIntern(hostPathFromRef(a) + hostPathFromRef(b));
}}

// env.utf8_* index contract (matches std Utf8Text): Unicode **scalars** via [...s],
// never JS string.length / indexOf UTF-16 code units. Host JS strings are UTF-16 storage only.
function hostUtf8Scalars(s) {{
    return [...hostPathFromRef(s)];
}}

function hostUtf8Length(s) {{
    return hostUtf8Scalars(s).length;
}}

// Unicode `White_Space` (25 code points), the same set std `Utf8Text::trim` uses.
// `String.prototype.trim` is not equivalent: it strips U+FEFF and keeps U+0085.
const UTF8_WHITE_SPACE = new Set([
    0x0009, 0x000a, 0x000b, 0x000c, 0x000d, 0x0020, 0x0085, 0x00a0, 0x1680,
    0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200a,
    0x2028, 0x2029, 0x202f, 0x205f, 0x3000,
]);

function hostUtf8IsWhiteSpace(scalar) {{
    return UTF8_WHITE_SPACE.has(scalar.codePointAt(0));
}}

function hostUtf8Trim(s) {{
    const scalars = hostUtf8Scalars(s);
    let start = 0;
    let end = scalars.length;
    while (start < end && hostUtf8IsWhiteSpace(scalars[start])) start += 1;
    while (end > start && hostUtf8IsWhiteSpace(scalars[end - 1])) end -= 1;
    return scalars.slice(start, end).join("");
}}

function hostUtf8ToLower(s) {{
    return hostPathFromRef(s).toLowerCase();
}}

function hostUtf8ToUpper(s) {{
    return hostPathFromRef(s).toUpperCase();
}}

function hostUtf8Replace(s, oldValue, newValue) {{
    const text = hostPathFromRef(s);
    const needle = hostPathFromRef(oldValue);
    const replacement = hostPathFromRef(newValue);
    if (hostUtf8Scalars(needle).length === 0) return text;
    return text.split(needle).join(replacement);
}}

function hostUtf8StartsWith(s, prefix) {{
    return hostPathFromRef(s).startsWith(hostPathFromRef(prefix)) ? 1 : 0;
}}

function hostUtf8EndsWith(s, suffix) {{
    return hostPathFromRef(s).endsWith(hostPathFromRef(suffix)) ? 1 : 0;
}}

function hostUtf8Contains(s, other) {{
    return hostPathFromRef(s).includes(hostPathFromRef(other)) ? 1 : 0;
}}

function hostUtf8Equals(a, b) {{
    return hostPathFromRef(a) === hostPathFromRef(b) ? 1 : 0;
}}

function hostUtf8IndexOf(s, other) {{
    const chars = hostUtf8Scalars(s);
    const needle = hostUtf8Scalars(other);
    if (needle.length === 0) return 0;
    if (needle.length > chars.length) return -1;
    for (let i = 0; i <= chars.length - needle.length; i++) {{
        let ok = true;
        for (let j = 0; j < needle.length; j++) {{
            if (chars[i + j] !== needle[j]) {{ ok = false; break; }}
        }}
        if (ok) return i;
    }}
    return -1;
}}

function hostUtf8Slice(s, start, count) {{
    const chars = hostUtf8Scalars(s);
    const from = Math.max(0, start | 0);
    const len = Math.max(0, count | 0);
    return chars.slice(from, from + len).join("");
}}

// env.utf16_* index contract (matches std Utf16Text): **UTF-16 code units**, i.e.
// native JS string indexing/`.length`. No scalar iteration: a lone surrogate
// stays one code unit and must not be replaced by U+FFFD. This is exactly what
// the utf8 helpers above would lose if they were reused here.

function hostUtf16Text(s) {{
    return hostPathFromRef(s);
}}

// `nyar.strings` 是编码无关的文本字面量表；同一份文本既可作 utf8 句柄，也可作
// utf16 句柄（JS 字符串本身就是 UTF-16 存储）。
function hostConstUtf16(index) {{
    return UTF8_LITERALS[index] ?? "";
}}

function hostUtf16Length(s) {{
    return hostUtf16Text(s).length;
}}

function hostUtf16Concat(a, b) {{
    return hostIntern(hostUtf16Text(a) + hostUtf16Text(b));
}}

function hostUtf16Slice(s, start, count) {{
    const text = hostUtf16Text(s);
    const from = Math.max(0, start | 0);
    const len = Math.max(0, count | 0);
    return hostIntern(text.slice(from, from + len));
}}

// JS `indexOf` returns a UTF-16 code-unit index — the std contract.
function hostUtf16IndexOf(s, other) {{
    return hostUtf16Text(s).indexOf(hostUtf16Text(other));
}}

// 与 `Utf8Text::trim` 同一份 `White_Space` 集合，但按 code unit 前进：代理项
// 不在集合内，因此 lone surrogate 会被保留。
function hostUtf16Trim(s) {{
    const text = hostUtf16Text(s);
    let start = 0;
    let end = text.length;
    while (start < end && UTF8_WHITE_SPACE.has(text.codePointAt(start))) start += 1;
    while (end > start && UTF8_WHITE_SPACE.has(text.codePointAt(end - 1))) end -= 1;
    return hostIntern(text.slice(start, end));
}}

function hostUtf16ToLower(s) {{
    return hostIntern(hostUtf16Text(s).toLowerCase());
}}

function hostUtf16ToUpper(s) {{
    return hostIntern(hostUtf16Text(s).toUpperCase());
}}

function hostUtf16Replace(s, oldValue, newValue) {{
    const text = hostUtf16Text(s);
    const needle = hostUtf16Text(oldValue);
    const replacement = hostUtf16Text(newValue);
    if (needle.length === 0) return hostIntern(text);
    return hostIntern(text.split(needle).join(replacement));
}}

function hostUtf16StartsWith(s, prefix) {{
    return hostUtf16Text(s).startsWith(hostUtf16Text(prefix)) ? 1 : 0;
}}

function hostUtf16EndsWith(s, suffix) {{
    return hostUtf16Text(s).endsWith(hostUtf16Text(suffix)) ? 1 : 0;
}}

function hostUtf16Contains(s, other) {{
    return hostUtf16Text(s).includes(hostUtf16Text(other)) ? 1 : 0;
}}

function hostUtf16Equals(a, b) {{
    return hostUtf16Text(a) === hostUtf16Text(b) ? 1 : 0;
}}

async function wasmResolveInstance(wasmBytes, importObject) {{
    const result = await WebAssembly.instantiate(wasmBytes, importObject);
    return result instanceof WebAssembly.Instance ? result : result.instance;
}}

function hostGetCurrentDirectory() {{
    return cwd();
}}

function hostFileExists(pathRef) {{
    const target = hostPathFromRef(pathRef);
    try {{
        accessSync(target, fsConstants.F_OK);
        return 1;
    }} catch {{
        return 0;
    }}
}}

function hostDirectoryExists(pathRef) {{
    const target = hostPathFromRef(pathRef);
    try {{
        accessSync(target, fsConstants.F_OK);
        return 1;
    }} catch {{
        return 0;
    }}
}}

function hostCreateDirectory(pathRef) {{
    const target = hostPathFromRef(pathRef);
    mkdirSync(target, {{ recursive: true }});
}}

function hostReadFileText(pathRef) {{
    const target = hostPathFromRef(pathRef);
    return hostIntern(readFileSync(target, "utf8"));
}}

function hostWriteFileText(pathRef, contentRef) {{
    const target = hostPathFromRef(pathRef);
    const content = typeof contentRef === "string" ? contentRef : String(contentRef ?? "");
    writeFileSync(target, content, "utf8");
    return 1;
}}

function hostGlobMatch(filePath, pattern) {{
    if (!pattern || pattern === "*") {{
        return true;
    }}
    const normalized = filePath.replace(/\\/g, "/");
    const pat = pattern.replace(/\\/g, "/");
    if (pat.startsWith("*.")) {{
        return normalized.endsWith(pat.slice(1));
    }}
    if (pat.includes("*")) {{
        const escaped = pat.replace(/[.+?^${{}}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*");
        return new RegExp(escaped + "$").test(normalized);
    }}
    return normalized.includes(pat);
}}

function hostEntryFullPath(target, entry) {{
    if (entry.parentPath && entry.parentPath.length > 0) {{
        return pathResolve(entry.parentPath, entry.name);
    }}
    return pathResolve(target, entry.name);
}}

function hostGetFiles(pathRef, patternRef, recursive) {{
    const target = hostPathFromRef(pathRef);
    const pattern = hostPathFromRef(patternRef);
    const entries = readdirSync(target, {{ withFileTypes: true, recursive: recursive !== 0 }});
    const files = [];
    for (const entry of entries) {{
        if (entry.isFile()) {{
            const full = hostEntryFullPath(target, entry);
            if (hostGlobMatch(full, pattern)) {{
                files.push(full);
            }}
        }}
    }}
    return hostIntern(JSON.stringify(files));
}}

{arg_parsing}const LIBRARY_MODE = {library_mode};
const wasmBytes = readFileSync(new URL("./{name}.wasm", import.meta.url));
{import_object}
{library_exports}
{cli_dispatch}
{executable_tail}"#,
        name = artifact_name,
        arg_parsing = smoke_arg_parsing,
        import_object = smoke_import_object,
        library_exports = if library_mode {
            let marshaler = library_invoke.map(library_invoke_marshaler_js).unwrap_or_default();
            format!(
                r#"{marshaler}export async function callExport(name, ...args) {{
    if (!exports) {{
        wasmInstance = await wasmResolveInstance(wasmBytes, importObject);
        exports = wasmInstance.exports;
    }}
    const fn = exports[name];
    if (typeof fn !== "function") {{
        throw new Error(`wasm export not found: ${{name}}`);
    }}
    const spec = LIBRARY_INVOKE?.exports?.[name];
    if (spec?.params) {{
        const marshaled = spec.params.map((param, index) => marshalLibraryArg(paramType(param), args[index], `arg${{index}}`));
        const raw = fn(...marshaled);
        return demarshalLibraryReturn(spec.returns, raw, "return");
    }}
    return fn(...args);
}}
"#,
                marshaler = marshaler
            )
        }
        else {
            String::new()
        },
        cli_dispatch = if library_mode { "" } else { cli_dispatch },
        executable_tail = if library_mode {
            String::new()
        }
        else {
            format!(
                r#"const exports = instance.exports;
const entry = exports.main ?? exports._start;
let result;
if (typeof entry === "function") {{
    result = entry();
}}
{output_logic}"#,
                output_logic = smoke_output_logic
            )
        },
        utf8_literals_json = utf8_literals_json,
        library_mode = if library_mode { "true" } else { "false" },
    )
}

/// 从 `WASM` 自定义段读取 `UTF-8` 字面量表。
///
/// 读取名为 `nyar.strings` 的自定义段（由 `lowering` 后端写入），
/// 使用发射器命名空间而非产品名，作为跨后端的线格式契约。
fn read_wasm_utf8_literals(wasm_path: &Path) -> Result<Vec<String>> {
    let bytes = fs::read(wasm_path).map_err(|e| format!("读取 WASM 失败：{}: {e}", wasm_path.display()))?;
    let module = WasmBinaryModule::from_bytes(&bytes).map_err(|error| format!("WASM 解析失败：{error}"))?;
    for section in &module.sections {
        if section.name.as_deref() == Some("nyar.strings") {
            let payload: serde_json::Value =
                serde_json::from_slice(&section.bytes).map_err(|e| format!("解析 nyar.strings 自定义段失败: {e}"))?;
            if let Some(values) = payload.get("strings").and_then(|value| value.as_array()) {
                return Ok(values.iter().filter_map(|value| value.as_str().map(str::to_string)).collect());
            }
        }
    }
    Ok(Vec::new())
}
