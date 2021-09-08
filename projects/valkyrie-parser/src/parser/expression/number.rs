use std::str::FromStr;

use valkyrie_ast::{ValkyrieIdentifierNode, ValkyrieIntegerNode};
use valkyrie_errors::BigInt;

use crate::parser::valkyrie::{NumberNode, NumberVariant};

use super::*;

impl NumberNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieASTNode {
        let hint = match &self.hint {
            Some(s) => s.visit(parser),
            None => ValkyrieIdentifierNode::default(),
        };
        match &self.variant {
            NumberVariant::IntegerNode(s) => match BigInt::from_str(&s.string) {
                Ok(o) => ValkyrieIntegerNode { hint, value: o }.to_node(parser.file, &s.position),
                Err(e) => {
                    parser.push_error(e.to_string(), &s.position);
                    parser.bad_node(&s.position)
                }
            },
            NumberVariant::DecimalNode(v) => {
                println!("DecimalNode: {:?}", v);
                todo!()
            }
            NumberVariant::ByteNode(v) => {
                println!("ByteNode: {:?}", v);
                todo!()
            }
        }
    }
}
