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

#[test]
fn operator_registry_seeds_and_rejects_duplicates() {
    let mut registry = OperatorRegistry::with_builtins();
    assert_eq!(registry.lookup_display_name("infix =="), Some(builtin_operator::infix_eq()));
    assert_eq!(registry.lookup_display_name("prefix !"), Some(builtin_operator::prefix_not()));
    assert!(matches!(registry.intern_unique(OperatorFixity::Infix, "==", 4, None), Err(OperatorRegistryError::DuplicateKey { .. })));
    let custom = registry.intern_unique(OperatorFixity::Infix, "+*", 7, None).expect("user op");
    assert_eq!(registry.lookup(OperatorFixity::Infix, "+*"), Some(custom));
}
