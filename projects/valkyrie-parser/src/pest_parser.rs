pub struct Valkyrie;
#[allow(dead_code, non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Rule {
    EOI,
    program,
    statement,
    emptyStatement,
    eos,
    comma_or_semi,
    Prefix,
    Postfix,
    Infix,
    Set,
    Or,
    LazyOr,
    Star,
    Slash,
    Proportion,
    Comma,
    Dot,
    Separate,
    Semicolon,
    Colon,
    Question,
    Underline,
    Import,
    Export,
    LeftShift,
    RightShift,
    LessEqual,
    GraterEqual,
    Less,
    Grater,
    Equivalent,
    NotEquivalent,
    Equal,
    NotEqual,
    o_add,
    Plus,
    Minus,
    o_times,
    Multiply,
    CenterDot,
    Kronecker,
    TensorProduct,
    Divide,
    Quotient,
    Modulo,
    Remainder,
    o_power,
    Power,
    Surd,
    Increase,
    Decrease,
    To,
    Elvis,
    Map,
    Quote,
    Acute,
    S1,
    S2,
    S3,
    S6,
    LogicOr,
    LogicAnd,
    LogicNot,
    Ellipsis,
    LogicXor,
    MapAll,
    Output,
    Concat,
    Destruct,
    DoubleBang,
    BitNot,
    Curry,
    Apply,
    LetAssign,
}
#[allow(clippy::all)]
impl ::pest::Parser<Rule> for Valkyrie {
    fn parse<'i>(
        rule: Rule,
        input: &'i str,
    ) -> ::std::result::Result<::pest::iterators::Pairs<'i, Rule>, ::pest::error::Error<Rule>> {
        mod rules {
            pub mod hidden {
                use super::super::Rule;
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn skip(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    Ok(state)
                }
            }
            pub mod visible {
                use super::super::Rule;
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn program(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.sequence(|state| {
                        self::SOI(state)
                            .and_then(|state| super::hidden::skip(state))
                            .and_then(|state| {
                                state.sequence(|state| {
                                    state.optional(|state| {
                                        self::statement(state).and_then(|state| {
                                            state.repeat(|state| {
                                                state.sequence(|state| {
                                                    super::hidden::skip(state)
                                                        .and_then(|state| self::statement(state))
                                                })
                                            })
                                        })
                                    })
                                })
                            })
                            .and_then(|state| super::hidden::skip(state))
                            .and_then(|state| self::EOI(state))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn statement(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::emptyStatement(state)
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn emptyStatement(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::emptyStatement, |state| {
                        self::eos(state).or_else(|state| self::Separate(state))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn eos(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::eos, |state| self::Semicolon(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn comma_or_semi(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Comma(state).or_else(|state| self::Semicolon(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Prefix(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Prefix, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            self::Plus(state)
                                .or_else(|state| self::Increase(state))
                                .or_else(|state| self::Decrease(state))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Postfix(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Postfix, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Infix(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Infix, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            self::o_add(state).or_else(|state| self::o_times(state))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Set(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Set, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("="))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Or(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Or, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("|"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LazyOr(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LazyOr, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("||"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Star(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Star, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("*"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Slash(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Slash, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("/"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Proportion(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Proportion, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string("::")
                                .or_else(|state| state.match_string("\u{2237}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Comma(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Comma, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string(",")
                                .or_else(|state| state.match_string("\u{ff0c}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Dot(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Dot, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("."))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Separate(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Separate, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(";;"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Semicolon(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Semicolon, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string(";")
                                .or_else(|state| state.match_string("\u{ff1b}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Colon(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Colon, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string(":")
                                .or_else(|state| state.match_string("\u{ff1a}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Question(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Question, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("?"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Underline(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Underline, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("_"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Import(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Import, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string("<<<")
                                .or_else(|state| state.match_string("\u{22d8}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Export(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Export, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string(">>>")
                                .or_else(|state| state.match_string("\u{22d9}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LeftShift(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LeftShift, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string("<<")
                                .or_else(|state| state.match_string("\u{226a}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn RightShift(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::RightShift, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string(">>")
                                .or_else(|state| state.match_string("\u{226b}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LessEqual(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LessEqual, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("<="))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn GraterEqual(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::GraterEqual, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(">="))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Less(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Less, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("<"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Grater(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Grater, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(">"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Equivalent(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Equivalent, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("==="))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn NotEquivalent(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::NotEquivalent, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("=!="))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Equal(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Equal, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("=="))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn NotEqual(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::NotEqual, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string("!=")
                                .or_else(|state| state.match_string("\u{2260}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn o_add(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Plus(state).or_else(|state| self::Minus(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Plus(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Plus, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("+"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Minus(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Minus, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("-"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn o_times(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Multiply(state)
                        .or_else(|state| self::CenterDot(state))
                        .or_else(|state| self::Kronecker(state))
                        .or_else(|state| self::TensorProduct(state))
                        .or_else(|state| self::Divide(state))
                        .or_else(|state| self::Quotient(state))
                        .or_else(|state| self::Modulo(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Multiply(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Multiply, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            self::Star(state).or_else(|state| state.match_string("\u{d7}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn CenterDot(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::CenterDot, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state.match_string("\u{22c5}")
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Kronecker(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Kronecker, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state.match_string("\u{2297}")
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn TensorProduct(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::TensorProduct, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state.match_string("\u{2299}")
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Divide(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Divide, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            self::Slash(state).or_else(|state| state.match_string("\u{f7}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Quotient(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Quotient, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("//"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Modulo(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Modulo, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("%"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Remainder(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Remainder, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state.match_string("\u{2052}")
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn o_power(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Power(state)
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Power(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Power, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("^"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Surd(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Surd, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state.match_string("\u{221a}")
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Increase(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Increase, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("++"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Decrease(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Decrease, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("--"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn To(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::To, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("->"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Elvis(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Elvis, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(":?"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Map(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Map, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("/@"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Quote(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Quote, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("`"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Acute(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Acute, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state.match_string("\u{b4}")
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn S1(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("'")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn S2(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("\"")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn S3(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("'''")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn S6(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("\"\"\"")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicOr(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicOr, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string("||")
                                .or_else(|state| state.match_string("\u{2227}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicAnd(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicAnd, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string("&&")
                                .or_else(|state| state.match_string("\u{2228}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicNot(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicNot, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state.match_string("\u{ac}")
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Ellipsis(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Ellipsis, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .match_string("...")
                                .or_else(|state| state.match_string("\u{2026}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicXor(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicXor, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state.match_string("\u{2295}")
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn MapAll(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::MapAll, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("//@"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Output(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Output, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("%%"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Concat(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Concat, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("~~"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Destruct(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Destruct, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("~="))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn DoubleBang(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::DoubleBang, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!!"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn BitNot(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::BitNot, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Curry(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Curry, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("@@@"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Apply(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Apply, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("@@"))
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LetAssign(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LetAssign, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("@="))
                    })
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn SOI(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.start_of_input()
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn EOI(
                    state: Box<::pest::ParserState<Rule>>,
                ) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::EOI, |state| state.end_of_input())
                }
            }
            pub use self::visible::*;
        }
        ::pest::state(input, |state| match rule {
            Rule::program => rules::program(state),
            Rule::statement => rules::statement(state),
            Rule::emptyStatement => rules::emptyStatement(state),
            Rule::eos => rules::eos(state),
            Rule::comma_or_semi => rules::comma_or_semi(state),
            Rule::Prefix => rules::Prefix(state),
            Rule::Postfix => rules::Postfix(state),
            Rule::Infix => rules::Infix(state),
            Rule::Set => rules::Set(state),
            Rule::Or => rules::Or(state),
            Rule::LazyOr => rules::LazyOr(state),
            Rule::Star => rules::Star(state),
            Rule::Slash => rules::Slash(state),
            Rule::Proportion => rules::Proportion(state),
            Rule::Comma => rules::Comma(state),
            Rule::Dot => rules::Dot(state),
            Rule::Separate => rules::Separate(state),
            Rule::Semicolon => rules::Semicolon(state),
            Rule::Colon => rules::Colon(state),
            Rule::Question => rules::Question(state),
            Rule::Underline => rules::Underline(state),
            Rule::Import => rules::Import(state),
            Rule::Export => rules::Export(state),
            Rule::LeftShift => rules::LeftShift(state),
            Rule::RightShift => rules::RightShift(state),
            Rule::LessEqual => rules::LessEqual(state),
            Rule::GraterEqual => rules::GraterEqual(state),
            Rule::Less => rules::Less(state),
            Rule::Grater => rules::Grater(state),
            Rule::Equivalent => rules::Equivalent(state),
            Rule::NotEquivalent => rules::NotEquivalent(state),
            Rule::Equal => rules::Equal(state),
            Rule::NotEqual => rules::NotEqual(state),
            Rule::o_add => rules::o_add(state),
            Rule::Plus => rules::Plus(state),
            Rule::Minus => rules::Minus(state),
            Rule::o_times => rules::o_times(state),
            Rule::Multiply => rules::Multiply(state),
            Rule::CenterDot => rules::CenterDot(state),
            Rule::Kronecker => rules::Kronecker(state),
            Rule::TensorProduct => rules::TensorProduct(state),
            Rule::Divide => rules::Divide(state),
            Rule::Quotient => rules::Quotient(state),
            Rule::Modulo => rules::Modulo(state),
            Rule::Remainder => rules::Remainder(state),
            Rule::o_power => rules::o_power(state),
            Rule::Power => rules::Power(state),
            Rule::Surd => rules::Surd(state),
            Rule::Increase => rules::Increase(state),
            Rule::Decrease => rules::Decrease(state),
            Rule::To => rules::To(state),
            Rule::Elvis => rules::Elvis(state),
            Rule::Map => rules::Map(state),
            Rule::Quote => rules::Quote(state),
            Rule::Acute => rules::Acute(state),
            Rule::S1 => rules::S1(state),
            Rule::S2 => rules::S2(state),
            Rule::S3 => rules::S3(state),
            Rule::S6 => rules::S6(state),
            Rule::LogicOr => rules::LogicOr(state),
            Rule::LogicAnd => rules::LogicAnd(state),
            Rule::LogicNot => rules::LogicNot(state),
            Rule::Ellipsis => rules::Ellipsis(state),
            Rule::LogicXor => rules::LogicXor(state),
            Rule::MapAll => rules::MapAll(state),
            Rule::Output => rules::Output(state),
            Rule::Concat => rules::Concat(state),
            Rule::Destruct => rules::Destruct(state),
            Rule::DoubleBang => rules::DoubleBang(state),
            Rule::BitNot => rules::BitNot(state),
            Rule::Curry => rules::Curry(state),
            Rule::Apply => rules::Apply(state),
            Rule::LetAssign => rules::LetAssign(state),
            Rule::EOI => rules::EOI(state),
        })
    }
}
