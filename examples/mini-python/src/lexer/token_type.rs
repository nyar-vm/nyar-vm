use gaia_types::lexer::TokenType;

/// Python Token 类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PythonTokenType {
    // 关键字
    Def,      // def
    Class,    // class
    If,       // if
    Elif,     // elif
    Else,     // else
    For,      // for
    While,    // while
    In,       // in
    Return,   // return
    Break,    // break
    Continue, // continue
    Pass,     // pass
    Import,   // import
    From,     // from
    As,       // as
    Try,      // try
    Except,   // except
    Finally,  // finally
    Raise,    // raise
    With,     // with
    Lambda,   // lambda
    And,      // and
    Or,       // or
    Not,      // not
    Is,       // is
    None,     // None
    True,     // True
    False,    // False

    // 标识符和字面量
    Identifier,
    Integer,
    Float,
    String,

    // 运算符
    Plus,             // +
    Minus,            // -
    Star,             // *
    Slash,            // /
    DoubleSlash,      // //
    DoubleSlashEqual, // //=
    Percent,          // %
    PercentEqual,     // %=
    DoubleStar,       // **
    DoubleStarEqual,  // **=
    Equal,            // =
    EqualEqual,       // ==
    BangEqual,        // !=
    Less,             // <
    LessEqual,        // <=
    Greater,          // >
    GreaterEqual,     // >=
    PlusEqual,        // +=
    MinusEqual,       // -=
    StarEqual,        // *=
    SlashEqual,       // /=
    LeftShift,        // <<
    RightShift,       // >>
    Pipe,             // |
    Caret,            // ^
    Ampersand,        // &
    Tilde,            // ~

    // 分隔符
    LeftParen,    // (
    RightParen,   // )
    LeftBracket,  // [
    RightBracket, // ]
    LeftBrace,    // {
    RightBrace,   // }
    Comma,        // ,
    Dot,          // .
    Colon,        // :
    Semicolon,    // ;
    Arrow,        // ->

    // Python 特有
    Indent,  // 缩进
    Dedent,  // 反缩进
    Newline, // 换行

    // 特殊
    Whitespace,
    Comment,
    Eof,
}

impl TokenType for PythonTokenType {
    const END_OF_STREAM: Self = Self::Eof;
}
