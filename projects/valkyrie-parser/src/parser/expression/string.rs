use std::mem::take;

use super::*;

impl StringNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let hint = match &self.hint {
            Some(s) => Some(s.visit(parser)),
            None => None,
        };
        let mut t_buffer = vec![];
        let mut s_buffer = String::new();
        for item in &self.raw.item {
            item.visit(parser, &mut t_buffer, &mut s_buffer)?
        }
        match t_buffer.is_empty() {
            true => {
                let string = ValkyrieASTNode::string(s_buffer, parser.file, &self.raw.position);
                match hint {
                    Some(s) => Ok(ValkyrieASTNode::string_interpolation(vec![string], s, parser.file, &self.raw.position)),
                    None => Ok(string),
                }
            }
            false => {
                let text = ValkyrieASTNode::string(take(&mut s_buffer), parser.file, &Range::default());
                t_buffer.push(text);
                Ok(ValkyrieASTNode::string_interpolation(t_buffer, hint.unwrap_or_default(), parser.file, &self.raw.position))
            }
        }
    }
}

impl StringItem {
    pub fn visit(
        &self,
        parser: &mut ValkyrieParser,
        t_buffer: &mut Vec<ValkyrieASTNode>,
        s_buffer: &mut String,
    ) -> ValkyrieResult {
        match self {
            StringItem::ESCAPE_U(c) => {
                let str = c.hex.iter().collect::<String>();
                let idx = u32::from_str_radix(&str, 16)?;
                match char::from_u32(idx) {
                    Some(s) => s_buffer.push(s),
                    None => Err(ValkyrieError::syntax_error(
                        "Out of range unicode codepoint",
                        FileSpan::new(parser.file, &c.position),
                    ))?,
                }
            }
            StringItem::ESCAPE_C(c) => {
                s_buffer.push(c.char);
            }
            StringItem::StringAny(c) => {
                s_buffer.push(*c);
            }
            StringItem::STRING_T(v) => {
                let text = ValkyrieASTNode::string(take(s_buffer), parser.file, &Range::default());
                t_buffer.push(text);
                t_buffer.push(v.expr.visit(parser)?);
            }
        }
        Ok(())
    }
}
