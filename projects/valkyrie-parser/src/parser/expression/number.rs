use std::str::FromStr;

use valkyrie_ast::{ValkyrieDecimalNode, ValkyrieIdentifierNode, ValkyrieIntegerNode};
use valkyrie_errors::{FBig, IBig};

use crate::parser::valkyrie::{NumberNode, NumberVariant};

use super::*;

impl NumberNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieASTNode {
        let hint = match &self.hint {
            Some(s) => s.visit(parser),
            None => ValkyrieIdentifierNode::default(),
        };
        match &self.variant {
            NumberVariant::IntegerNode(v) => match IBig::from_str(&v.string) {
                Ok(o) => ValkyrieIntegerNode { hint, value: o }.to_node(parser.file, &v.position),
                Err(e) => {
                    parser.push_error(e.to_string(), &v.position);
                    parser.bad_node(&v.position)
                }
            },
            NumberVariant::DecimalNode(v) => match FBig::from_str(&v.string) {
                Ok(o) => ValkyrieDecimalNode { hint, value: o }.to_node(parser.file, &v.position),
                Err(e) => {
                    parser.push_error(e.to_string(), &v.position);
                    parser.bad_node(&v.position)
                }
            }
            NumberVariant::ByteNode(v) => {
                println!("ByteNode: {:?}", v);
                todo!()
            }
        }
    }
}
