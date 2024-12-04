use nyar_types::*;

#[test]
fn attribute_registry_interns_custom_names() {
    let mut registry = AttributeRegistry::with_builtins();
    assert_eq!(registry.lookup("export"), Some(builtin_attribute::export()));
    assert_eq!(registry.lookup("workload_phase"), Some(builtin_attribute::workload_phase()));
    let custom = registry.intern("my_attr");
    assert_eq!(registry.lookup("my_attr"), Some(custom));
    assert_eq!(registry.intern("my_attr"), custom);
    assert!(matches!(registry.intern_unique("export"), Err(AttributeRegistryError::DuplicateName { .. })));
}
