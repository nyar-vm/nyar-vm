use nyar_emitter::backend::wasi::canonical_wasm_artifact_stem;

#[test]
fn artifact_stem_preserves_configured_name_without_project_special_cases() {
    for name in ["demo", "legion", "legion.tools", "legion_tools", "another.library"] {
        assert_eq!(canonical_wasm_artifact_stem(name), name);
    }
}
