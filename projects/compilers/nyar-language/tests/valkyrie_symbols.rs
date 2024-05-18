use nyar_language::{
    types::{
        Identifier, NamePath, SourceID, SourceSpan,
        hir::{HirBlock, HirDocumentation, HirFunction, HirVisibility, ValkyrieType},
    },
    valkyrie::symbols::{stable_hir_function_name_path, stable_hir_function_symbol},
};

#[test]
fn stable_hir_function_name_path_does_not_recurse() {
    let module = NamePath::new(vec![Identifier::new("main")]);
    let function = HirFunction {
        declaration: None,
        instance: None,
        name: Identifier::new("answer"),
        where_constraints: Vec::new(),
        declaring_namespace: NamePath::default(),
        doc: HirDocumentation::default(),
        annotations: Vec::new(),
        generics: Vec::new(),
        params: Vec::new(),
        return_type: ValkyrieType::Unit,
        body: HirBlock { statements: Vec::new(), expr: None, span: SourceSpan::new(SourceID::default(), 0, 0) },
        span: SourceSpan::new(SourceID::default(), 0, 0),
        visibility: HirVisibility::default(),
        is_abstract: false,
        is_final: false,
        is_virtual: false,
        is_override: false,
    };
    assert_eq!(stable_hir_function_symbol(&module, &function), "main::answer");
    assert_eq!(stable_hir_function_name_path(&module, &function).to_string(), "main::answer");
}
