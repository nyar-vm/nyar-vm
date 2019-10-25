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
    expr,
    term,
    node,
    condition,
    expressions,
    trinoculars,
    data,
    dict,
    list,
    slice,
    index,
    key_value,
    key_valid,
    element,
    index_element,
    index_range,
    index_step,
    Null,
    Boolean,
    True,
    False,
    Byte,
    Byte_BIN,
    Byte_OCT,
    Byte_HEX,
    Number,
    Decimal,
    DecimalBad,
    Complex,
    Integer,
    ComplexHandler,
    String,
    StringSingle,
    StringBlock,
    LiteralString,
    LiteralBlock,
    WHITESPACE,
    COMMENT,
    LineCommentSimple,
    LineCommentTodo,
    LineCommentFixme,
    LineCommentWarning,
    MultiLineComment,
    Symbol,
    mustSymbol,
    maybeSymbol,
    namespace,
    SYMBOL,
    NameCharacter,
    NameStartCharacter,
    Zero,
    X,
    O,
    B,
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
    fn parse<'i>(rule: Rule, input: &'i str) -> ::std::result::Result<::pest::iterators::Pairs<'i, Rule>, ::pest::error::Error<Rule>> {
        mod rules {
            pub mod hidden {
                use super::super::Rule;
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn skip(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    if state.atomicity() == ::pest::Atomicity::NonAtomic {
                        state.sequence(|state| state.repeat(|state| super::visible::WHITESPACE(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::visible::COMMENT(state).and_then(|state| state.repeat(|state| super::visible::WHITESPACE(state)))))))
                    } else {
                        Ok(state)
                    }
                }
            }
            pub mod visible {
                use super::super::Rule;
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn program(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.sequence(|state| self::SOI(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| self::statement(state).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| self::statement(state)))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::EOI(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn statement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::emptyStatement(state).or_else(|state| state.sequence(|state| self::expr(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state))))).or_else(|state| self::data(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn emptyStatement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::emptyStatement, |state| self::eos(state).or_else(|state| self::Separate(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn eos(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::eos, |state| self::Semicolon(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn comma_or_semi(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Comma(state).or_else(|state| self::Semicolon(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn expr(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| {
                        state.rule(Rule::expr, |state| {
                            state
                                .sequence(|state| self::term(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.lookahead(false, |state| self::NEWLINE(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::slice(state)))
                                .or_else(|state| self::trinoculars(state))
                                .or_else(|state| state.sequence(|state| self::term(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.sequence(|state| self::Infix(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::term(state))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.sequence(|state| self::Infix(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::term(state))))))))))))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn term(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::term, |state| state.sequence(|state| state.sequence(|state| state.optional(|state| self::Prefix(state).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| self::Prefix(state))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::node(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| self::Postfix(state).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| self::Postfix(state))))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn node(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::data(state).or_else(|state| state.sequence(|state| state.match_string("(").and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn condition(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::condition, |state| self::node(state).or_else(|state| self::expr(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn expressions(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::expressions, |state| {
                        state.sequence(|state| {
                            state
                                .match_string("(")
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| self::expr(state))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.sequence(|state| state.optional(|state| state.sequence(|state| state.optional(|state| self::comma_or_semi(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.sequence(|state| state.optional(|state| self::comma_or_semi(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state))))))))))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.optional(|state| self::comma_or_semi(state)))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.match_string(")"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn trinoculars(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("trinoculars")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn data(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::data, |state| self::dict(state).or_else(|state| self::list(state)).or_else(|state| self::Null(state)).or_else(|state| self::Boolean(state)).or_else(|state| self::Byte(state)).or_else(|state| self::Number(state)).or_else(|state| self::String(state)).or_else(|state| self::Symbol(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn dict(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::dict, |state| {
                        state.sequence(|state| {
                            state
                                .match_string("{")
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.optional(|state| self::key_value(state)))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.sequence(|state| state.optional(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::key_value(state))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::key_value(state))))))))))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.optional(|state| self::Comma(state)))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.match_string("}"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn list(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::list, |state| {
                        state.sequence(|state| {
                            state
                                .match_string("[")
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.optional(|state| self::element(state)))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.sequence(|state| state.optional(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::element(state))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::element(state))))))))))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.optional(|state| self::Comma(state)))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.match_string("]"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn slice(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::slice, |state| {
                        state.sequence(|state| {
                            state
                                .match_string("[")
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| self::index(state))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.sequence(|state| state.optional(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::index(state))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::index(state))))))))))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.optional(|state| self::Comma(state)))
                                .and_then(|state| super::hidden::skip(state))
                                .and_then(|state| state.match_string("]"))
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn index(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::index, |state| self::index_step(state).or_else(|state| self::index_range(state)).or_else(|state| self::index_element(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn key_value(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::key_value, |state| state.sequence(|state| self::key_valid(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::element(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn key_valid(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::key_valid, |state| self::Integer(state).or_else(|state| self::SYMBOL(state)).or_else(|state| self::String(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn element(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::element, |state| self::data(state).or_else(|state| self::expr(state)).or_else(|state| self::statement(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn index_element(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::index_element, |state| self::element(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn index_range(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::index_range, |state| state.sequence(|state| state.optional(|state| self::element(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::element(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn index_step(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::index_step, |state| state.sequence(|state| state.optional(|state| self::element(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::element(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::element(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Null(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Null, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("null")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Boolean(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::Boolean, |state| self::True(state).or_else(|state| self::False(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn True(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::True, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("true")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn False(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::False, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("false")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Byte(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Byte, |state| self::Byte_BIN(state).or_else(|state| self::Byte_OCT(state)).or_else(|state| self::Byte_HEX(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Byte_BIN(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Byte_BIN, |state| state.sequence(|state| self::Zero(state).and_then(|state| self::B(state)).and_then(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_BIN_DIGIT(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_BIN_DIGIT(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Byte_OCT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Byte_OCT, |state| state.sequence(|state| self::Zero(state).and_then(|state| self::O(state)).and_then(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_OCT_DIGIT(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_OCT_DIGIT(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Byte_HEX(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Byte_HEX, |state| state.sequence(|state| self::Zero(state).and_then(|state| self::X(state)).and_then(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_HEX_DIGIT(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_HEX_DIGIT(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Number(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Number, |state| self::Complex(state).or_else(|state| self::Decimal(state)).or_else(|state| self::DecimalBad(state)).or_else(|state| self::Integer(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Decimal(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Decimal, |state| state.sequence(|state| self::Integer(state).and_then(|state| self::Dot(state)).and_then(|state| self::ASCII_DIGIT(state)).and_then(|state| state.repeat(|state| self::ASCII_DIGIT(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn DecimalBad(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::DecimalBad, |state| state.sequence(|state| self::Integer(state).and_then(|state| self::Dot(state))).or_else(|state| state.sequence(|state| self::Dot(state).and_then(|state| self::ASCII_DIGIT(state)).and_then(|state| state.repeat(|state| self::ASCII_DIGIT(state)))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Complex(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Complex, |state| state.sequence(|state| self::Decimal(state).or_else(|state| self::Integer(state)).and_then(|state| self::ComplexHandler(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Integer(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Integer, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::Zero(state).or_else(|state| state.sequence(|state| self::ASCII_DIGIT(state).and_then(|state| state.repeat(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_NONZERO_DIGIT(state)))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn ComplexHandler(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::ComplexHandler, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::SYMBOL(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn String(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::String, |state| self::StringSingle(state).or_else(|state| self::StringBlock(state)).or_else(|state| self::LiteralString(state)).or_else(|state| self::LiteralBlock(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn StringSingle(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::StringSingle, |state| state.sequence(|state| self::S2(state).and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| self::S2(state)).and_then(|state| self::ANY(state))))).and_then(|state| self::S2(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn StringBlock(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::StringBlock, |state| state.sequence(|state| self::S6(state).and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| self::S6(state)).and_then(|state| self::ANY(state))))).and_then(|state| self::S6(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LiteralString(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::LiteralString, |state| state.sequence(|state| self::S1(state).and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| self::S1(state)).and_then(|state| self::ANY(state))))).and_then(|state| self::S1(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LiteralBlock(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::LiteralBlock, |state| state.sequence(|state| self::S3(state).and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| self::S3(state)).and_then(|state| self::ANY(state))))).and_then(|state| self::S3(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn WHITESPACE(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::Atomic, |state| self::NEWLINE(state).or_else(|state| self::SPACE_SEPARATOR(state)).or_else(|state| state.match_string("\t")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn COMMENT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::COMMENT, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::MultiLineComment(state).or_else(|state| self::LineCommentSimple(state)).or_else(|state| self::LineCommentTodo(state)).or_else(|state| self::LineCommentFixme(state)).or_else(|state| self::LineCommentWarning(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LineCommentSimple(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::LineCommentSimple, |state| state.sequence(|state| state.match_string("///").and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| self::NEWLINE(state)).and_then(|state| self::ANY(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LineCommentTodo(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::LineCommentTodo, |state| state.sequence(|state| state.match_string("//?").and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| self::NEWLINE(state)).and_then(|state| self::ANY(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LineCommentFixme(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::LineCommentFixme, |state| state.sequence(|state| state.match_string("//!").and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| self::NEWLINE(state)).and_then(|state| self::ANY(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LineCommentWarning(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::LineCommentWarning, |state| state.sequence(|state| state.match_string("//*").and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| self::NEWLINE(state)).and_then(|state| self::ANY(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn MultiLineComment(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::MultiLineComment, |state| state.sequence(|state| state.match_string("%%%").and_then(|state| state.repeat(|state| self::MultiLineComment(state).or_else(|state| state.sequence(|state| state.lookahead(false, |state| state.match_string("%%%")).and_then(|state| self::ANY(state)))))).and_then(|state| state.match_string("%%%")))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Symbol(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Symbol, |state| self::maybeSymbol(state).or_else(|state| self::mustSymbol(state)).or_else(|state| self::SYMBOL(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn mustSymbol(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::mustSymbol, |state| state.sequence(|state| self::namespace(state).and_then(|state| state.sequence(|state| self::Dot(state).and_then(|state| self::SYMBOL(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| self::Dot(state).and_then(|state| self::SYMBOL(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn maybeSymbol(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::maybeSymbol, |state| state.sequence(|state| self::SYMBOL(state).and_then(|state| state.sequence(|state| self::Dot(state).and_then(|state| self::SYMBOL(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| self::Dot(state).and_then(|state| self::SYMBOL(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn namespace(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::namespace, |state| state.sequence(|state| self::SYMBOL(state).and_then(|state| state.sequence(|state| self::Proportion(state).and_then(|state| self::SYMBOL(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| self::Proportion(state).and_then(|state| self::SYMBOL(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn SYMBOL(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::SYMBOL, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.sequence(|state| self::NameStartCharacter(state).and_then(|state| state.repeat(|state| self::NameCharacter(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn NameCharacter(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::ASCII_DIGIT(state).or_else(|state| self::NameStartCharacter(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn NameStartCharacter(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Underline(state).or_else(|state| self::ASCII_ALPHA(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Zero(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("0")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn X(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("x").or_else(|state| state.match_string("X"))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn O(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("o").or_else(|state| state.match_string("O"))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn B(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("b").or_else(|state| state.match_string("B"))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Prefix(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Prefix, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::Plus(state).or_else(|state| self::Increase(state)).or_else(|state| self::Decrease(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Postfix(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Postfix, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Infix(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Infix, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::o_add(state).or_else(|state| self::o_times(state)).or_else(|state| self::o_power(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Set(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Set, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("=")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Or(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Or, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("|")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LazyOr(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LazyOr, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("||")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Star(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Star, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("*")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Slash(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Slash, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("/")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Proportion(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Proportion, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("::").or_else(|state| state.match_string("\u{2237}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Comma(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Comma, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(",").or_else(|state| state.match_string("\u{ff0c}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Dot(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Dot, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(".")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Separate(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Separate, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(";;")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Semicolon(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Semicolon, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(";").or_else(|state| state.match_string("\u{ff1b}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Colon(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Colon, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(":").or_else(|state| state.match_string("\u{ff1a}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Question(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Question, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("?")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Underline(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Underline, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("_")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Import(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Import, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("<<<").or_else(|state| state.match_string("\u{22d8}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Export(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Export, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(">>>").or_else(|state| state.match_string("\u{22d9}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LeftShift(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LeftShift, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("<<").or_else(|state| state.match_string("\u{226a}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn RightShift(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::RightShift, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(">>").or_else(|state| state.match_string("\u{226b}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LessEqual(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LessEqual, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("<=")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn GraterEqual(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::GraterEqual, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(">=")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Less(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Less, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("<")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Grater(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Grater, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(">")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Equivalent(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Equivalent, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("===")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn NotEquivalent(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::NotEquivalent, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("=!=")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Equal(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Equal, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("==")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn NotEqual(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::NotEqual, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!=").or_else(|state| state.match_string("\u{2260}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn o_add(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Plus(state).or_else(|state| self::Minus(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Plus(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Plus, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("+")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Minus(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Minus, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("-")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn o_times(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Multiply(state).or_else(|state| self::CenterDot(state)).or_else(|state| self::Kronecker(state)).or_else(|state| self::TensorProduct(state)).or_else(|state| self::Divide(state)).or_else(|state| self::Quotient(state)).or_else(|state| self::Modulo(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Multiply(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Multiply, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::Star(state).or_else(|state| state.match_string("\u{d7}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn CenterDot(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::CenterDot, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\u{22c5}")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Kronecker(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Kronecker, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\u{2297}")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn TensorProduct(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::TensorProduct, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\u{2299}")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Divide(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Divide, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::Slash(state).or_else(|state| state.match_string("\u{f7}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Quotient(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Quotient, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("//")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Modulo(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Modulo, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("%")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Remainder(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Remainder, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\u{2052}")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn o_power(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Power(state)
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Power(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Power, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("^")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Surd(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Surd, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\u{221a}")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Increase(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Increase, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("++")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Decrease(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Decrease, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("--")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn To(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::To, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("->")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Elvis(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Elvis, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(":?")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Map(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Map, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("/@")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Quote(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Quote, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("`")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Acute(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Acute, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\u{b4}")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn S1(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("'")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn S2(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("\"")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn S3(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("'''")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn S6(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("\"\"\"")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicOr(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicOr, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("||").or_else(|state| state.match_string("\u{2227}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicAnd(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicAnd, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("&&").or_else(|state| state.match_string("\u{2228}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicNot(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicNot, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\u{ac}")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Ellipsis(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Ellipsis, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("...").or_else(|state| state.match_string("\u{2026}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicXor(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicXor, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\u{2295}")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn MapAll(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::MapAll, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("//@")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Output(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Output, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("%%")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Concat(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Concat, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("~~")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Destruct(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Destruct, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("~=")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn DoubleBang(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::DoubleBang, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!!")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn BitNot(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::BitNot, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Curry(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Curry, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("@@@")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Apply(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Apply, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("@@")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LetAssign(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LetAssign, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("@=")))
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn NEWLINE(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("\n").or_else(|state| state.match_string("\r\n")).or_else(|state| state.match_string("\r"))
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_ALPHA(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('a'..'z').or_else(|state| state.match_range('A'..'Z'))
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                fn SPACE_SEPARATOR(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_char_by(::pest::unicode::SPACE_SEPARATOR)
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_BIN_DIGIT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('0'..'1')
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_NONZERO_DIGIT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('1'..'9')
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ANY(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.skip(1)
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn SOI(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.start_of_input()
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn EOI(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::EOI, |state| state.end_of_input())
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_HEX_DIGIT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('0'..'9').or_else(|state| state.match_range('a'..'f')).or_else(|state| state.match_range('A'..'F'))
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_DIGIT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('0'..'9')
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_OCT_DIGIT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('0'..'7')
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
            Rule::expr => rules::expr(state),
            Rule::term => rules::term(state),
            Rule::node => rules::node(state),
            Rule::condition => rules::condition(state),
            Rule::expressions => rules::expressions(state),
            Rule::trinoculars => rules::trinoculars(state),
            Rule::data => rules::data(state),
            Rule::dict => rules::dict(state),
            Rule::list => rules::list(state),
            Rule::slice => rules::slice(state),
            Rule::index => rules::index(state),
            Rule::key_value => rules::key_value(state),
            Rule::key_valid => rules::key_valid(state),
            Rule::element => rules::element(state),
            Rule::index_element => rules::index_element(state),
            Rule::index_range => rules::index_range(state),
            Rule::index_step => rules::index_step(state),
            Rule::Null => rules::Null(state),
            Rule::Boolean => rules::Boolean(state),
            Rule::True => rules::True(state),
            Rule::False => rules::False(state),
            Rule::Byte => rules::Byte(state),
            Rule::Byte_BIN => rules::Byte_BIN(state),
            Rule::Byte_OCT => rules::Byte_OCT(state),
            Rule::Byte_HEX => rules::Byte_HEX(state),
            Rule::Number => rules::Number(state),
            Rule::Decimal => rules::Decimal(state),
            Rule::DecimalBad => rules::DecimalBad(state),
            Rule::Complex => rules::Complex(state),
            Rule::Integer => rules::Integer(state),
            Rule::ComplexHandler => rules::ComplexHandler(state),
            Rule::String => rules::String(state),
            Rule::StringSingle => rules::StringSingle(state),
            Rule::StringBlock => rules::StringBlock(state),
            Rule::LiteralString => rules::LiteralString(state),
            Rule::LiteralBlock => rules::LiteralBlock(state),
            Rule::WHITESPACE => rules::WHITESPACE(state),
            Rule::COMMENT => rules::COMMENT(state),
            Rule::LineCommentSimple => rules::LineCommentSimple(state),
            Rule::LineCommentTodo => rules::LineCommentTodo(state),
            Rule::LineCommentFixme => rules::LineCommentFixme(state),
            Rule::LineCommentWarning => rules::LineCommentWarning(state),
            Rule::MultiLineComment => rules::MultiLineComment(state),
            Rule::Symbol => rules::Symbol(state),
            Rule::mustSymbol => rules::mustSymbol(state),
            Rule::maybeSymbol => rules::maybeSymbol(state),
            Rule::namespace => rules::namespace(state),
            Rule::SYMBOL => rules::SYMBOL(state),
            Rule::NameCharacter => rules::NameCharacter(state),
            Rule::NameStartCharacter => rules::NameStartCharacter(state),
            Rule::Zero => rules::Zero(state),
            Rule::X => rules::X(state),
            Rule::O => rules::O(state),
            Rule::B => rules::B(state),
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
