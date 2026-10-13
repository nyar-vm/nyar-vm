use super::*;

#[test]
fn explicit_impl_for_non_struct_type_supplies_where_evidence() {
    let compiler = crate::ValkyrieCompiler::default();
    let module = compiler.compile_source(
        "trait Display { micro display(self): utf8 } imply utf8: Display { micro display(self): utf8 { return self } } micro present<T>(value: T): utf8 where T: Display { return \"ok\" } micro caller(value: utf8): utf8 { return present(value) }"
    ).expect("primitive explicit impl must satisfy generic bound");
    assert_eq!(module.functions.iter().find(|f| f.name.as_str() == "present").unwrap().where_constraints.len(), 1);
}

#[test]
fn absent_explicit_impl_for_non_struct_type_is_rejected() {
    let compiler = crate::ValkyrieCompiler::default();
    assert!(compiler.compile_source(
        "trait Display { micro display(self): utf8 } micro present<T>(value: T): utf8 where T: Display { return \"ok\" } micro caller(value: utf8): utf8 { return present(value) }"
    ).is_err());
}
