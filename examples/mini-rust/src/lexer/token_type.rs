use gaia_types::lexer::TokenType;

/// Token 类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RustTokenType {
    // 关键字
    Fn,
    Let,
    Mut,
    Return,
    If,
    Else,
    While,
    For,
    In,
    Struct,
    True,
    False,

    // 标识符和字面量
    Identifier,
    Integer,
    Float,
    CharLiteral,
    StringLiteral,

    // 运算符
    Plus,         // +
    Minus,        // -
    Star,         // *
    Slash,        // /
    Equal,        // =
    EqualEqual,   // ==
    NotEqual,     // !=
    Less,         // <
    LessEqual,    // <=
    Greater,      // >
    GreaterEqual, // >=
    Bang,         // !
    AndAnd,       // &&
    OrOr,         // ||

    // 分隔符
    LeftParen,    // (
    RightParen,   // )
    LeftBrace,    // {
    RightBrace,   // }
    LeftBracket,  // [
    RightBracket, // ]
    Semicolon,    // ;
    Comma,        // ,
    Dot,          // .
    DotDot,       // ..
    Arrow,        // ->
    Colon,        // :

    // 特殊
    Whitespace,
    Comment,
    Newline,
    Eof,
}

impl TokenType for RustTokenType {
    const END_OF_STREAM: Self = Self::Eof;
}
