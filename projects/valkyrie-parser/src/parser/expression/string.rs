use super::*;

impl StringNode {
    pub fn visit(&self, parser: &mut ValkyrieParser) -> ValkyrieResult<ValkyrieASTNode> {
        let hint = match &self.hint {
            Some(s) => Some(s.visit(parser)),
            None => None,
        };
        let mut pure_string = true;
        let mut buffer = String::new();
        for item in &self.raw.item {
            item.visit(parser, &mut buffer, &mut pure_string)?
        }
        match pure_string {
            true => {
                let string = ValkyrieASTNode::string(buffer, parser.file, &self.raw.position);
                match hint {
                    Some(s) => Ok(ValkyrieASTNode::string_interpolation(vec![string], s, parser.file, &self.raw.position)),
                    None => Ok(string),
                }
            }
            false => Ok(ValkyrieASTNode::string(buffer, parser.file, &self.raw.position)),
        }
    }
}

impl StringItem {
    pub fn visit(&self, parser: &mut ValkyrieParser, s_buffer: &mut String, pure_flag: &mut bool) -> ValkyrieResult {
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
        }
        Ok(())
    }
}
