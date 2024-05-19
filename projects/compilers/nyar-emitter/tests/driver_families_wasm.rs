use nyar_emitter::*;

/// 验证 `wasm_physical_artifact_name` 对任意项目名原样返回，无特殊分支。
#[test]
fn physical_name_preserves_artifact_names() {
    assert_eq!(wasm_physical_artifact_name("demo"), "demo");
    assert_eq!(wasm_physical_artifact_name("legion"), "legion");
    assert_eq!(wasm_physical_artifact_name("legion.tools"), "legion.tools");
    assert_eq!(wasm_physical_artifact_name("main_legion"), "main_legion");
    assert_eq!(wasm_physical_artifact_name("wasm_interop_stdout"), "wasm_interop_stdout");
    assert_eq!(wasm_physical_artifact_name("alpha_entry__main_alpha_entry"), "alpha_entry__main_alpha_entry");
}
