use nyar_language::{
    types::{
        Identifier, NamePath, SourceID, SourceSpan,
        hir::{
            HirBlock, HirDocumentation, HirExpr, HirExprKind, HirFunction, HirLiteral, HirModule, HirStringLiteral, HirStringSegment,
            HirStruct, HirVisibility, ValkyrieType,
        },
    },
    valkyrie::types::hir::{HirArgument, HirAttribute},
    *,
};

fn test_span() -> SourceSpan {
    SourceSpan::new(SourceID::default(), 0, 0)
}

fn empty_module(structs: Vec<HirStruct>) -> HirModule {
    HirModule {
        name: NamePath::new(vec![Identifier::new("test")]),
        doc: HirDocumentation::default(),
        imports: vec![],
        warnings: Vec::new(),
        submodules: vec![],
        functions: vec![],
        structs,
        enums: vec![],
        flags: vec![],
        traits: vec![],
        impls: vec![],
        type_functions: vec![],
        type_families: vec![],
        widgets: vec![],
        singletons: vec![],
        statements: vec![],
        type_aliases: vec![],
    }
}

fn route_attr(method: &str, path: &str) -> HirAttribute {
    HirAttribute::with_arguments(
        NamePath::new(vec![Identifier::new(method)]),
        vec![HirArgument {
            key: None,
            value: Box::new(HirExpr {
                kind: HirExprKind::Literal(HirLiteral::String(HirStringLiteral {
                    prefix: None,
                    quote_count: 1,
                    segments: vec![HirStringSegment::Text(path.to_string())],
                })),
                span: test_span(),
            }),
        }],
    )
}

fn health_controller() -> HirStruct {
    HirStruct {
        name: Identifier::new("HealthController"),
        methods: vec![HirFunction {
            declaration: None,
            instance: None,
            name: Identifier::new("get_health"),
            where_constraints: Vec::new(),
            declaring_namespace: NamePath::default(),
            doc: HirDocumentation::default(),
            annotations: vec![route_attr("get", "/api/health")],
            generics: vec![],
            params: vec![],
            return_type: ValkyrieType::Named(Identifier::new("AtlasResult")),
            body: HirBlock { statements: vec![], expr: None, span: test_span() },
            span: test_span(),
            visibility: HirVisibility::public(),
            is_abstract: false,
            is_final: false,
            is_virtual: false,
            is_override: false,
        }],
        ..Default::default()
    }
}

#[test]
fn route_prefix_from_class_doc() {
    let strukt = HirStruct {
        name: Identifier::new("OrdersController"),
        doc: HirDocumentation::from_lines(vec!["@route_prefix(\"/api\")".to_string()]),
        methods: vec![HirFunction {
            declaration: None,
            instance: None,
            name: Identifier::new("get_orders"),
            where_constraints: Vec::new(),
            declaring_namespace: NamePath::default(),
            doc: HirDocumentation::default(),
            annotations: vec![route_attr("get", "/orders")],
            generics: vec![],
            params: vec![],
            return_type: ValkyrieType::Named(Identifier::new("AtlasResult")),
            body: HirBlock { statements: vec![], expr: None, span: test_span() },
            span: test_span(),
            visibility: HirVisibility::public(),
            is_abstract: false,
            is_final: false,
            is_virtual: false,
            is_override: false,
        }],
        ..Default::default()
    };
    let module = empty_module(vec![strukt]);
    let routes = collect_atlas_routes(&module);
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].path, "/api/orders");
}

#[test]
fn collect_routes_from_controller() {
    let module = empty_module(vec![health_controller()]);
    let routes = collect_atlas_routes(&module);
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].http_method, "GET");
    assert_eq!(routes[0].path, "/api/health");
    assert_eq!(routes[0].handler_id, "health");
}

#[test]
fn generated_v_contains_register_and_invoke() {
    let module = empty_module(vec![health_controller()]);
    let result = inject_atlas_routes(&module);
    assert!(result.generated_v.contains("atlas_register_routes"));
    assert!(result.generated_v.contains("atlas_invoke_handler"));
    assert!(result.generated_v.contains("HealthController::wire"));
    assert!(result.generated_v.contains("get_route(\"/api/health\", \"health\")"));
}
