//! Kotlin 到 Nyar 字节码的翻译器

use nyar_vm::NyarError;
use chomsky_uir::{IntentBuilder, IKunTree, EGraph, IKun, ConstraintAnalysis, Id};
use chomsky_extract::IKunExtractor;
use chomsky_cost::DEFAULT_COST_MODEL;
use oak_kotlin::ast::*;
use oak_kotlin::kind::KotlinSyntaxKind;
use oak_core::{GreenNode, GreenTree, Language};
use oak_kotlin::language::KotlinLanguage;

pub struct NyarTranslator;

impl NyarTranslator {
    pub fn new() -> Self {
        Self
    }

    pub fn translate_to_tree(&self, root: &KotlinRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = self.translate_to_graph(root, &mut egraph)?;
        let extractor = IKunExtractor::new(&egraph, &DEFAULT_COST_MODEL);
        Ok(extractor.extract(root_id))
    }

    pub fn translate_to_graph(&self, root: &KotlinRoot, egraph: &mut EGraph<IKun, ConstraintAnalysis>) -> Result<Id, NyarError> {
        let mut builder = IntentBuilder::new(egraph);
        
        if let Some(green) = &root.green {
            let mut context = TranslationContext {
                builder: &mut builder,
                source: &root.source,
                offset: 0,
            };
            let id = context.translate_node(green)?;
            context.builder.set_root(id);
            Ok(id)
        } else {
            // Fallback for empty/invalid tree
            let hello_str = builder.string("Hello from Mini Kotlin!");
            let print_sym = builder.symbol("println");
            let call = builder.extension("call", vec![print_sym, hello_str]);
            let main_body = builder.seq(vec![call]);
            let main_method = builder.extension("method", vec![
                builder.string("main"),
                builder.string("void"),
                builder.seq(vec![]), // params
                main_body
            ]);
            let class_members = builder.seq(vec![main_method]);
            let class_node = builder.extension("class", vec![
                builder.string("MainKt"),
                class_members
            ]);
            let root_id = builder.seq(vec![class_node]);
            builder.set_root(root_id);
            Ok(root_id)
        }
    }
}

struct TranslationContext<'a> {
    builder: &'a mut IntentBuilder<'a>,
    source: &'a str,
    offset: usize,
}

impl<'a> TranslationContext<'a> {
    fn translate_node(&mut self, node: &GreenNode<'static, KotlinLanguage>) -> Result<Id, NyarError> {
        let kind: KotlinSyntaxKind = node.kind.into();
        let old_offset = self.offset;
        
        let result = match kind {
            KotlinSyntaxKind::SourceFile => {
                let mut members = Vec::new();
                for child in node.children() {
                    if let GreenTree::Node(child_node) = child {
                        members.push(self.translate_node(child_node)?);
                    } else {
                        self.offset += child.len() as usize;
                    }
                }
                Ok(self.builder.seq(members))
            }
            KotlinSyntaxKind::ClassDeclaration => {
                let mut name = "Anonymous".to_string();
                let mut members = Vec::new();
                for child in node.children() {
                    match child {
                        GreenTree::Node(child_node) => {
                            let child_kind: KotlinSyntaxKind = child_node.kind.into();
                            if child_kind == KotlinSyntaxKind::Block {
                                for member in child_node.children() {
                                    if let GreenTree::Node(m_node) = member {
                                        members.push(self.translate_node(m_node)?);
                                    } else {
                                        self.offset += member.len() as usize;
                                    }
                                }
                            } else {
                                members.push(self.translate_node(child_node)?);
                            }
                        }
                        GreenTree::Leaf(leaf) => {
                            let leaf_kind: KotlinSyntaxKind = leaf.kind.into();
                            if leaf_kind == KotlinSyntaxKind::Identifier {
                                name = self.source[self.offset..self.offset + leaf.length as usize].to_string();
                            }
                            self.offset += leaf.length as usize;
                        }
                    }
                }
                let class_members = self.builder.seq(members);
                Ok(self.builder.extension("class", vec![
                    self.builder.string(name),
                    class_members
                ]))
            }
            KotlinSyntaxKind::FunctionDeclaration => {
                let mut name = "anonymous".to_string();
                let mut body = self.builder.seq(vec![]);
                let mut params = Vec::new();

                for child in node.children() {
                    match child {
                        GreenTree::Node(child_node) => {
                            let child_kind: KotlinSyntaxKind = child_node.kind.into();
                            match child_kind {
                                KotlinSyntaxKind::Block => {
                                    body = self.translate_node(child_node)?;
                                }
                                KotlinSyntaxKind::Parameter => {
                                    params.push(self.translate_node(child_node)?);
                                }
                                _ => {
                                    self.translate_node(child_node)?;
                                }
                            }
                        }
                        GreenTree::Leaf(leaf) => {
                            let leaf_kind: KotlinSyntaxKind = leaf.kind.into();
                            if leaf_kind == KotlinSyntaxKind::Identifier {
                                name = self.source[self.offset..self.offset + leaf.length as usize].to_string();
                            }
                            self.offset += leaf.length as usize;
                        }
                    }
                }
                
                let params_seq = self.builder.seq(params);
                Ok(self.builder.extension("method", vec![
                    self.builder.string(name),
                    self.builder.string("void"), // TODO: proper return type
                    params_seq,
                    body
                ]))
            }
            KotlinSyntaxKind::Block => {
                let mut stmts = Vec::new();
                for child in node.children() {
                    if let GreenTree::Node(child_node) = child {
                        stmts.push(self.translate_node(child_node)?);
                    } else {
                        self.offset += child.len() as usize;
                    }
                }
                Ok(self.builder.seq(stmts))
            }
            KotlinSyntaxKind::Parameter => {
                let mut name = "p".to_string();
                for child in node.children() {
                    if let GreenTree::Leaf(leaf) = child {
                        let leaf_kind: KotlinSyntaxKind = leaf.kind.into();
                        if leaf_kind == KotlinSyntaxKind::Identifier {
                            name = self.source[self.offset..self.offset + leaf.length as usize].to_string();
                        }
                        self.offset += leaf.length as usize;
                    } else if let GreenTree::Node(n) = child {
                        self.translate_node(n)?;
                    }
                }
                Ok(self.builder.extension("parameter", vec![
                    self.builder.string(name),
                    self.builder.string("Any"), // TODO: proper type
                ]))
            }
            _ => {
                // For other nodes, just advance offset and return a placeholder
                for child in node.children() {
                    match child {
                        GreenTree::Node(child_node) => {
                            self.translate_node(child_node)?;
                        }
                        GreenTree::Leaf(leaf) => {
                            self.offset += leaf.length as usize;
                        }
                    }
                }
                Ok(self.builder.seq(vec![]))
            }
        };
        
        self.offset = old_offset + node.text_len as usize;
        result
    }
}
