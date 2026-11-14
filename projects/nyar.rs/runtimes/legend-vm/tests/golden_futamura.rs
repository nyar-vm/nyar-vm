//! Futamura / partial-evaluation pipeline proofs.
//!
//! Host-script PE product path: specialize(interpret, program) → **native residual**
//! → Windows PE (not OVM). `.odin` / [`OdinVm`] remain probe-only for JS/Python stubs.

use std::collections::HashMap;

use legend_vm::{
    LegacyValue, LegendVmRunner,
    compiler::{CompileArtifact, ResidualValue, eval_native_residual, eval_native_residual_with_inputs, eval_native_residual_with_trace},
};
use odin_vm::{OdinVm, Value};

#[test]
fn bytecode_probe_stack_compiler_to_ovm_js_stub() {
    let runner = LegendVmRunner::new();
    let artifact = runner.compile_module("javascript", "console.log('probe');", "probe").expect("compile module stub");
    let CompileArtifact::BytecodeProbe(module) = artifact
    else {
        panic!("JS stub must stay on bytecode probe lane");
    };
    let bytes = runner.compile_to_nyar(&module).expect("encode nyar bytes");
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("load nyar module");
    let result = vm.run(&loaded, "main", Vec::new()).expect("run nyar module");
    assert_eq!(result, Value::I32(0));
}

#[test]
fn bytecode_probe_python_stub_uses_nyar_path() {
    let runner = LegendVmRunner::new();
    let artifact = runner.compile_module("python", "print('probe')", "probe").expect("compile module stub");
    let CompileArtifact::BytecodeProbe(module) = artifact
    else {
        panic!("Python stub must stay on bytecode probe lane");
    };
    let bytes = runner.compile_to_nyar(&module).expect("encode nyar bytes");
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("load nyar module");
    let result = vm.run(&loaded, "main", Vec::new()).expect("run nyar module");
    assert_eq!(result, Value::I32(0));
}

#[test]
fn lua_pe_specializes_to_native_residual_not_nyar() {
    let runner = LegendVmRunner::new();
    let artifact = runner.compile_module("lua", "return 1", "ok").expect("lua compile_module");
    let CompileArtifact::Native(module) = artifact
    else {
        panic!("Lua PE must be Native residual, not bytecode / OVM");
    };
    assert!(module.is_native_bound());
    assert!(module.op_count() > 0);
    assert!(module.function("main").is_some());
}

#[test]
fn lua_pe_first_projection_arithmetic_matches_interpret_via_native_residual() {
    let source = "local x = 1 + 2\nreturn x\n";
    assert_native_pe_matches_interpret(source, |value| matches!(value, ResidualValue::Int(3)));
}

#[test]
fn lua_pe_print_residual_fails_closed_instead_of_dropping_effect() {
    let source = r#"
local x = 1 + 2
print(x)
"#;
    let runner = LegendVmRunner::new();
    let residual = runner.specialize_native("lua", source, "lua_pe").expect("specialize");
    assert!(residual.is_native_bound(), "残差仍应保留 Print 操作");
    // 诊断解释可以对照语义；产品 lowering 必须失败闭合，不能执行后丢弃 effect。
    let specialized = eval_native_residual(&residual).expect("diagnostic eval");
    assert!(matches!(specialized, ResidualValue::Int(3)), "unexpected residual value: {specialized:?}");
    let error = legend_vm::compiler::emit_native_pe(&residual, None).expect_err("Print 残差不得被编译期吞掉");
    assert!(error.to_string().contains("Print"), "unexpected error: {error}");
}

#[test]
fn lua_short_circuit_residual_preserves_operand_values() {
    // Lua 的 `and`/`or` 返回操作数；残差不得像布尔表达式那样塌缩成 true/false。
    let cases = [
        ("return 7 or 9", ResidualValue::Int(7)),
        ("return 0 or 9", ResidualValue::Int(0)),
        ("return false or 9", ResidualValue::Int(9)),
        ("return 7 and 9", ResidualValue::Int(9)),
        ("return false and 9", ResidualValue::Bool(false)),
        ("return nil and 9", ResidualValue::Nil),
    ];
    for (source, expected) in cases {
        let runner = LegendVmRunner::new();
        let residual = runner.specialize_native("lua", source, "lua_pe").expect("specialize");
        let evaluated = eval_native_residual(&residual).expect("eval native residual");
        assert_eq!(evaluated, expected, "source: {source}");
    }
}

#[test]
fn lua_pe_first_projection_control_flow_matches_interpret_via_native_residual() {
    let source = r#"
local sum = 0
for i = 1, 3 do
  sum = sum + i
end
if sum == 6 then
  return sum
else
  return 0
end
"#;
    assert_native_pe_matches_interpret(source, |value| matches!(value, ResidualValue::Int(6)));
}

#[test]
fn lua_pe_first_projection_function_and_concat_matches_interpret_via_native_residual() {
    let source = r#"
function add(x, y)
  return x + y
end
local total = add(2, 3)
return "n:" .. total
"#;
    assert_native_pe_matches_interpret(source, |value| matches!(value, ResidualValue::String(text) if text == "n:5"));
}

#[test]
fn lua_pe_emits_windows_pe_from_native_residual() {
    let runner = LegendVmRunner::new();
    let residual = runner.specialize_native("lua", "return 1 + 2", "lua_pe").expect("specialize");
    assert!(residual.is_native_bound());

    let dir = std::env::temp_dir().join("legend-vm-lua-pe-native");
    let _ = std::fs::create_dir_all(&dir);
    let out = dir.join("lua_pe.exe");
    let pe = runner.emit_native_pe(&residual, &out).expect("emit PE");
    assert!(pe.starts_with(b"MZ"), "native product must be a Windows PE (MZ), got {} bytes", pe.len());
    assert_eq!(std::fs::read(&out).expect("read pe"), pe);
}

fn assert_native_pe_matches_interpret(source: &str, residual_ok: impl FnOnce(&ResidualValue) -> bool) {
    let runner = LegendVmRunner::new();
    let mut env = HashMap::new();
    let interpreted = runner.run("lua", source, &mut env).expect("interpret");

    let residual = runner.specialize_native("lua", source, "lua_pe").expect("specialize → native");
    assert!(residual.is_native_bound(), "Lua PE residual must be native-bound");
    assert!(residual.op_count() > 0, "specialize must residualize ops");

    let specialized = eval_native_residual(&residual).expect("eval native residual");
    assert!(residual_ok(&specialized), "residual result unexpected: {specialized:?}; interpret={interpreted:?}");
    assert_eq!(specialized.to_key(), legacy_key(&interpreted), "residual={specialized:?} interpret={interpreted:?}");

    if matches!(specialized, ResidualValue::Int(_)) {
        let pe = legend_vm::compiler::emit_native_pe(&residual, None).expect("emit PE bytes");
        assert!(pe.starts_with(b"MZ"), "product path must emit PE, not .odin");
    }
    else {
        // Non-integer residuals have no x64 lowering yet: the product path must
        // fail closed instead of emitting a success-shaped exit-0 stub PE.
        let error = legend_vm::compiler::emit_native_pe(&residual, None).expect_err("non-integer residual must fail closed");
        assert!(error.to_string().contains("not implemented"), "unexpected error: {error}");
    }
}

/// 编译时未知输入：顶层自由变量成为残差 `main` 的形参，解释器与残差在多组
/// 具体输入上给出相同结果（分支两侧都覆盖）。
#[test]
fn lua_pe_unknown_inputs_match_interpret_across_values() {
    let source = r#"
local a = x + y
if a > 10 then
  return a
else
  return a * 2
end
"#;
    assert_unknown_input_matches_interpret(
        source,
        &[("x", LegacyValue::Int(3)), ("y", LegacyValue::Int(4))],
        |value| matches!(value, ResidualValue::Int(14)),
    );
    assert_unknown_input_matches_interpret(
        source,
        &[("x", LegacyValue::Int(7)), ("y", LegacyValue::Int(9))],
        |value| matches!(value, ResidualValue::Int(16)),
    );
}

/// 未知输入的类型与数值假设失效时，guest 解释器与残差必须走同一转换规则，
/// 不能一边报错一边静默给出别的答案。
#[test]
fn lua_pe_unknown_input_type_mismatch_matches_interpret() {
    let source = "return x + y\n";
    assert_unknown_input_matches_interpret(
        source,
        &[("x", LegacyValue::String("abc".to_string())), ("y", LegacyValue::Int(5))],
        // 字符串无法解析为数值时按 guest 规则取 0；结果可能是 Float(5)，键仍是 "5"。
        |value| value.to_key() == "5",
    );
}

/// 未知输入参与循环条件：残差的标签/跳转必须按输入求值，而不是展开成常量。
#[test]
fn lua_pe_unknown_input_drives_loop_matches_interpret() {
    let source = r#"
local s = 0
while s < n do
  s = s + 1
end
return s
"#;
    assert_unknown_input_matches_interpret(source, &[("n", LegacyValue::Int(3))], |value| matches!(value, ResidualValue::Int(3)));
}

/// 参数化残差没有编译期常量可折叠：产品 PE 路径必须显式失败，禁止固定答案。
#[test]
fn lua_pe_parameterized_residual_fails_closed_for_pe_product() {
    let runner = LegendVmRunner::new();
    let residual = runner.specialize_native("lua", "return n + 1", "lua_pe").expect("specialize");
    assert_eq!(residual.function("main").expect("main").parameters, vec!["n".to_string()]);
    let error = legend_vm::compiler::emit_native_pe(&residual, None).expect_err("参数化残差不得常量折叠成产品");
    assert!(error.to_string().contains("parameterized residuals"), "unexpected error: {error}");
}

/// 残化函数读不到 guest 的动态外层绑定：必须特化期失败，而不是运行期读成 `nil`。
#[test]
fn lua_pe_free_variable_in_function_fails_closed() {
    let runner = LegendVmRunner::new();
    let error = runner
        .specialize_native("lua", "function f() return y end\nreturn f()\n", "lua_pe")
        .expect_err("函数内自由变量必须失败闭合");
    assert!(error.to_string().contains("free variable `y`"), "unexpected error: {error}");
}

/// 副作用顺序：解释器的 `print` 序列与残差运行时必须逐事件一致。
#[test]
fn lua_pe_effect_order_matches_interpret() {
    let cases = [
        "print(1)\nprint(2)\nreturn 3\n",
        "if flag then print(\"yes\") else print(\"no\") end\nreturn 0\n",
        "for i = 1, 3 do print(i) end\nreturn 0\n",
        "function f(v) print(v) return v end\nprint(f(7))\nreturn 0\n",
        "local t = { 1 }\nt[1] = 5\nprint(t[1])\nreturn t[1]\n",
    ];
    for source in cases {
        let mut lua_env = HashMap::new();
        lua_env.insert("flag".to_string(), rusty_lua::LuaValue::Bool(true));
        let mut recorded = rusty_lua::RecordedEffects::default();
        let interpreted = rusty_lua::evaluate_lua_source_with_effects(source, &mut lua_env, &mut recorded).expect("interpret");

        let residual = LegendVmRunner::new().specialize_native("lua", source, "lua_pe").expect("specialize");
        let mut inputs: HashMap<String, ResidualValue> = HashMap::new();
        inputs.insert("flag".to_string(), ResidualValue::Bool(true));
        let (specialized, trace) = eval_native_residual_with_trace(&residual, &inputs).expect("eval native residual");

        assert_eq!(trace, recorded.prints, "source: {source}");
        assert_eq!(specialized.to_key(), interpreted.to_string_value(), "source: {source}");
    }
}

/// 动态对象：构造、字段/下标读写、长度与 guest builtin 都保留为残余操作，
/// 残差结果与解释器一致。
#[test]
fn lua_pe_tables_match_interpret() {
    let cases = [
        ("local t = { 1, 2, name = \"x\" }\nt[2] = 5\nt.extra = 9\nreturn t[1] + t[2] + t.extra\n", "15"),
        ("local t = { }\nt[1] = 4\nt[3] = 6\nreturn #t\n", "3"),
        ("local t = { 1, 2, 3 }\nt[3] = nil\nreturn #t\n", "2"),
        ("local t = { a = 1 }\nreturn t.a\n", "1"),
        ("local s = \"abc\"\nreturn s[2]\n", "b"),
        ("local t = {}\ntable_insert(t, 7)\ntable_insert(t, 8)\nreturn t[1] + t[2]\n", "15"),
    ];
    for (source, expected) in cases {
        assert_residual_matches_interpret(source, expected);
    }
}

/// 未定义的运行时调用保留为残余操作，运行期按 guest 规则解析为 `nil`。
#[test]
fn lua_pe_resolves_unknown_call_at_runtime() {
    assert_residual_matches_interpret("return missing(1)\n", "");
}

/// 未知输入作为动态下标键进入残余对象读取。
#[test]
fn lua_pe_unknown_input_as_table_key_matches_interpret() {
    let source = "local t = { 10, 20, 30 }\nreturn t[k]\n";
    assert_unknown_input_matches_interpret(source, &[("k", LegacyValue::Int(2))], |value| value.to_key() == "20");
}

/// 对象残差没有编译期常量可折叠：PE 产品路径必须显式失败。
#[test]
fn lua_pe_table_residual_fails_closed_for_pe_product() {
    let runner = LegendVmRunner::new();
    let residual = runner.specialize_native("lua", "local t = { 1 }\nreturn t[1]\n", "lua_pe").expect("specialize");
    let error = legend_vm::compiler::emit_native_pe(&residual, None).expect_err("对象残差不得编译期折叠成产品");
    assert!(error.to_string().contains("runtime objects"), "unexpected error: {error}");
}

fn assert_residual_matches_interpret(source: &str, expected: &str) {
    let runner = LegendVmRunner::new();
    let mut env = HashMap::new();
    let interpreted = runner.run("lua", source, &mut env).expect("interpret");
    let residual = runner.specialize_native("lua", source, "lua_pe").expect("specialize → native");
    let specialized = eval_native_residual(&residual).expect("eval native residual");
    assert_eq!(specialized.to_key(), legacy_key(&interpreted), "residual={specialized:?} interpret={interpreted:?}");
    assert_eq!(specialized.to_key(), expected, "source: {source}; residual={specialized:?}");
}

fn assert_unknown_input_matches_interpret(
    source: &str,
    inputs: &[(&str, LegacyValue)],
    residual_ok: impl FnOnce(&ResidualValue) -> bool,
) {
    let runner = LegendVmRunner::new();
    let mut env: HashMap<String, LegacyValue> = inputs.iter().map(|(name, value)| (name.to_string(), value.clone())).collect();
    let interpreted = runner.run("lua", source, &mut env).expect("interpret");

    let residual = runner.specialize_native("lua", source, "lua_pe").expect("specialize → native");
    let main = residual.function("main").expect("residual main");
    let expected_names: Vec<String> = inputs.iter().map(|(name, _)| name.to_string()).collect();
    assert_eq!(main.parameters, expected_names, "顶层自由变量必须成为残差 `main` 的未知输入形参");

    let residual_inputs: HashMap<String, ResidualValue> =
        inputs.iter().map(|(name, value)| (name.to_string(), residual_input(value))).collect();
    let specialized = eval_native_residual_with_inputs(&residual, &residual_inputs).expect("eval native residual");
    assert!(residual_ok(&specialized), "residual result unexpected: {specialized:?}; interpret={interpreted:?}");
    assert_eq!(specialized.to_key(), legacy_key(&interpreted), "residual={specialized:?} interpret={interpreted:?}");
}

/// guest `LegacyValue` → 残差值的唯一转换点（夹具不复制数值规则）。
fn residual_input(value: &LegacyValue) -> ResidualValue {
    match value {
        LegacyValue::Null => ResidualValue::Nil,
        LegacyValue::Bool(value) => ResidualValue::Bool(*value),
        LegacyValue::Int(value) => ResidualValue::Int(*value),
        LegacyValue::String(value) => ResidualValue::String(value.clone()),
        other => ResidualValue::String(other.to_string_value()),
    }
}

/// 解释器结果经 `LegacyValue` 适配层返回；`Null` 的宿主显示是 `null`，
/// 而残差的 `Nil` 与 guest `nil` 同为空串——差分比较统一到 guest 语义。
fn legacy_key(value: &LegacyValue) -> String {
    match value {
        LegacyValue::Null => String::new(),
        other => other.to_string_value(),
    }
}

/// 导出函数 ABI：残差产品是 `PE` DLL，未知输入由宿主按 Win64 调用约定传入。
#[cfg(windows)]
mod exported_abi {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use legend_vm::compiler::{ResidualValue, emit_native_pe_library, eval_native_residual_with_inputs};

    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut core::ffi::c_void;
        fn GetProcAddress(module: *mut core::ffi::c_void, name: *const u8) -> *mut core::ffi::c_void;
        fn FreeLibrary(module: *mut core::ffi::c_void) -> i32;
    }

    struct Library {
        module: *mut core::ffi::c_void,
        path: PathBuf,
    }

    impl Drop for Library {
        fn drop(&mut self) {
            unsafe {
                FreeLibrary(self.module);
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn load(dll: &[u8], tag: &str) -> Library {
        let path = std::env::temp_dir().join(format!("legend-residual-{tag}-{}.dll", std::process::id()));
        std::fs::write(&path, dll).expect("write residual dll");
        let mut path_bytes = path.to_string_lossy().into_owned().into_bytes();
        path_bytes.push(0);
        let module = unsafe { LoadLibraryA(path_bytes.as_ptr()) };
        assert!(!module.is_null(), "LoadLibraryA failed for {}", path.display());
        Library { module, path }
    }

    fn call(module: *mut core::ffi::c_void, name: &str, arguments: &[i64]) -> i64 {
        let mut symbol_name = name.as_bytes().to_vec();
        symbol_name.push(0);
        let symbol = unsafe { GetProcAddress(module, symbol_name.as_ptr()) };
        assert!(!symbol.is_null(), "exported symbol `{name}` missing");
        unsafe {
            match arguments {
                [] => {
                    let function: extern "system" fn() -> i64 = std::mem::transmute(symbol);
                    function()
                }
                [a] => {
                    let function: extern "system" fn(i64) -> i64 = std::mem::transmute(symbol);
                    function(*a)
                }
                [a, b] => {
                    let function: extern "system" fn(i64, i64) -> i64 = std::mem::transmute(symbol);
                    function(*a, *b)
                }
                [a, b, c] => {
                    let function: extern "system" fn(i64, i64, i64) -> i64 = std::mem::transmute(symbol);
                    function(*a, *b, *c)
                }
                [a, b, c, d] => {
                    let function: extern "system" fn(i64, i64, i64, i64) -> i64 = std::mem::transmute(symbol);
                    function(*a, *b, *c, *d)
                }
                _ => panic!("the Win64 text ABI supports at most 4 integer inputs"),
            }
        }
    }

    fn evaluate(module: &legend_vm::compiler::NativeResidualModule, arguments: &[&str]) -> i64 {
        let inputs: HashMap<String, ResidualValue> =
            arguments.iter().enumerate().map(|(index, name)| ((*name).to_string(), ResidualValue::Int(index as i64 + 1))).collect();
        match eval_native_residual_with_inputs(module, &inputs).expect("eval residual") {
            ResidualValue::Int(value) => value,
            other => panic!("expected integer residual result, got {other:?}"),
        }
    }

    fn assert_exported_matches_evaluator(source: &str, cases: &[(&[i64], &[(&str, i64)])], tag: &str) {
        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", source, "lua_pe").expect("specialize");
        let dll = emit_native_pe_library(&residual).expect("emit residual dll");
        let library = load(&dll, tag);
        for (arguments, inputs) in cases {
            let inputs: HashMap<String, ResidualValue> =
                inputs.iter().map(|(name, value)| ((*name).to_string(), ResidualValue::Int(*value))).collect();
            let expected = match eval_native_residual_with_inputs(&residual, &inputs).expect("eval residual") {
                ResidualValue::Int(value) => value,
                other => panic!("expected integer residual result, got {other:?}"),
            };
            assert_eq!(call(library.module, "main", arguments), expected, "source: {source}; args: {arguments:?}");
        }
    }

    #[test]
    fn exported_residual_runs_unknown_inputs_from_host() {
        assert_exported_matches_evaluator(
            "return x + y\n",
            &[(&[3, 4], &[("x", 3), ("y", 4)]), (&[10, -2], &[("x", 10), ("y", -2)]), (&[-5, 20], &[("x", -5), ("y", 20)])],
            "add",
        );
    }

    #[test]
    fn exported_residual_branches_on_unknown_inputs() {
        assert_exported_matches_evaluator(
            "if x > 5 then\n  return x * 2\nelse\n  return x + 1\nend\n",
            &[(&[7], &[("x", 7)]), (&[2], &[("x", 2)]), (&[5], &[("x", 5)])],
            "branch",
        );
    }

    #[test]
    fn exported_residual_calls_residual_functions() {
        assert_exported_matches_evaluator(
            "function add(a, b)\n  return a + b\nend\nreturn add(x, y)\n",
            &[(&[4, 6], &[("x", 4), ("y", 6)])],
            "call",
        );
    }

    #[test]
    fn exported_residual_returns_static_string_descriptors() {
        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", "return \"hello\"\n", "lua_pe").expect("specialize");
        let dll = emit_native_pe_library(&residual).expect("emit residual dll");
        let library = load(&dll, "string");
        let descriptor = call(library.module, "main", &[]);
        assert_eq!(read_descriptor(descriptor), "hello");
    }

    #[test]
    fn exported_residual_returns_string_from_branch() {
        let runner = legend_vm::LegendVmRunner::new();
        let residual =
            runner.specialize_native("lua", "if x > 0 then return \"pos\" else return \"neg\" end\n", "lua_pe").expect("specialize");
        let dll = emit_native_pe_library(&residual).expect("emit residual dll");
        let library = load(&dll, "string-branch");
        assert_eq!(read_descriptor(call(library.module, "main", &[1])), "pos");
        assert_eq!(read_descriptor(call(library.module, "main", &[-1])), "neg");
    }

    /// 宿主读取静态字符串描述符：`{ i32 self_relative_ptr; u32 len }` 紧跟字节。
    fn read_descriptor(address: i64) -> String {
        unsafe {
            let base = address as usize;
            let relative = core::ptr::read_unaligned(base as *const i32);
            let len = core::ptr::read_unaligned((base + 4) as *const u32) as usize;
            let pointer = (base as isize + relative as isize) as *const u8;
            let bytes = core::slice::from_raw_parts(pointer, len);
            String::from_utf8(bytes.to_vec()).expect("descriptor must hold UTF-8")
        }
    }

    #[test]
    fn exported_residual_prints_strings_through_the_effect_sink() {
        let dir = std::env::temp_dir().join(format!("legend-residual-print-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("out.txt");

        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", "print(\"a\")\nprint(\"b\")\nreturn 0\n", "lua_pe").expect("specialize");
        let dll = emit_native_pe_library(&residual).expect("emit residual dll");
        let library = load(&dll, "print");
        {
            let file = std::fs::File::create(&path).expect("create capture file");
            let handle = std::os::windows::io::AsRawHandle::as_raw_handle(&file) as i64;
            let _ = call(library.module, "main", &[handle]);
        }
        let observed = std::fs::read_to_string(&path).expect("read captured effects");
        assert_eq!(observed, "a\nb\n", "print order must reach the host sink");

        let residual = runner.specialize_native("lua", "print(\"a\", \"b\")\nreturn 0\n", "lua_pe").expect("specialize");
        let dll = emit_native_pe_library(&residual).expect("emit residual dll");
        let library = load(&dll, "print-tab");
        {
            let file = std::fs::File::create(&path).expect("create capture file");
            let handle = std::os::windows::io::AsRawHandle::as_raw_handle(&file) as i64;
            let _ = call(library.module, "main", &[handle]);
        }
        let observed = std::fs::read_to_string(&path).expect("read captured effects");
        assert_eq!(observed, "a\tb\n", "guest separates print arguments with tabs");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn exported_residual_formats_numbers_booleans_and_nil() {
        let dir = std::env::temp_dir().join(format!("legend-residual-format-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("out.txt");

        let runner = legend_vm::LegendVmRunner::new();
        let source = "print(42)\nprint(-7, true, false)\nprint()\nprint(nil)\nreturn 0\n";
        let residual = runner.specialize_native("lua", source, "lua_pe").expect("specialize");
        let dll = emit_native_pe_library(&residual).expect("emit residual dll");
        let library = load(&dll, "format");
        {
            let file = std::fs::File::create(&path).expect("create capture file");
            let handle = std::os::windows::io::AsRawHandle::as_raw_handle(&file) as i64;
            let _ = call(library.module, "main", &[handle]);
        }
        let observed = std::fs::read_to_string(&path).expect("read captured effects");
        assert_eq!(observed, "42\n-7\ttrue\tfalse\n\n\n", "guest formatting must match the interpreter");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn exported_residual_prints_inside_residual_functions() {
        let dir = std::env::temp_dir().join(format!("legend-residual-print-nested-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("out.txt");

        let source = "function add(a, b)\n  return a + b\nend\nfunction show(v)\n  print(v)\n  return v\nend\nprint(add(2, 3))\nshow(9)\nreturn 0\n";
        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", source, "lua_pe").expect("specialize");
        let dll = emit_native_pe_library(&residual).expect("emit residual dll");
        let library = load(&dll, "print-nested");
        {
            let file = std::fs::File::create(&path).expect("create capture file");
            let handle = std::os::windows::io::AsRawHandle::as_raw_handle(&file) as i64;
            let _ = call(library.module, "main", &[handle]);
        }
        let observed = std::fs::read_to_string(&path).expect("read captured effects");
        assert_eq!(observed, "5\n9\n", "effects in residual functions must share the passed sink");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn exported_residual_fails_closed_for_unsupported_effects_and_objects() {
        let runner = legend_vm::LegendVmRunner::new();
        for (source, needle) in [
            ("function f(s)\n  return 0\nend\nf(\"x\")\nreturn 0\n", "passes a string descriptor"),
            ("local t = { 1 }\nreturn t[1]\n", "TableNew"),
            ("return x / y\n", "Div"),
            ("return x .. \"s\"\n", "Concat"),
            ("return \"a\" == \"b\"\n", "byte-wise comparison"),
        ] {
            let residual = runner.specialize_native("lua", source, "lua_pe").expect("specialize");
            let error = emit_native_pe_library(&residual).expect_err("unsupported residual must fail closed");
            assert!(error.contains(needle), "source: {source}; unexpected error: {error}");
        }
    }

    #[test]
    fn exported_residual_still_evaluates_through_the_reference_runtime() {
        // 参考求值器与导出 ABI 使用同一残差模块：一个模块可以同时走两条证据路径。
        let residual = legend_vm::LegendVmRunner::new().specialize_native("lua", "return x + 1\n", "lua_pe").expect("specialize");
        assert_eq!(evaluate(&residual, &["x"]), 2);
    }
}

/// 导出函数 ABI：Linux 残差产品是 x86-64 `ELF` `.so`，宿主 `dlopen`/`dlsym`，
/// 未知输入按 SysV64 整数形参传入，`print` 经 `write(2)` 写到隐藏的首个 fd 形参。
#[cfg(target_os = "linux")]
mod exported_elf_abi {
    use std::collections::HashMap;
    use std::ffi::CString;
    use std::os::unix::io::AsRawFd;
    use std::path::PathBuf;

    use legend_vm::compiler::{ResidualValue, emit_native_elf_library, eval_native_residual_with_inputs};

    unsafe extern "C" {
        fn dlopen(filename: *const i8, flag: i32) -> *mut core::ffi::c_void;
        fn dlsym(handle: *mut core::ffi::c_void, symbol: *const i8) -> *mut core::ffi::c_void;
        fn dlclose(handle: *mut core::ffi::c_void) -> i32;
        fn dlerror() -> *const i8;
    }

    fn last_dl_error() -> String {
        unsafe {
            let message = dlerror();
            if message.is_null() {
                "no dlerror".to_string()
            }
            else {
                std::ffi::CStr::from_ptr(message).to_string_lossy().into_owned()
            }
        }
    }

    struct Library {
        module: *mut core::ffi::c_void,
        path: PathBuf,
    }

    impl Drop for Library {
        fn drop(&mut self) {
            unsafe {
                dlclose(self.module);
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn load(shared: &[u8], tag: &str) -> Library {
        let path = std::env::temp_dir().join(format!("legend-residual-{tag}-{}.so", std::process::id()));
        std::fs::write(&path, shared).expect("write residual .so");
        let name = CString::new(path.to_string_lossy().as_bytes()).expect("utf8 path");
        let module = unsafe { dlopen(name.as_ptr(), 2 /* RTLD_NOW */) };
        assert!(!module.is_null(), "dlopen failed for {}: {} ({} bytes)", path.display(), last_dl_error(), shared.len());
        Library { module, path }
    }

    fn symbol(module: *mut core::ffi::c_void, name: &str) -> *mut core::ffi::c_void {
        let name = CString::new(name).expect("symbol name");
        let symbol = unsafe { dlsym(module, name.as_ptr()) };
        assert!(!symbol.is_null(), "exported symbol `{name:?}` missing");
        symbol
    }

    fn call(module: *mut core::ffi::c_void, name: &str, arguments: &[i64]) -> i64 {
        let symbol = symbol(module, name);
        unsafe {
            match arguments {
                [] => {
                    let function: extern "C" fn() -> i64 = std::mem::transmute(symbol);
                    function()
                }
                [a] => {
                    let function: extern "C" fn(i64) -> i64 = std::mem::transmute(symbol);
                    function(*a)
                }
                [a, b] => {
                    let function: extern "C" fn(i64, i64) -> i64 = std::mem::transmute(symbol);
                    function(*a, *b)
                }
                [a, b, c] => {
                    let function: extern "C" fn(i64, i64, i64) -> i64 = std::mem::transmute(symbol);
                    function(*a, *b, *c)
                }
                [a, b, c, d] => {
                    let function: extern "C" fn(i64, i64, i64, i64) -> i64 = std::mem::transmute(symbol);
                    function(*a, *b, *c, *d)
                }
                [a, b, c, d, e] => {
                    let function: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(symbol);
                    function(*a, *b, *c, *d, *e)
                }
                _ => panic!("the SysV64 text ABI supports at most 6 integer inputs"),
            }
        }
    }

    fn assert_exported_matches_evaluator(source: &str, cases: &[(&[i64], &[(&str, i64)])], tag: &str) {
        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", source, "lua_elf").expect("specialize");
        let shared = emit_native_elf_library(&residual).expect("emit residual .so");
        let library = load(&shared, tag);
        for (arguments, inputs) in cases {
            let inputs: HashMap<String, ResidualValue> =
                inputs.iter().map(|(name, value)| ((*name).to_string(), ResidualValue::Int(*value))).collect();
            let expected = match eval_native_residual_with_inputs(&residual, &inputs).expect("eval residual") {
                ResidualValue::Int(value) => value,
                other => panic!("expected integer residual result, got {other:?}"),
            };
            assert_eq!(call(library.module, "main", arguments), expected, "source: {source}; args: {arguments:?}");
        }
    }

    #[test]
    fn exported_residual_runs_unknown_inputs_from_host() {
        assert_exported_matches_evaluator(
            "return x + y\n",
            &[(&[3, 4], &[("x", 3), ("y", 4)]), (&[10, -2], &[("x", 10), ("y", -2)]), (&[-5, 20], &[("x", -5), ("y", 20)])],
            "elf-add",
        );
    }

    #[test]
    fn exported_residual_branches_and_calls_on_linux() {
        assert_exported_matches_evaluator(
            "if x > 5 then\n  return x * 2\nelse\n  return x + 1\nend\n",
            &[(&[7], &[("x", 7)]), (&[2], &[("x", 2)])],
            "elf-branch",
        );
        assert_exported_matches_evaluator(
            "function add(a, b)\n  return a + b\nend\nreturn add(x, y)\n",
            &[(&[4, 6], &[("x", 4), ("y", 6)])],
            "elf-call",
        );
    }

    #[test]
    fn exported_residual_returns_static_string_descriptors() {
        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", "if x > 0 then return \"pos\" else return \"neg\" end\n", "lua_elf")
            .expect("specialize");
        let shared = emit_native_elf_library(&residual).expect("emit residual .so");
        let library = load(&shared, "elf-string");
        assert_eq!(read_descriptor(call(library.module, "main", &[1])), "pos");
        assert_eq!(read_descriptor(call(library.module, "main", &[-1])), "neg");
    }

    /// 宿主读取静态字符串描述符：`{ i32 self_relative_ptr; u32 len }` 紧跟字节。
    fn read_descriptor(address: i64) -> String {
        unsafe {
            let base = address as usize;
            let relative = core::ptr::read_unaligned(base as *const i32);
            let len = core::ptr::read_unaligned((base + 4) as *const u32) as usize;
            let pointer = (base as isize + relative as isize) as *const u8;
            let bytes = core::slice::from_raw_parts(pointer, len);
            String::from_utf8(bytes.to_vec()).expect("descriptor must hold UTF-8")
        }
    }

    #[test]
    fn exported_residual_prints_through_the_write_syscall() {
        let dir = std::env::temp_dir().join(format!("legend-residual-elf-print-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("out.txt");
        let runner = legend_vm::LegendVmRunner::new();

        for (source, expected) in [
            ("print(\"a\")\nprint(\"b\")\nreturn 0\n", "a\nb\n"),
            ("print(\"a\", \"b\")\nreturn 0\n", "a\tb\n"),
            ("print(42)\nprint(-7, true, false)\nprint()\nprint(nil)\nreturn 0\n", "42\n-7\ttrue\tfalse\n\n\n"),
            (
                "function add(a, b)\n  return a + b\nend\nfunction show(v)\n  print(v)\n  return v\nend\nprint(add(2, 3))\nshow(9)\nreturn 0\n",
                "5\n9\n",
            ),
        ] {
            let residual = runner.specialize_native("lua", source, "lua_elf").expect("specialize");
            let shared = emit_native_elf_library(&residual).expect("emit residual .so");
            let library = load(&shared, "elf-print");
            {
                let file = std::fs::File::create(&path).expect("create capture file");
                let fd = file.as_raw_fd() as i64;
                let _ = call(library.module, "main", &[fd]);
            }
            let observed = std::fs::read_to_string(&path).expect("read captured effects");
            assert_eq!(observed, expected, "source: {source}");
        }

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn exported_residual_fails_closed_for_unsupported_shapes_on_linux() {
        let runner = legend_vm::LegendVmRunner::new();
        for (source, needle) in [("local t = { 1 }\nreturn t[1]\n", "TableNew"), ("return x / y\n", "Div")] {
            let residual = runner.specialize_native("lua", source, "lua_elf").expect("specialize");
            let error = emit_native_elf_library(&residual).expect_err("unsupported residual must fail closed");
            assert!(error.contains(needle), "source: {source}; unexpected error: {error}");
        }
    }
}

/// Node-API `.node` lane：Node 的 Node-API 装载器 `require()` 产物并取回残差值。
#[cfg(target_os = "linux")]
mod node_addon_abi {
    use legend_vm::compiler::emit_native_node_addon;

    fn addon(source: &str, tag: &str) -> std::path::PathBuf {
        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", source, "lua_node").expect("specialize");
        let bytes = emit_native_node_addon(&residual).expect("emit node addon");
        let path = std::env::temp_dir().join(format!("legend-addon-{tag}-{}.node", std::process::id()));
        std::fs::write(&path, bytes).expect("write addon");
        path
    }

    fn run_node(script: &str) -> String {
        let output = std::process::Command::new("node").arg("-e").arg(script).output().expect("spawn node");
        assert!(
            output.status.success(),
            "node failed: status={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim_end().to_string()
    }

    #[test]
    fn node_requires_the_addon_and_reads_the_residual_value() {
        let path = addon("return 42\n", "value");
        let script = format!("console.log(require('{}'))", path.display());
        assert_eq!(run_node(&script), "42", "module.exports 必须是残差 main 的数值");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn node_addon_prints_through_stdout() {
        let path = addon("print(\"hello\")\nprint(3)\nreturn 7\n", "print");
        let script = format!("console.log(require('{}'))", path.display());
        assert_eq!(run_node(&script), "hello\n3\n7", "print 顺序与 module.exports 共用 stdout");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn node_addon_rejects_non_integer_results() {
        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", "return \"text\"\n", "lua_node").expect("specialize");
        let error = emit_native_node_addon(&residual).expect_err("字符串结果必须失败闭合");
        assert!(error.contains("Node"), "unexpected error: {error}");
    }
}

/// CPython 扩展 lane：`python3` 真实 `import` 产物并读取属性。
#[cfg(target_os = "linux")]
mod python_extension_abi {
    use std::path::PathBuf;

    use legend_vm::compiler::emit_native_python_extension;

    fn extension(source: &str, name: &str) -> (PathBuf, PathBuf) {
        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", source, "lua_py").expect("specialize");
        let bytes = emit_native_python_extension(&residual, name).expect("emit python extension");
        let dir = std::env::temp_dir().join(format!("legend-python-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let library = dir.join(format!("{name}.so"));
        std::fs::write(&library, bytes).expect("write extension");
        (dir, library)
    }

    fn run_python(dir: &std::path::Path, script: &str) -> String {
        let output = std::process::Command::new("python3")
            .arg("-c")
            .arg(format!("import sys; sys.path.insert(0, '{}'); {script}", dir.display()))
            .output()
            .expect("spawn python3");
        assert!(
            output.status.success(),
            "python3 failed: status={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim_end().to_string()
    }

    #[test]
    fn python_imports_the_extension_and_reads_the_residual_value() {
        let (dir, _library) = extension("return 42\n", "legend_probe");
        assert_eq!(run_python(&dir, "import legend_probe; print(legend_probe.value)"), "42");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 诊断：直接经 `ctypes` 调用 `PyInit_<name>`，报告返回值类型。
    #[test]
    fn python_extension_init_returns_a_module_object() {
        let (dir, library) = extension("return 42\n", "legend_probe_diag");
        let script = format!(
            "import ctypes; lib=ctypes.CDLL('{}', mode=ctypes.RTLD_GLOBAL); lib.PyInit_legend_probe_diag.restype=ctypes.c_void_p; pointer=lib.PyInit_legend_probe_diag(); print('ptr', pointer); print('type', type(ctypes.cast(pointer, ctypes.py_object).value).__name__ if pointer else 'null')",
            library.display()
        );
        let output = std::process::Command::new("python3").arg("-c").arg(&script).output().expect("spawn python3");
        let text = format!(
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(text.contains("type module"), "PyInit 必须返回 module 对象：{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn python_extension_prints_through_stdout() {
        let (dir, _library) = extension("print(\"hello\")\nprint(3)\nreturn 7\n", "legend_print");
        assert_eq!(
            run_python(&dir, "import legend_print; print(legend_print.value)"),
            "hello\n3\n7",
            "print 顺序与模块属性共用 stdout"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn python_extension_rejects_string_results() {
        let runner = legend_vm::LegendVmRunner::new();
        let residual = runner.specialize_native("lua", "return \"text\"\n", "lua_py").expect("specialize");
        let error = emit_native_python_extension(&residual, "legend_bad").expect_err("字符串结果必须失败闭合");
        assert!(error.contains("CPython"), "unexpected error: {error}");
    }
}
