//! Mini C 抽象语法树定义

use serde::{Deserialize, Serialize};

/// C 程序
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

/// 声明
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Declaration {
    /// 函数声明
    Function {
        return_type: Type,
        name: String,
        parameters: Vec<Parameter>,
        body: Option<CompoundStatement>,
    },
    /// 变量声明
    Variable {
        type_: Type,
        name: String,
        initializer: Option<Expression>,
    },
    /// 结构体声明
    Struct {
        name: String,
        fields: Vec<StructField>,
    },
    /// 预处理器指令
    Preprocessor { directive: String, content: String },
}

/// 语句
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    /// 复合语句 (代码块)
    Compound(CompoundStatement),
    /// 表达式语句
    Expression(Option<Expression>),
    /// 返回语句
    Return(Option<Expression>),
    /// 条件语句
    If {
        condition: Expression,
        then_stmt: Box<Statement>,
        else_stmt: Option<Box<Statement>>,
    },
    /// while 循环
    While {
        condition: Expression,
        body: Box<Statement>,
    },
    /// for 循环
    For {
        init: Option<Expression>,
        condition: Option<Expression>,
        update: Option<Expression>,
        body: Box<Statement>,
    },
    /// break 语句
    Break,
    /// continue 语句
    Continue,
    /// switch 语句
    Switch {
        expression: Expression,
        body: Box<Statement>,
    },
    /// case 标签
    Case {
        value: Expression,
        statement: Box<Statement>,
    },
    /// default 标签
    Default(Box<Statement>),
    /// goto 语句
    Goto(String),
    /// 标签语句
    Label {
        name: String,
        statement: Box<Statement>,
    },
    /// 变量声明语句
    Declaration(Declaration),
}

/// 复合语句 (代码块)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompoundStatement {
    pub statements: Vec<Statement>,
}

/// 表达式
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    /// 字面量
    Literal(Literal),
    /// 标识符
    Identifier(String),
    /// 二元运算
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
    /// 一元运算
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },
    /// 赋值表达式
    Assignment {
        left: Box<Expression>,
        operator: AssignmentOperator,
        right: Box<Expression>,
    },
    /// 函数调用
    Call {
        function: Box<Expression>,
        arguments: Vec<Expression>,
    },
    /// 数组访问
    ArrayAccess {
        array: Box<Expression>,
        index: Box<Expression>,
    },
    /// 成员访问 (.)
    MemberAccess {
        object: Box<Expression>,
        member: String,
    },
    /// 指针成员访问 (->)
    PointerAccess {
        pointer: Box<Expression>,
        member: String,
    },
    /// 条件表达式 (三元运算符)
    Conditional {
        condition: Box<Expression>,
        true_expr: Box<Expression>,
        false_expr: Box<Expression>,
    },
    /// sizeof 表达式
    Sizeof(Box<Expression>),
    /// 类型转换
    Cast {
        type_: Box<Type>,
        expression: Box<Expression>,
    },
    /// 逗号表达式
    Comma(Vec<Expression>),
}

/// 字面量
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    /// 整数
    Integer(i64),
    /// 浮点数
    Float(f64),
    /// 字符
    Character(char),
    /// 字符串
    String(String),
}

/// 二元运算符
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BinaryOperator {
    // 算术运算符
    Add,      // +
    Subtract, // -
    Multiply, // *
    Divide,   // /
    Modulo,   // %

    // 比较运算符
    Equal,        // ==
    NotEqual,     // !=
    Less,         // <
    LessEqual,    // <=
    Greater,      // >
    GreaterEqual, // >=

    // 逻辑运算符
    LogicalAnd, // &&
    LogicalOr,  // ||

    // 位运算符
    BitwiseAnd, // &
    BitwiseOr,  // |
    BitwiseXor, // ^
    LeftShift,  // <<
    RightShift, // >>
}

/// 一元运算符
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Plus,          // +
    Minus,         // -
    LogicalNot,    // !
    BitwiseNot,    // ~
    Dereference,   // *
    AddressOf,     // &
    PreIncrement,  // ++expr
    PostIncrement, // expr++
    PreDecrement,  // --expr
    PostDecrement, // expr--
}

/// 赋值运算符
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssignmentOperator {
    Assign,           // =
    AddAssign,        // +=
    SubAssign,        // -=
    MulAssign,        // *=
    DivAssign,        // /=
    ModAssign,        // %=
    AndAssign,        // &=
    OrAssign,         // |=
    XorAssign,        // ^=
    LeftShiftAssign,  // <<=
    RightShiftAssign, // >>=
}

/// 类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    /// 基本类型
    Basic(BasicType),
    /// 指针类型
    Pointer(Box<Type>),
    /// 数组类型
    Array {
        element_type: Box<Type>,
        size: Option<Box<Expression>>,
    },
    /// 函数类型
    Function {
        return_type: Box<Type>,
        parameters: Vec<Type>,
    },
    /// 结构体类型
    Struct(String),
    /// 联合体类型
    Union(String),
    /// 枚举类型
    Enum(String),
    /// typedef 类型
    Typedef(String),
}

/// 基本类型
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BasicType {
    Void,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
    Signed,
    Unsigned,
}

/// 函数参数
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub type_: Type,
    pub name: Option<String>,
}

/// 结构体字段
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructField {
    pub type_: Type,
    pub name: String,
}

impl Program {
    /// 创建一个新的程序
    pub fn new() -> Self {
        Self {
            declarations: Vec::new(),
        }
    }

    /// 添加声明
    pub fn add_declaration(&mut self, declaration: Declaration) {
        self.declarations.push(declaration);
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

impl CompoundStatement {
    /// 创建一个新的复合语句
    pub fn new() -> Self {
        Self {
            statements: Vec::new(),
        }
    }

    /// 添加语句
    pub fn add_statement(&mut self, statement: Statement) {
        self.statements.push(statement);
    }
}

impl Default for CompoundStatement {
    fn default() -> Self {
        Self::new()
    }
}

impl Expression {
    /// 创建一个标识符表达式
    pub fn identifier(name: impl Into<String>) -> Self {
        Self::Identifier(name.into())
    }

    /// 创建一个字符串字面量表达式
    pub fn string(value: impl Into<String>) -> Self {
        Self::Literal(Literal::String(value.into()))
    }

    /// 创建一个整数字面量表达式
    pub fn integer(value: i64) -> Self {
        Self::Literal(Literal::Integer(value))
    }

    /// 创建一个浮点数字面量表达式
    pub fn float(value: f64) -> Self {
        Self::Literal(Literal::Float(value))
    }

    /// 创建一个字符字面量表达式
    pub fn character(value: char) -> Self {
        Self::Literal(Literal::Character(value))
    }
}

impl Declaration {
    /// 创建一个函数声明
    pub fn function(
        return_type: Type,
        name: impl Into<String>,
        parameters: Vec<Parameter>,
        body: Option<CompoundStatement>,
    ) -> Self {
        Self::Function {
            return_type,
            name: name.into(),
            parameters,
            body,
        }
    }

    /// 创建一个变量声明
    pub fn variable(type_: Type, name: impl Into<String>, initializer: Option<Expression>) -> Self {
        Self::Variable {
            type_,
            name: name.into(),
            initializer,
        }
    }
}

impl Statement {
    /// 创建一个表达式语句
    pub fn expression(expr: Option<Expression>) -> Self {
        Self::Expression(expr)
    }

    /// 创建一个返回语句
    pub fn return_stmt(value: Option<Expression>) -> Self {
        Self::Return(value)
    }

    /// 创建一个复合语句
    pub fn compound(statements: Vec<Statement>) -> Self {
        Self::Compound(CompoundStatement { statements })
    }
}
