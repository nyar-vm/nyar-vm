#![cfg(false)] // legacy: architecture refactor (TargetExecutionPlan) — excluded until ported
//! Valkyrie AST pretty printer（先注册前端 provider，再走平台注册表）。

use nyar_format::{
    formatter::{FormatOptions, PrintStyle},
    printer::print_document,
};
use oak_valkyrie::printer::{
    PrintOptions as OakPrintOptions, PrintStyle as OakPrintStyle, render_document as oak_render_document, to_document as oak_to_document,
};
use rusty_valkyrie::{compiler::syntax::oak::parse_source, text::ToDocument};

fn oak_options(style: PrintStyle, options: &FormatOptions) -> OakPrintOptions {
    OakPrintOptions {
        style: match style {
            PrintStyle::Compact => OakPrintStyle::Compact,
            PrintStyle::Indented => OakPrintStyle::Indented,
        },
        indent_width: options.indent_width,
        max_width: options.max_width,
    }
}

#[test]
fn valkyrie_source_print_via_oak() {
    let out = rusty_valkyrie::format_provider::print_valkyrie_source("micro main(){let x=1}", PrintStyle::Compact, &FormatOptions::default())
        .unwrap();
    assert!(out.contains("micro main()"));
    assert!(out.contains("let x=1"));
}

#[test]
fn valkyrie_ast_print_produces_document() {
    rusty_valkyrie::register_format_providers();
    let options = FormatOptions::default();
    let root = parse_source("micro main(){let x=1}").expect("parse");
    let doc = oak_to_document(&root).expect("to_document");
    let compact = oak_render_document(&doc, &oak_options(PrintStyle::Compact, &options));
    assert!(compact.contains("micro main()"));
    let via_trait = root.to_document().expect("trait to_document");
    let again = oak_render_document(&via_trait, &oak_options(PrintStyle::Compact, &options));
    assert_eq!(compact, again);
    let via_registry = print_document("v", &root, PrintStyle::Compact, &options).unwrap();
    assert_eq!(compact, via_registry);
}
