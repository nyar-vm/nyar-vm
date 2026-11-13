//! Futamura / partial-evaluation pipeline proofs.
//!
//! Host-script PE product path: specialize(interpret, program) → **native residual**
//! → Windows PE (not OVM). `.odin` / [`OdinVm`] remain probe-only for JS/Python stubs.

use std::collections::HashMap;

use legend_vm::{
    LegacyValue, LegendVmRunner,
    compiler::{CompileArtifact, ResidualValue, eval_native_residual},
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

fn legacy_key(value: &LegacyValue) -> String {
    value.to_string_value()
}
