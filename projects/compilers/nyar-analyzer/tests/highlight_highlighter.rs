struct LexicalStub;
impl Highlighter for LexicalStub {
    fn language_id(&self) -> &str {
        "demo"
    }
    fn kind(&self) -> HighlighterKind {
        HighlighterKind::Lexical
    }
    fn highlight(&self, request: &HighlightRequest<'_>) -> Vec<HighlightSpan> {
        vec![HighlightSpan::new(HighlightKind::Identifier, 0..request.source.len().min(1))]
    }
}

struct SemanticStub;
impl Highlighter for SemanticStub {
    fn language_id(&self) -> &str {
        "demo"
    }
    fn kind(&self) -> HighlighterKind {
        HighlighterKind::Semantic
    }
    fn highlight(&self, request: &HighlightRequest<'_>) -> Vec<HighlightSpan> {
        if request.analysis.is_none() {
            return Vec::new();
        }
        vec![HighlightSpan::new(HighlightKind::TypeIdentifier, 0..1)]
    }
}

struct DemoProvider;
impl HighlighterProvider for DemoProvider {
    fn language_ids(&self) -> &[&str] {
        &["demo", "dm"]
    }
    fn highlighters(&self) -> Vec<Box<dyn Highlighter>> {
        vec![Box::new(LexicalStub), Box::new(SemanticStub)]
    }
}

#[test]
fn registry_registers_both_kinds() {
    let mut reg = HighlighterRegistry::new();
    reg.register_provider(&DemoProvider);
    assert!(reg.get("demo", HighlighterKind::Lexical).is_some());
    assert!(reg.get("dm", HighlighterKind::Semantic).is_some());
}

#[test]
fn resolve_falls_back_to_lexical_without_analysis() {
    let mut reg = HighlighterRegistry::new();
    reg.register_provider(&DemoProvider);
    let h = reg.resolve("demo", HighlighterKind::Semantic, false).unwrap();
    assert_eq!(h.kind(), HighlighterKind::Lexical);
}

#[test]
fn merge_overlays_semantic_on_lexical() {
    let mut reg = HighlighterRegistry::new();
    reg.register_provider(&DemoProvider);
    struct Ctx;
    impl AnalysisContext for Ctx {
        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    let ctx = Ctx;
    let request = HighlightRequest::with_analysis("X", &ctx);
    let spans = reg.highlight_merged("demo", &request).unwrap();
    assert!(spans.iter().any(|s| s.kind == HighlightKind::TypeIdentifier));
}
