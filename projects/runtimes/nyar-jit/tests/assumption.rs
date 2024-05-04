use nyar_jit::*;

#[test]
fn baseline_assumptions_include_version() {
    let list = baseline_scalar_assumptions(3);
    assert!(list.contains(&JitAssumption::ScalarLeafShape));
    assert!(list.contains(&JitAssumption::ModuleVersion(3)));
}
