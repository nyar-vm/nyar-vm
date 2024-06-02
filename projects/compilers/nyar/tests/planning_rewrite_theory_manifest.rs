use nyar::{planning::rewrite_theory_manifest::RewriteTheoryManifestV1, *};

#[test]
fn manifest_pattern_rule_produces_term_rewrite() {
    let manifest = RewriteTheoryManifestV1::from_json(
        r#"{
                "fragment": "graphic",
                "rules": [
                    {
                        "name": "add-identity",
                        "left": "core.add(?x, 0)",
                        "right": "?x",
                        "phase": "local",
                        "capabilities": []
                    }
                ]
            }"#,
    )
    .expect("manifest");
    let theory = manifest.to_rewrite_theory();
    assert_eq!(theory.term_rewrites.len(), 1);
    assert!(theory.equations.is_empty());
    assert_eq!(theory.term_rewrites[0].name.as_str(), "add-identity");
}

#[test]
fn manifest_flat_rule_still_produces_equation() {
    let theory = builtin_graphic_manifest().to_rewrite_theory();
    assert!(!theory.equations.is_empty());
}
