use gaia_types::lexer::TokenType;

/// C Token 类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CTokenType {
    // 关键字
    Auto,     // auto
    Break,    // break
    Case,     // case
    Char,     // char
    Const,    // const
    Continue, // continue
    Default,  // default
    Do,       // do
    Double,   // double
    Else,     // else
    Enum,     // enum
    Extern,   // extern
    Float,    // float
    For,      // for
    Goto,     // goto
    If,       // if
    Int,      // int
    Long,     // long
    Register, // register
    Return,   // return
    Short,    // short
    Signed,   // signed
    Sizeof,   // sizeof
    Static,   // static
    Struct,   // struct
    Switch,   // switch
    Typedef,  // typedef
    Union,    // union
    Unsigned, // unsigned
    Void,     // void
    Volatile, // volatile
    While,    // while

    // 标识符和字面量
    Identifier,
    IntegerLiteral,
    FloatLiteral,
    CharLiteral,
    StringLiteral,

    // 运算符
    Plus,            // +
    Minus,           // -
    Star,            // *
    Slash,           // /
    Percent,         // %
    Equal,           // =
    EqualEqual,      // ==
    BangEqual,       // !=
    Less,            // <
    LessEqual,       // <=
    Greater,         // >
    GreaterEqual,    // >=
    PlusEqual,       // +=
    MinusEqual,      // -=
    StarEqual,       // *=
    SlashEqual,      // /=
    PercentEqual,    // %=
    LeftShift,       // <<
    RightShift,      // >>
    LeftShiftEqual,  // <<=
    RightShiftEqual, // >>=
    Pipe,            // |
    Caret,           // ^
    Ampersand,       // &
    Tilde,           // ~
    PipeEqual,       // |=
    CaretEqual,      // ^=
    AmpersandEqual,  // &=
    LogicalAnd,      // &&
    LogicalOr,       // ||
    Bang,            // !
    Question,        // ?
    Increment,       // ++
    Decrement,       // --
    Arrow,           // ->

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

    // 预处理器
    Hash,    // #
    Include, // include (预处理指令)
    Define,  // define (预处理指令)

    // 特殊
    Whitespace,
    Comment,
    Newline,
    Eof,
}

impl TokenType for CTokenType {
    const END_OF_STREAM: Self = Self::Eof;
}
