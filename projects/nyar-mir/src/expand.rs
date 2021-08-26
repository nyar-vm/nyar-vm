#![feature(prelude_import)]
#[macro_use]
extern crate std;

#[prelude_import]
use std::prelude::rust_2018::*;

mod list_parser {
    #[allow(unused_imports)]
    use super::*;
    type Input = str;
    type PositionRepr = <Input as ::peg::Parse>::PositionRepr;

    #[allow(unused_parens)]
    struct ParseState<'input> {
        _phantom: ::std::marker::PhantomData<(&'input ())>,
    }

    impl<'input> ParseState<'input> {
        fn new() -> ParseState<'input> {
            ParseState { _phantom: ::std::marker::PhantomData }
        }
    }

    fn __parse_number<'input>(
        __input: &'input Input,
        __state: &mut ParseState<'input>,
        __err_state: &mut ::peg::error::ErrorState,
        __pos: usize,
    ) -> ::peg::RuleResult<u32> {
        #![allow(non_snake_case, unused, clippy::redundant_closure_call)]
        {
            let __seq_res = {
                let str_start = __pos;
                match {
                    let mut __repeat_pos = __pos;
                    let mut __repeat_value = ::alloc::vec::Vec::new();
                    loop {
                        let __pos = __repeat_pos;
                        let __step_res = match ::peg::ParseElem::parse_elem(__input, __pos) {
                            ::peg::RuleResult::Matched(__next, __ch) => match __ch {
                                '0'..='9' => ::peg::RuleResult::Matched(__next, ()),
                                _ => {
                                    __err_state.mark_failure(__pos, "['0' ..= '9']");
                                    ::peg::RuleResult::Failed
                                }
                            },
                            ::peg::RuleResult::Failed => {
                                __err_state.mark_failure(__pos, "['0' ..= '9']");
                                ::peg::RuleResult::Failed
                            }
                        };
                        match __step_res {
                            ::peg::RuleResult::Matched(__newpos, __value) => {
                                __repeat_pos = __newpos;
                                __repeat_value.push(__value);
                            }
                            ::peg::RuleResult::Failed => {
                                break;
                            }
                        }
                    }
                    if __repeat_value.len() >= 1 {
                        ::peg::RuleResult::Matched(__repeat_pos, ())
                    }
                    else {
                        ::peg::RuleResult::Failed
                    }
                } {
                    ::peg::RuleResult::Matched(__newpos, _) => {
                        ::peg::RuleResult::Matched(__newpos, ::peg::ParseSlice::parse_slice(__input, str_start, __newpos))
                    }
                    ::peg::RuleResult::Failed => ::peg::RuleResult::Failed,
                }
            };
            match __seq_res {
                ::peg::RuleResult::Matched(__pos, n) => match (|| n.parse().or(Err("u32")))() {
                    Ok(res) => ::peg::RuleResult::Matched(__pos, res),
                    Err(expected) => {
                        __err_state.mark_failure(__pos, expected);
                        ::peg::RuleResult::Failed
                    }
                },
                ::peg::RuleResult::Failed => ::peg::RuleResult::Failed,
            }
        }
    }

    pub fn list(__input: &Input) -> ::std::result::Result<Vec<u32>, ::peg::error::ParseError<PositionRepr>> {
        #![allow(non_snake_case, unused)]
        let mut __err_state = ::peg::error::ErrorState::new(::peg::Parse::start(__input));
        let mut __state = ParseState::new();
        match __parse_list(__input, &mut __state, &mut __err_state, ::peg::Parse::start(__input)) {
            ::peg::RuleResult::Matched(__pos, __value) => {
                if ::peg::Parse::is_eof(__input, __pos) {
                    return Ok(__value);
                }
                else {
                    __err_state.mark_failure(__pos, "EOF");
                }
            }
            _ => (),
        }
        __state = ParseState::new();
        __err_state.reparse_for_error();
        match __parse_list(__input, &mut __state, &mut __err_state, ::peg::Parse::start(__input)) {
            ::peg::RuleResult::Matched(__pos, __value) => {
                if ::peg::Parse::is_eof(__input, __pos) {
                    {
                        ::std::rt::begin_panic("Parser is nondeterministic: succeeded when reparsing for error position")
                    };
                }
                else {
                    __err_state.mark_failure(__pos, "EOF");
                }
            }
            _ => (),
        }
        Err(__err_state.into_parse_error(__input))
    }

    fn __parse_list<'input>(
        __input: &'input Input,
        __state: &mut ParseState<'input>,
        __err_state: &mut ::peg::error::ErrorState,
        __pos: usize,
    ) -> ::peg::RuleResult<Vec<u32>> {
        #![allow(non_snake_case, unused, clippy::redundant_closure_call)]
        match ::peg::ParseLiteral::parse_string_literal(__input, __pos, "[") {
            ::peg::RuleResult::Matched(__pos, __val) => {
                let __seq_res = {
                    let mut __repeat_pos = __pos;
                    let mut __repeat_value = ::alloc::vec::Vec::new();
                    loop {
                        let __pos = __repeat_pos;
                        let __pos = if __repeat_value.is_empty() {
                            __pos
                        }
                        else {
                            let __sep_res = match ::peg::ParseLiteral::parse_string_literal(__input, __pos, ",") {
                                ::peg::RuleResult::Matched(__pos, __val) => ::peg::RuleResult::Matched(__pos, __val),
                                ::peg::RuleResult::Failed => {
                                    __err_state.mark_failure(__pos, "\",\"");
                                    ::peg::RuleResult::Failed
                                }
                            };
                            match __sep_res {
                                ::peg::RuleResult::Matched(__newpos, _) => __newpos,
                                ::peg::RuleResult::Failed => break,
                            }
                        };
                        let __step_res = __parse_number(__input, __state, __err_state, __pos);
                        match __step_res {
                            ::peg::RuleResult::Matched(__newpos, __value) => {
                                __repeat_pos = __newpos;
                                __repeat_value.push(__value);
                            }
                            ::peg::RuleResult::Failed => {
                                break;
                            }
                        }
                    }
                    ::peg::RuleResult::Matched(__repeat_pos, __repeat_value)
                };
                match __seq_res {
                    ::peg::RuleResult::Matched(__pos, l) => {
                        match ::peg::ParseLiteral::parse_string_literal(__input, __pos, "]") {
                            ::peg::RuleResult::Matched(__pos, __val) => ::peg::RuleResult::Matched(__pos, (|| l)()),
                            ::peg::RuleResult::Failed => {
                                __err_state.mark_failure(__pos, "\"]\"");
                                ::peg::RuleResult::Failed
                            }
                        }
                    }
                    ::peg::RuleResult::Failed => ::peg::RuleResult::Failed,
                }
            }
            ::peg::RuleResult::Failed => {
                __err_state.mark_failure(__pos, "\"[\"");
                ::peg::RuleResult::Failed
            }
        }
    }
}
