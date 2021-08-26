use super::*;

impl ParsingContext {
    pub fn parse_match(&mut self, pairs: &Token) -> Result<ASTNode> {
        for pair in pairs {
            match pair.rule {
                Rule::MATCH => continue,
                Rule::WITH => continue,
                Rule::CASE => continue,
                Rule::case_statement => {
                    self.case_statement(&pair)?;
                }
                Rule::effect_type => continue,
                Rule::expr_inline => {
                    self.parse_expr(&pair)?;
                }
                _ => pair.debug_cases()?,
            };
        }
        todo!()
    }
    fn case_statement(&mut self, pairs: &Token) -> Result<ASTNode> {
        for pair in pairs {
            match pair.rule {
                Rule::CASE => continue,
                Rule::case_item => {
                    self.case_item(&pair)?;
                }
                _ => pair.debug_cases()?,
            };
        }
        todo!()
    }
    fn case_item(&mut self, pairs: &Token) -> Result<ASTNode> {
        for pair in pairs {
            match pair.rule {
                Rule::Symbol => {
                    self.parse_symbol(&pair);
                }
                _ => pair.debug_cases()?,
            };
        }
        todo!()
    }
}
