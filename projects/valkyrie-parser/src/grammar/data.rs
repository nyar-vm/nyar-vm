use super::*;

impl ParsingContext {
    pub(crate) fn parse_data(&self, pairs: Pair<Rule>) -> ASTNode {
        let pair = pairs.into_inner().nth(0).unwrap();
        match pair.as_rule() {
            Rule::string => self.parse_string(pair),
            Rule::Special => self.parse_special(pair),
            Rule::Number => self.parse_number(pair),
            Rule::Byte => self.parse_byte(pair),
            Rule::symbol => self.parse_symbol(pair),
            Rule::list => self.parse_list_or_tuple(pair, true),
            Rule::dict => self.parse_dict(pair),
            _ => debug_cases!(pair),
        }
    }

    fn parse_dict(&self, pairs: Pair<Rule>) -> ASTNode {
        let mut vec: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE => continue,
                Rule::expr => vec.push(self.parse_expr(pair)),
                Rule::key_value => {}
                _ => debug_cases!(pair),
            };
        }
        return ASTNode::default();
    }

    fn parse_kv(&self, pairs: Pair<Rule>) -> ASTNode {
        todo!()
        // let (mut k, mut v) = (ASTNode::default(), ASTNode::default());
        // for pair in pairs.into_inner() {
        //     match pair.as_rule() {
        //         Rule::WHITESPACE | Rule::Colon => continue,
        //         Rule::symbol => k = self.parse_symbol(pair),
        //         Rule::expr => v = self.parse_expr(pair),
        //         _ => debug_cases!(pair),
        //     };
        // }
        // match k.kind {
        //     ASTKind::Nothing => k,
        //     _ => ASTNode::kv_pair(k, v),
        // }
    }

    fn parse_list_or_tuple(&self, pairs: Pair<Rule>, is_list: bool) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut vec: Vec<ASTNode> = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::WHITESPACE | Rule::Comma => continue,
                Rule::expr => vec.push(self.parse_expr(pair)),
                _ => unreachable!(),
            };
        }
        match is_list {
            true => ASTNode::list(vec, r),
            false => ASTNode::tuple(vec, r),
        }
    }

    fn parse_number(&self, pairs: Pair<Rule>) -> ASTNode {
        todo!()
        // let r = self.get_span(&pairs);
        // let (mut h, mut t, mut i) = Default::default();
        // for pair in pairs.into_inner() {
        //     match pair.as_rule() {
        //         Rule::Integer => {
        //             i = true;
        //             t = pair.as_str().to_string();
        //         }
        //         Rule::Decimal => {
        //             i = false;
        //             t = pair.as_str().to_string();
        //         }
        //         // Rule::DecimalBad => {
        //         //     // h = "dec";
        //         //     let s = pair.as_str();
        //         //     if s.starts_with('.') { t = "0".to_string() + s } else { t = s.to_string() + "0" }
        //         // }
        //         Rule::Complex => {
        //             for inner in pair.into_inner() {
        //                 match inner.as_rule() {
        //                     Rule::Integer => {
        //                         i = true;
        //                         t = inner.as_str().to_string()
        //                     }
        //                     Rule::Decimal => {
        //                         i = false;
        //                         t = inner.as_str().to_string()
        //                     }
        //                     Rule::SYMBOL => h = inner.as_str(),
        //                     _ => unreachable!(),
        //                 };
        //             }
        //         }
        //         _ => unreachable!(),
        //     };
        // }
        // ASTNode::number(h, t.as_str(), i, r)
    }

    fn parse_byte(&self, pairs: Pair<Rule>) -> ASTNode {
        todo!()
        // let r = self.get_span(&pairs);
        // let s = pairs.as_str();
        // let t = &s[2..s.len()];
        // let h = s.chars().nth(1).unwrap();
        // ASTNode::bytes(h, t, r)
    }

    fn parse_special(&self, pairs: Pair<Rule>) -> ASTNode {
        todo!()
        // let r = self.get_span(&pairs);
        // let pair = pairs.into_inner().nth(0).unwrap();
        // match pair.as_rule() {
        //     Rule::True => ASTNode::boolean(true, r),
        //     Rule::False => ASTNode::boolean(false, r),
        //     Rule::Null => ASTNode::null(r),
        //     _ => unreachable!(),
        // }
    }

    fn parse_symbol(&self, pairs: Pair<Rule>) -> ASTNode {
        todo!()
    }
    //     let r = self.get_span(&pairs);
    //     let mut scope = vec![];
    //     match pairs.as_rule() {
    //         Rule::SYMBOL => scope.push(pairs.as_str().to_string()),
    //         _ => {
    //             for pair in pairs.into_inner() {
    //                 match pair.as_rule() {
    //                     Rule::SYMBOL => scope.push(pair.as_str().to_string()),
    //                     Rule::namespace => {
    //                         for inner in pair.into_inner() {
    //                             match inner.as_rule() {
    //                                 Rule::Proportion => (),
    //                                 Rule::SYMBOL => scope.push(inner.as_str().to_string()),
    //                                 _ => unreachable!(),
    //                             };
    //                         }
    //                     }
    //                     _ => unreachable!(),
    //                 };
    //             }
    //         }
    //     }
    //     ASTNode::symbol(&scope, r)
    // }
}

impl ParsingContext {
    fn parse_string(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut handler = "";
        // let (mut h, mut t) = Default::default();
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::StringTemplate => match handler.is_empty() {
                    true => self.parse_string_template(pair),
                    false => self.parse_string_raw(pair, handler),
                },
                _ => debug_cases!(pair), // _ => unreachable!(),
            };
        }

        todo!()
        // ASTNode::string(t, r)
    }
    fn parse_string_raw(&self, pairs: Pair<Rule>, handler: &str) -> ASTNode {
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Quotation => continue,
                Rule::Apostrophe => continue,
                Rule::StringItem => self.parse_string_item(pair),
                // Rule::SYMBOL => h = pair.as_str(),
                // Rule::StringEmpty => continue,
                // Rule::StringNormal => {
                //     for inner in pair.into_inner() {
                //         match inner.as_rule() {
                //             Rule::StringText => t = unescape(inner.as_str()),
                //             _ => continue,
                //         };
                //     }
                // }
                // Rule::StringLiteral => {
                //     for inner in pair.into_inner() {
                //         match inner.as_rule() {
                //             Rule::StringLiteralText => t = unescape(inner.as_str()),
                //             _ => continue,
                //         };
                //     }
                // }
                _ => debug_cases!(pair), // _ => unreachable!(),
            };
        }
        todo!()
    }
    fn parse_string_template(&self, pairs: Pair<Rule>) -> ASTNode {
        let r = self.get_span(&pairs);
        let mut items = vec![];
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                Rule::Quotation => continue,
                Rule::Apostrophe => continue,
                Rule::StringItem => items.push(self.parse_string_item(pair)),
                // Rule::SYMBOL => h = pair.as_str(),
                // Rule::StringEmpty => continue,
                // Rule::StringNormal => {
                //     for inner in pair.into_inner() {
                //         match inner.as_rule() {
                //             Rule::StringText => t = unescape(inner.as_str()),
                //             _ => continue,
                //         };
                //     }
                // }
                // Rule::StringLiteral => {
                //     for inner in pair.into_inner() {
                //         match inner.as_rule() {
                //             Rule::StringLiteralText => t = unescape(inner.as_str()),
                //             _ => continue,
                //         };
                //     }
                // }
                _ => debug_cases!(pair), // _ => unreachable!(),
            };
        }
        ASTNode::string_template(items, r)
    }
    fn parse_string_item(&self, pairs: Pair<Rule>) -> ASTNode {
        println!("{:?}", pairs.as_str());
        for pair in pairs.into_inner() {
            match pair.as_rule() {
                // Rule::SYMBOL => h = pair.as_str(),
                // Rule::StringEmpty => continue,
                // Rule::StringNormal => {
                //     for inner in pair.into_inner() {
                //         match inner.as_rule() {
                //             Rule::StringText => t = unescape(inner.as_str()),
                //             _ => continue,
                //         };
                //     }
                // }
                // Rule::StringLiteral => {
                //     for inner in pair.into_inner() {
                //         match inner.as_rule() {
                //             Rule::StringLiteralText => t = unescape(inner.as_str()),
                //             _ => continue,
                //         };
                //     }
                // }
                _ => debug_cases!(pair), // _ => unreachable!(),
            };
        }
        todo!()
    }
}
