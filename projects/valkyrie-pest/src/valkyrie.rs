pub struct ValkyrieParser;
#[allow(dead_code, non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Rule {
    EOI,
    program,
    statement,
    emptyStatement,
    eos,
    separator,
    comma_or_semi,
    block_or_stmt,
    Semicolon,
    Separate,
    importStatement,
    use_alias,
    use_module_select,
    use_module_string,
    module_block,
    module_tuple,
    ModuleSplit,
    Import,
    As,
    controlFlow,
    block,
    if_statement,
    if_block,
    else_if_block,
    else_block,
    condition,
    If,
    Else,
    match_statement,
    case_statement,
    Match,
    Case,
    for_statement,
    for_in_loop,
    For,
    In,
    non_local_exit,
    Type,
    classStatement,
    traitStatement,
    short_block,
    short_statement,
    short_annotation,
    Trait,
    Class,
    extendStatement,
    with_trait,
    Extend,
    With,
    assignStatement,
    assign_name,
    assign_pair,
    Let,
    defineStatement,
    define_terms,
    define_parameter,
    define_pair,
    Where,
    annotation,
    annotation_call,
    expression,
    expr,
    term,
    node,
    bracket_call,
    bracket_once,
    dot_call,
    apply,
    apply_kv,
    apply_generic,
    function_name,
    function_module,
    type_expr,
    type_hint,
    generic_type,
    parametric_types,
    parametric_types_pair,
    parametric_types_term,
    TypeInfix,
    data,
    tuple,
    list,
    slice,
    index,
    index_range,
    index_step,
    dict,
    key_value,
    key_valid,
    Byte,
    Decimal,
    Integer,
    Complex,
    Zero,
    Special,
    String,
    StringTemplate,
    StringItem,
    StringSimple,
    XmlEscaped,
    Apostrophe,
    Quotation,
    Symbol,
    namepath,
    freepath,
    SYMBOL_XID,
    SYMBOL_ESCAPE,
    Quote,
    WHITESPACE,
    COMMENT,
    Comment1,
    CommentN,
    Tilde,
    Modifier,
    Prefix,
    Suffix,
    Infix,
    Or,
    LazyOr,
    Star,
    Slash,
    Solidus,
    Proportion,
    Comma,
    Dot,
    Colon,
    Question,
    Underline,
    Load,
    Save,
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
    Set,
    Plus,
    Minus,
    Multiply,
    CenterDot,
    Kronecker,
    TensorProduct,
    Divide,
    Quotient,
    Modulo,
    Power,
    Surd,
    Increase,
    Decrease,
    To,
    Elvis,
    LogicOr,
    LogicAnd,
    LogicNot,
    LogicXor,
    Ellipsis,
    Output,
    DoubleBang,
    Bang,
    Sharp,
    At,
}
#[allow(clippy::all)]
impl ::pest::Parser<Rule> for ValkyrieParser {
    fn parse<'i>(rule: Rule, input: &'i str) -> ::std::result::Result<::pest::iterators::Pairs<'i, Rule>, ::pest::error::Error<Rule>> {
        mod rules {
            pub mod hidden {
                use super::super::Rule;
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn skip(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    if state.atomicity() == ::pest::Atomicity::NonAtomic { state.sequence(|state| state.repeat(|state| super::visible::WHITESPACE(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::visible::COMMENT(state).and_then(|state| state.repeat(|state| super::visible::WHITESPACE(state))))))) } else { Ok(state) }
                }
            }
            pub mod visible {
                use super::super::Rule;
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn program(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.sequence(|state| self::SOI(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::statement(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::statement(state))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::EOI(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn statement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::statement, |state| self::emptyStatement(state).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::importStatement(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::classStatement(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::traitStatement(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::extendStatement(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::controlFlow(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::assignStatement(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::defineStatement(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::annotation(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| self::expression(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn emptyStatement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::emptyStatement, |state| self::eos(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn eos(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::eos, |state| self::Separate(state).or_else(|state| self::Semicolon(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn separator(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::separator, |state| self::eos(state).or_else(|state| self::Comma(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn comma_or_semi(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Comma(state).or_else(|state| self::Semicolon(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn block_or_stmt(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.restore_on_err(|state| self::block(state)).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::Set(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::statement(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Semicolon(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Semicolon, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(";")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Separate(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Separate, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(";;")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn importStatement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::importStatement, |state| state.restore_on_err(|state| state.sequence(|state| self::Import(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| self::Dot(state).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| self::Dot(state)))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::use_alias(state)))).or_else(|state| state.sequence(|state| self::Import(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.restore_on_err(|state| state.sequence(|state| state.sequence(|state| state.optional(|state| self::Dot(state).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| self::Dot(state))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::use_module_select(state)))).or_else(|state| state.restore_on_err(|state| self::use_module_string(state)))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn use_alias(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::use_alias, |state| state.restore_on_err(|state| state.sequence(|state| self::String(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::As(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::SYMBOL(state)))).or_else(|state| state.sequence(|state| self::SYMBOL(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.sequence(|state| self::ModuleSplit(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::SYMBOL(state))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.sequence(|state| self::ModuleSplit(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::SYMBOL(state)))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::As(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::SYMBOL(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn use_module_select(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::use_module_select, |state| state.sequence(|state| self::SYMBOL(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.sequence(|state| self::ModuleSplit(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::SYMBOL(state))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.sequence(|state| self::ModuleSplit(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::SYMBOL(state)))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.sequence(|state| self::ModuleSplit(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.restore_on_err(|state| self::module_block(state)).or_else(|state| self::Star(state)))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn use_module_string(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::use_module_string, |state| state.sequence(|state| self::String(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.sequence(|state| self::ModuleSplit(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.restore_on_err(|state| self::module_block(state)).or_else(|state| self::Star(state)))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn module_block(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::module_block, |state| state.sequence(|state| state.match_string("{").and_then(|state| super::hidden::skip(state)).and_then(|state| self::module_tuple(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| state.optional(|state| self::comma_or_semi(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::module_tuple(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| state.optional(|state| self::comma_or_semi(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::module_tuple(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::comma_or_semi(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn module_tuple(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::module_tuple, |state| state.restore_on_err(|state| self::use_alias(state)).or_else(|state| state.restore_on_err(|state| self::use_module_select(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn ModuleSplit(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Dot(state).or_else(|state| self::Proportion(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Import(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Import, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("using")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn As(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::As, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("as")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn controlFlow(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.restore_on_err(|state| self::non_local_exit(state)).or_else(|state| state.restore_on_err(|state| self::if_statement(state))).or_else(|state| state.restore_on_err(|state| self::for_statement(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn block(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::block, |state| state.sequence(|state| state.match_string("{").and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| self::statement(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::statement(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::statement(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("}")))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn if_statement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::if_statement, |state| state.sequence(|state| self::if_block(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::else_if_block(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::else_if_block(state))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::else_block(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn if_block(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.sequence(|state| self::If(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::condition(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::block(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn else_if_block(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.sequence(|state| self::Else(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::If(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::condition(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::block(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn else_block(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.sequence(|state| self::Else(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::block(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn condition(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.restore_on_err(|state| state.sequence(|state| state.match_string("(").and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")")))).or_else(|state| state.restore_on_err(|state| self::expr(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn If(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::If, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("if")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Else(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Else, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("else")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn match_statement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::match_statement, |state| state.sequence(|state| self::Match(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expression(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("{")).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| self::case_statement(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::case_statement(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::case_statement(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn case_statement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::case_statement, |state| state.sequence(|state| self::Colon(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::statement(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Match(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Match, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("match")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Case(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Case, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("case")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn for_statement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::for_statement, |state| state.sequence(|state| self::For(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::for_in_loop(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn for_in_loop(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::for_in_loop, |state| state.sequence(|state| self::SYMBOL(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::In(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::block(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn For(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::For, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("for")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn In(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::In, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("in")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn non_local_exit(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::non_local_exit, |state| state.sequence(|state| state.match_string("return").or_else(|state| state.match_string("resume")).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Type(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Type, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("type")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn classStatement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::classStatement, |state| state.sequence(|state| self::Class(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::assign_pair(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::short_block(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn traitStatement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::traitStatement, |state| state.sequence(|state| self::Trait(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::assign_pair(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::short_block(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn short_block(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::short_block, |state| state.sequence(|state| state.match_string("{").and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::short_statement(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::short_statement(state))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("}"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn short_statement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::short_statement, |state| self::emptyStatement(state).or_else(|state| state.restore_on_err(|state| state.sequence(|state| state.optional(|state| state.match_string("def")).and_then(|state| super::hidden::skip(state)).and_then(|state| self::define_terms(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::assign_pair(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::separator(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::short_annotation(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn short_annotation(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::short_annotation, |state| state.sequence(|state| state.sequence(|state| self::annotation_call(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::annotation_call(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::annotation_call(state)))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::short_statement(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Trait(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Trait, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("trait")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Class(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Class, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("class")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn extendStatement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::extendStatement, |state| state.sequence(|state| self::Extend(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Symbol(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::parametric_types(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::with_trait(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::short_block(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn with_trait(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::with_trait, |state| state.sequence(|state| self::With(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Symbol(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::parametric_types(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Extend(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Extend, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("extend")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn With(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::With, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("with")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn assignStatement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::assignStatement, |state| state.restore_on_err(|state| state.sequence(|state| self::Let(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("(")).and_then(|state| super::hidden::skip(state)).and_then(|state| self::assign_name(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")")).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::type_hint(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Set(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::Let(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("(")).and_then(|state| super::hidden::skip(state)).and_then(|state| self::assign_name(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")")).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::type_hint(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::Let(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::assign_name(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Set(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))).or_else(|state| state.sequence(|state| self::Let(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::assign_name(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::eos(state)))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::short_statement(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state))))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn assign_name(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.sequence(|state| self::assign_pair(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::assign_pair(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::assign_pair(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::Comma(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn assign_pair(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::assign_pair, |state| state.sequence(|state| state.sequence(|state| state.optional(|state| self::Modifier(state).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| self::Modifier(state))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Symbol(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::type_hint(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Let(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Let, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("let")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn defineStatement(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::defineStatement, |state| state.sequence(|state| state.match_string("def").and_then(|state| super::hidden::skip(state)).and_then(|state| self::define_terms(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn define_terms(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.sequence(|state| self::assign_pair(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::parametric_types(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::define_parameter(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::type_hint(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::parametric_types_term(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::block_or_stmt(state)))))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::define_parameter(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::type_hint(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::block_or_stmt(state)))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn define_parameter(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::define_parameter, |state| state.sequence(|state| state.match_string("(").and_then(|state| super::hidden::skip(state)).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::define_pair(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::define_pair(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::define_pair(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::Comma(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")")))).or_else(|state| state.match_string(")")))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn define_pair(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::define_pair, |state| state.sequence(|state| self::SYMBOL(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::type_hint(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Set(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Where(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Where, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("where")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn annotation(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::annotation, |state| state.sequence(|state| state.sequence(|state| self::annotation_call(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::annotation_call(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::annotation_call(state)))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::statement(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn annotation_call(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::annotation_call, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.sequence(|state| self::At(state).and_then(|state| state.restore_on_err(|state| self::list(state)).or_else(|state| state.restore_on_err(|state| self::apply(state))).or_else(|state| self::Symbol(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn expression(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::expression, |state| state.sequence(|state| self::expr(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn expr(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::expr, |state| state.sequence(|state| self::term(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Infix(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::term(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::Infix(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::term(state))))))))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn term(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::term, |state| state.sequence(|state| state.sequence(|state| state.optional(|state| self::Prefix(state).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| self::Prefix(state))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::node(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| self::Suffix(state).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| self::Suffix(state))))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn node(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::node, |state| state.restore_on_err(|state| state.sequence(|state| state.match_string("(").and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")")))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::data(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::bracket_once(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.sequence(|state| self::bracket_call(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::bracket_call(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::bracket_call(state)))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::bracket_once(state)))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| state.sequence(|state| self::bracket_call(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::bracket_call(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::bracket_call(state)))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::bracket_once(state)))))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| state.sequence(|state| self::bracket_call(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::bracket_call(state)).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| self::bracket_call(state)))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::bracket_once(state)))))))))))))))).or_else(|state| state.restore_on_err(|state| self::data(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn bracket_call(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::dot_call(state).or_else(|state| state.restore_on_err(|state| self::slice(state))).or_else(|state| state.restore_on_err(|state| self::apply(state))).or_else(|state| state.restore_on_err(|state| self::apply_generic(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn bracket_once(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.restore_on_err(|state| self::dict(state)).or_else(|state| state.restore_on_err(|state| self::block(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn dot_call(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::dot_call, |state| state.sequence(|state| self::Dot(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Integer(state).or_else(|state| self::Symbol(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn apply(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::apply, |state| state.sequence(|state| state.match_string("(").and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")").or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::apply_kv(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::apply_kv(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::apply_kv(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::Comma(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")")))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn apply_kv(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::apply_kv, |state| state.restore_on_err(|state| state.sequence(|state| self::SYMBOL(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)))).or_else(|state| state.restore_on_err(|state| self::expr(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn apply_generic(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::apply_generic, |state| state.sequence(|state| state.match_string("::").and_then(|state| super::hidden::skip(state)).and_then(|state| self::generic_type(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn function_name(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::function_name, |state| self::SYMBOL(state))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn function_module(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::function_module, |state| state.sequence(|state| state.optional(|state| state.sequence(|state| self::namepath(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Dot(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.sequence(|state| self::SYMBOL(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Dot(state))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.sequence(|state| self::SYMBOL(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Dot(state))))))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn type_expr(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.sequence(|state| self::term(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::TypeInfix(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::term(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::TypeInfix(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::term(state))))))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn type_hint(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::type_hint, |state| state.sequence(|state| self::Colon(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::type_expr(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn generic_type(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::generic_type, |state| state.sequence(|state| state.match_string("<").and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::Comma(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(">"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn parametric_types(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::parametric_types, |state| state.sequence(|state| state.match_string("<").and_then(|state| super::hidden::skip(state)).and_then(|state| self::parametric_types_pair(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::parametric_types_pair(state))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::parametric_types_pair(state)))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(">"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn parametric_types_pair(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::parametric_types_pair, |state| state.sequence(|state| state.optional(|state| self::Plus(state).or_else(|state| self::Minus(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::SYMBOL(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn parametric_types_term(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::parametric_types_term, |state| state.sequence(|state| self::Where(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| self::expr(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::expr(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state))))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::expr(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::eos(state))))))))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn TypeInfix(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::TypeInfix, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::Or(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn data(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::data, |state| state.restore_on_err(|state| self::tuple(state)).or_else(|state| state.restore_on_err(|state| self::dict(state))).or_else(|state| state.restore_on_err(|state| self::block(state))).or_else(|state| state.restore_on_err(|state| self::list(state))).or_else(|state| self::Special(state)).or_else(|state| self::Byte(state)).or_else(|state| self::Complex(state)).or_else(|state| self::Decimal(state)).or_else(|state| self::Integer(state)).or_else(|state| state.restore_on_err(|state| self::String(state))).or_else(|state| self::Symbol(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn tuple(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::tuple, |state| state.sequence(|state| state.match_string("(").and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")").or_else(|state| state.restore_on_err(|state| state.sequence(|state| self::expr(state).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::Comma(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string(")")))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn list(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::list, |state| state.sequence(|state| state.match_string("[").and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("]").or_else(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::expr(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::Comma(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("]"))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn slice(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::slice, |state| state.sequence(|state| state.match_string("[").and_then(|state| super::hidden::skip(state)).and_then(|state| self::index(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::index(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::index(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::Comma(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("]")))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn index(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::index, |state| state.restore_on_err(|state| self::index_step(state)).or_else(|state| state.restore_on_err(|state| self::index_range(state))).or_else(|state| state.restore_on_err(|state| self::expr(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn index_range(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::index_range, |state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::expr(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::expr(state)))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn index_step(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::index_step, |state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| self::expr(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::expr(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::expr(state)))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn dict(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::dict, |state| state.sequence(|state| state.match_string("[").and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("]"))).or_else(|state| state.sequence(|state| state.match_string("{").and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| state.restore_on_err(|state| self::key_value(state)))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.sequence(|state| state.optional(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::key_value(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| super::hidden::skip(state).and_then(|state| state.restore_on_err(|state| state.sequence(|state| self::Comma(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::key_value(state))))))))))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.optional(|state| self::Comma(state))).and_then(|state| super::hidden::skip(state)).and_then(|state| state.match_string("}"))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn key_value(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::key_value, |state| state.sequence(|state| self::key_valid(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Colon(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::expr(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn key_valid(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Integer(state).or_else(|state| self::Symbol(state)).or_else(|state| state.restore_on_err(|state| self::String(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Byte(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Byte, |state| state.sequence(|state| self::Zero(state).and_then(|state| self::ASCII_ALPHA(state)).and_then(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Decimal(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Decimal, |state| state.sequence(|state| self::Integer(state).and_then(|state| self::Dot(state)).and_then(|state| self::Integer(state)).and_then(|state| state.repeat(|state| self::Integer(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Integer(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Integer, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::Zero(state).or_else(|state| state.sequence(|state| self::ASCII_NONZERO_DIGIT(state).and_then(|state| state.repeat(|state| state.sequence(|state| state.optional(|state| self::Underline(state)).and_then(|state| self::ASCII_DIGIT(state)))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Complex(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Complex, |state| state.sequence(|state| self::Decimal(state).or_else(|state| self::Integer(state)).and_then(|state| super::hidden::skip(state)).and_then(|state| self::SYMBOL(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Zero(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("0")
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Special(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::Special, |state| state.match_string("true").or_else(|state| state.match_string("false"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn String(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::String, |state| state.sequence(|state| state.optional(|state| self::Symbol(state)).and_then(|state| state.restore_on_err(|state| self::StringTemplate(state)).or_else(|state| self::StringSimple(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn StringTemplate(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::StringTemplate, |state| state.sequence(|state| state.stack_push(|state| state.sequence(|state| self::Quotation(state).and_then(|state| self::Quotation(state)).and_then(|state| self::Quotation(state)).and_then(|state| state.repeat(|state| self::Quotation(state)))).or_else(|state| state.sequence(|state| self::Apostrophe(state).and_then(|state| self::Apostrophe(state)).and_then(|state| self::Apostrophe(state)).and_then(|state| state.repeat(|state| self::Apostrophe(state))))).or_else(|state| self::Quotation(state)).or_else(|state| self::Apostrophe(state))).and_then(|state| state.repeat(|state| state.restore_on_err(|state| self::StringItem(state)))).and_then(|state| self::POP(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn StringItem(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::StringItem, |state| state.sequence(|state| state.match_string("$").and_then(|state| self::ASCII_ALPHANUMERIC(state))).or_else(|state| state.restore_on_err(|state| state.sequence(|state| state.match_string("${").and_then(|state| self::expression(state)).and_then(|state| state.match_string("}"))))).or_else(|state| state.sequence(|state| state.match_string("\\x").and_then(|state| self::ASCII_ALPHANUMERIC(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)))).or_else(|state| state.sequence(|state| state.match_string("\\u").and_then(|state| self::ASCII_ALPHANUMERIC(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)))).or_else(|state| state.sequence(|state| state.match_string("\\U").and_then(|state| self::ASCII_ALPHANUMERIC(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)).and_then(|state| self::ASCII_ALPHANUMERIC(state)))).or_else(|state| state.sequence(|state| state.match_string("\\").and_then(|state| self::ANY(state)))).or_else(|state| state.sequence(|state| state.lookahead(false, |state| self::PEEK(state)).and_then(|state| self::ANY(state))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn StringSimple(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::StringSimple, |state| {
                        state.atomic(::pest::Atomicity::Atomic, |state| {
                            state
                                .sequence(|state| {
                                    state
                                        .match_string("«")
                                        .and_then(|state| {
                                            let strings = ["»"];
                                            state.skip_until(&strings)
                                        })
                                        .and_then(|state| state.match_string("»"))
                                })
                                .or_else(|state| {
                                    state.sequence(|state| {
                                        state
                                            .match_string("‹")
                                            .and_then(|state| {
                                                let strings = ["›"];
                                                state.skip_until(&strings)
                                            })
                                            .and_then(|state| state.match_string("›"))
                                    })
                                })
                        })
                    })
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn XmlEscaped(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::XmlEscaped, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.sequence(|state| state.match_string("&").and_then(|state| state.sequence(|state| self::ASCII_ALPHANUMERIC(state).and_then(|state| state.repeat(|state| self::ASCII_ALPHANUMERIC(state))))).and_then(|state| self::Semicolon(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Apostrophe(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Apostrophe, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("'")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Quotation(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Quotation, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\"")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Symbol(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::Symbol, |state| self::SYMBOL_XID(state).or_else(|state| self::SYMBOL_ESCAPE(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn namepath(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::namepath, |state| state.sequence(|state| self::Symbol(state).and_then(|state| state.sequence(|state| self::Proportion(state).and_then(|state| self::Symbol(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| self::Proportion(state).and_then(|state| self::Symbol(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn freepath(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::CompoundAtomic, |state| state.rule(Rule::freepath, |state| state.sequence(|state| self::Symbol(state).and_then(|state| state.sequence(|state| self::Proportion(state).or_else(|state| self::Dot(state)).and_then(|state| self::Symbol(state)))).and_then(|state| state.repeat(|state| state.sequence(|state| self::Proportion(state).or_else(|state| self::Dot(state)).and_then(|state| self::Symbol(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn SYMBOL_XID(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::SYMBOL_XID, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.sequence(|state| self::XID_START(state).and_then(|state| state.repeat(|state| self::XID_CONTINUE(state)))).or_else(|state| state.sequence(|state| self::Underline(state).and_then(|state| self::XID_CONTINUE(state)).and_then(|state| state.repeat(|state| self::XID_CONTINUE(state)))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn SYMBOL_ESCAPE(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::SYMBOL_ESCAPE, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.sequence(|state| self::Quote(state).and_then(|state| state.lookahead(false, |state| state.sequence(|state| self::Quote(state).and_then(|state| self::ANY(state))))).and_then(|state| self::Quote(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Quote(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Quote, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("`")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn WHITESPACE(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::Atomic, |state| self::NEWLINE(state).or_else(|state| self::WHITE_SPACE(state)))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn COMMENT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::COMMENT, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.restore_on_err(|state| self::CommentN(state)).or_else(|state| self::Comment1(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Comment1(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Comment1, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.sequence(|state| self::Tilde(state).and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| self::NEWLINE(state)).and_then(|state| self::ANY(state))))))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn CommentN(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::CommentN, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.sequence(|state| state.stack_push(|state| state.sequence(|state| self::Tilde(state).and_then(|state| self::Tilde(state)).and_then(|state| self::Tilde(state)).and_then(|state| state.repeat(|state| self::Tilde(state))))).and_then(|state| state.repeat(|state| state.sequence(|state| state.lookahead(false, |state| state.sequence(|state| self::Tilde(state).and_then(|state| self::PEEK(state)))).and_then(|state| self::ANY(state))))).and_then(|state| self::POP(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Tilde(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Tilde, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("~")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Modifier(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.atomic(::pest::Atomicity::NonAtomic, |state| state.rule(Rule::Modifier, |state| state.sequence(|state| state.lookahead(false, |state| state.sequence(|state| self::SYMBOL(state).and_then(|state| super::hidden::skip(state)).and_then(|state| self::Dot(state).or_else(|state| self::Set(state)).or_else(|state| self::Comma(state)).or_else(|state| self::Colon(state)).or_else(|state| self::Semicolon(state)).or_else(|state| state.match_string("{")).or_else(|state| state.match_string("}")).or_else(|state| state.match_string("(")).or_else(|state| state.match_string(")")).or_else(|state| state.match_string("<")).or_else(|state| state.match_string(">"))))).and_then(|state| super::hidden::skip(state)).and_then(|state| self::SYMBOL(state)))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Prefix(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Prefix, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::Bang(state).or_else(|state| self::Plus(state)).or_else(|state| self::Minus(state)).or_else(|state| self::Star(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Suffix(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Suffix, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::DoubleBang(state).or_else(|state| self::Bang(state)).or_else(|state| self::Question(state))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Infix(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    self::Set(state).or_else(|state| self::Increase(state)).or_else(|state| self::Decrease(state)).or_else(|state| self::Plus(state)).or_else(|state| self::Minus(state)).or_else(|state| self::Multiply(state)).or_else(|state| self::CenterDot(state)).or_else(|state| self::Kronecker(state)).or_else(|state| self::TensorProduct(state)).or_else(|state| self::Divide(state)).or_else(|state| self::Quotient(state)).or_else(|state| self::Modulo(state)).or_else(|state| self::Power(state)).or_else(|state| self::Grater(state)).or_else(|state| self::GraterEqual(state)).or_else(|state| self::Equal(state))
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
                pub fn Solidus(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Solidus, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("\\")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Proportion(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Proportion, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("::").or_else(|state| state.match_string("∷"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Comma(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Comma, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(",")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Dot(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Dot, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(".")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Colon(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Colon, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(":")))
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
                pub fn Load(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Load, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("<<<").or_else(|state| state.match_string("⋘"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Save(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Save, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(">>>").or_else(|state| state.match_string("⋙"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LeftShift(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LeftShift, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("<<").or_else(|state| state.match_string("≪"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn RightShift(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::RightShift, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string(">>").or_else(|state| state.match_string("≫"))))
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
                    state.rule(Rule::NotEqual, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!=").or_else(|state| state.match_string("≠"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Set(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Set, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("=")))
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
                pub fn Multiply(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Multiply, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::Star(state).or_else(|state| state.match_string("×"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn CenterDot(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::CenterDot, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("⋅")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Kronecker(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Kronecker, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("⊗")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn TensorProduct(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::TensorProduct, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("⊙")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Divide(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Divide, |state| state.atomic(::pest::Atomicity::Atomic, |state| self::Slash(state).or_else(|state| state.match_string("÷"))))
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
                pub fn Power(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Power, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("^")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Surd(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Surd, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("√")))
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
                pub fn LogicOr(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicOr, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("||").or_else(|state| state.match_string("∧"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicAnd(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicAnd, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("&&").or_else(|state| state.match_string("∨"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicNot(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicNot, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("¬")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn LogicXor(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::LogicXor, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("⊕")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Ellipsis(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Ellipsis, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("...").or_else(|state| state.match_string("…"))))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Output(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Output, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("%%")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn DoubleBang(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::DoubleBang, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!!")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Bang(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Bang, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("!")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn Sharp(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::Sharp, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("#")))
                }
                #[inline]
                #[allow(non_snake_case, unused_variables)]
                pub fn At(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::At, |state| state.atomic(::pest::Atomicity::Atomic, |state| state.match_string("@")))
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ANY(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.skip(1)
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn EOI(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.rule(Rule::EOI, |state| state.end_of_input())
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn SOI(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.start_of_input()
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn PEEK(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.stack_peek()
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn POP(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.stack_pop()
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_DIGIT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('0'..'9')
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_NONZERO_DIGIT(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('1'..'9')
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_ALPHA(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('a'..'z').or_else(|state| state.match_range('A'..'Z'))
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn ASCII_ALPHANUMERIC(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_range('a'..'z').or_else(|state| state.match_range('A'..'Z')).or_else(|state| state.match_range('0'..'9'))
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                pub fn NEWLINE(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_string("\n").or_else(|state| state.match_string("\r\n")).or_else(|state| state.match_string("\r"))
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                fn WHITE_SPACE(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_char_by(::pest::unicode::WHITE_SPACE)
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                fn XID_CONTINUE(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_char_by(::pest::unicode::XID_CONTINUE)
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                fn XID_START(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_char_by(::pest::unicode::XID_START)
                }
                #[inline]
                #[allow(dead_code, non_snake_case, unused_variables)]
                fn SYMBOL(state: Box<::pest::ParserState<Rule>>) -> ::pest::ParseResult<Box<::pest::ParserState<Rule>>> {
                    state.match_char_by(::pest::unicode::SYMBOL)
                }
            }
            pub use self::visible::*;
        }
        ::pest::state(input, |state| match rule {
            Rule::program => rules::program(state),
            Rule::statement => rules::statement(state),
            Rule::emptyStatement => rules::emptyStatement(state),
            Rule::eos => rules::eos(state),
            Rule::separator => rules::separator(state),
            Rule::comma_or_semi => rules::comma_or_semi(state),
            Rule::block_or_stmt => rules::block_or_stmt(state),
            Rule::Semicolon => rules::Semicolon(state),
            Rule::Separate => rules::Separate(state),
            Rule::importStatement => rules::importStatement(state),
            Rule::use_alias => rules::use_alias(state),
            Rule::use_module_select => rules::use_module_select(state),
            Rule::use_module_string => rules::use_module_string(state),
            Rule::module_block => rules::module_block(state),
            Rule::module_tuple => rules::module_tuple(state),
            Rule::ModuleSplit => rules::ModuleSplit(state),
            Rule::Import => rules::Import(state),
            Rule::As => rules::As(state),
            Rule::controlFlow => rules::controlFlow(state),
            Rule::block => rules::block(state),
            Rule::if_statement => rules::if_statement(state),
            Rule::if_block => rules::if_block(state),
            Rule::else_if_block => rules::else_if_block(state),
            Rule::else_block => rules::else_block(state),
            Rule::condition => rules::condition(state),
            Rule::If => rules::If(state),
            Rule::Else => rules::Else(state),
            Rule::match_statement => rules::match_statement(state),
            Rule::case_statement => rules::case_statement(state),
            Rule::Match => rules::Match(state),
            Rule::Case => rules::Case(state),
            Rule::for_statement => rules::for_statement(state),
            Rule::for_in_loop => rules::for_in_loop(state),
            Rule::For => rules::For(state),
            Rule::In => rules::In(state),
            Rule::non_local_exit => rules::non_local_exit(state),
            Rule::Type => rules::Type(state),
            Rule::classStatement => rules::classStatement(state),
            Rule::traitStatement => rules::traitStatement(state),
            Rule::short_block => rules::short_block(state),
            Rule::short_statement => rules::short_statement(state),
            Rule::short_annotation => rules::short_annotation(state),
            Rule::Trait => rules::Trait(state),
            Rule::Class => rules::Class(state),
            Rule::extendStatement => rules::extendStatement(state),
            Rule::with_trait => rules::with_trait(state),
            Rule::Extend => rules::Extend(state),
            Rule::With => rules::With(state),
            Rule::assignStatement => rules::assignStatement(state),
            Rule::assign_name => rules::assign_name(state),
            Rule::assign_pair => rules::assign_pair(state),
            Rule::Let => rules::Let(state),
            Rule::defineStatement => rules::defineStatement(state),
            Rule::define_terms => rules::define_terms(state),
            Rule::define_parameter => rules::define_parameter(state),
            Rule::define_pair => rules::define_pair(state),
            Rule::Where => rules::Where(state),
            Rule::annotation => rules::annotation(state),
            Rule::annotation_call => rules::annotation_call(state),
            Rule::expression => rules::expression(state),
            Rule::expr => rules::expr(state),
            Rule::term => rules::term(state),
            Rule::node => rules::node(state),
            Rule::bracket_call => rules::bracket_call(state),
            Rule::bracket_once => rules::bracket_once(state),
            Rule::dot_call => rules::dot_call(state),
            Rule::apply => rules::apply(state),
            Rule::apply_kv => rules::apply_kv(state),
            Rule::apply_generic => rules::apply_generic(state),
            Rule::function_name => rules::function_name(state),
            Rule::function_module => rules::function_module(state),
            Rule::type_expr => rules::type_expr(state),
            Rule::type_hint => rules::type_hint(state),
            Rule::generic_type => rules::generic_type(state),
            Rule::parametric_types => rules::parametric_types(state),
            Rule::parametric_types_pair => rules::parametric_types_pair(state),
            Rule::parametric_types_term => rules::parametric_types_term(state),
            Rule::TypeInfix => rules::TypeInfix(state),
            Rule::data => rules::data(state),
            Rule::tuple => rules::tuple(state),
            Rule::list => rules::list(state),
            Rule::slice => rules::slice(state),
            Rule::index => rules::index(state),
            Rule::index_range => rules::index_range(state),
            Rule::index_step => rules::index_step(state),
            Rule::dict => rules::dict(state),
            Rule::key_value => rules::key_value(state),
            Rule::key_valid => rules::key_valid(state),
            Rule::Byte => rules::Byte(state),
            Rule::Decimal => rules::Decimal(state),
            Rule::Integer => rules::Integer(state),
            Rule::Complex => rules::Complex(state),
            Rule::Zero => rules::Zero(state),
            Rule::Special => rules::Special(state),
            Rule::String => rules::String(state),
            Rule::StringTemplate => rules::StringTemplate(state),
            Rule::StringItem => rules::StringItem(state),
            Rule::StringSimple => rules::StringSimple(state),
            Rule::XmlEscaped => rules::XmlEscaped(state),
            Rule::Apostrophe => rules::Apostrophe(state),
            Rule::Quotation => rules::Quotation(state),
            Rule::Symbol => rules::Symbol(state),
            Rule::namepath => rules::namepath(state),
            Rule::freepath => rules::freepath(state),
            Rule::SYMBOL_XID => rules::SYMBOL_XID(state),
            Rule::SYMBOL_ESCAPE => rules::SYMBOL_ESCAPE(state),
            Rule::Quote => rules::Quote(state),
            Rule::WHITESPACE => rules::WHITESPACE(state),
            Rule::COMMENT => rules::COMMENT(state),
            Rule::Comment1 => rules::Comment1(state),
            Rule::CommentN => rules::CommentN(state),
            Rule::Tilde => rules::Tilde(state),
            Rule::Modifier => rules::Modifier(state),
            Rule::Prefix => rules::Prefix(state),
            Rule::Suffix => rules::Suffix(state),
            Rule::Infix => rules::Infix(state),
            Rule::Or => rules::Or(state),
            Rule::LazyOr => rules::LazyOr(state),
            Rule::Star => rules::Star(state),
            Rule::Slash => rules::Slash(state),
            Rule::Solidus => rules::Solidus(state),
            Rule::Proportion => rules::Proportion(state),
            Rule::Comma => rules::Comma(state),
            Rule::Dot => rules::Dot(state),
            Rule::Colon => rules::Colon(state),
            Rule::Question => rules::Question(state),
            Rule::Underline => rules::Underline(state),
            Rule::Load => rules::Load(state),
            Rule::Save => rules::Save(state),
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
            Rule::Set => rules::Set(state),
            Rule::Plus => rules::Plus(state),
            Rule::Minus => rules::Minus(state),
            Rule::Multiply => rules::Multiply(state),
            Rule::CenterDot => rules::CenterDot(state),
            Rule::Kronecker => rules::Kronecker(state),
            Rule::TensorProduct => rules::TensorProduct(state),
            Rule::Divide => rules::Divide(state),
            Rule::Quotient => rules::Quotient(state),
            Rule::Modulo => rules::Modulo(state),
            Rule::Power => rules::Power(state),
            Rule::Surd => rules::Surd(state),
            Rule::Increase => rules::Increase(state),
            Rule::Decrease => rules::Decrease(state),
            Rule::To => rules::To(state),
            Rule::Elvis => rules::Elvis(state),
            Rule::LogicOr => rules::LogicOr(state),
            Rule::LogicAnd => rules::LogicAnd(state),
            Rule::LogicNot => rules::LogicNot(state),
            Rule::LogicXor => rules::LogicXor(state),
            Rule::Ellipsis => rules::Ellipsis(state),
            Rule::Output => rules::Output(state),
            Rule::DoubleBang => rules::DoubleBang(state),
            Rule::Bang => rules::Bang(state),
            Rule::Sharp => rules::Sharp(state),
            Rule::At => rules::At(state),
            Rule::EOI => rules::EOI(state),
        })
    }
}
