# AST (抽象语法树)

AST (Abstract Syntax Tree) 是 Nyar 虚拟机平台中的第一层中间表示，负责将各种前端语言的语法结构转换为统一的树形表示。AST 层专注于语法结构的忠实表示，为后续的语义分析和优化提供基础。

## 概述

AST 作为 Nyar 平台的入口层，接收来自各种语言前端（如 Valkyrie、TypeScript、Python 等）的语法树，并将其转换为标准化的 Nyar AST 格式。这种设计使得不同的源语言都能享受到 Nyar 平台提供的统一优化和后端支持。

## 支持的编程语言特性

### 🔧 **基础语言构造**

**表达式系统**:
- **字面量**: 数字、字符串、布尔值、字符
- **标识符**: 变量引用、函数引用
- **二元运算**: 算术、比较、逻辑、位运算
- **一元运算**: 取负、逻辑非、位非
- **函数调用**: 支持多参数、命名参数、可变参数
- **成员访问**: 结构体字段访问、方法调用

**控制流结构**:
- **条件表达式**: if-else、三元运算符
- **循环结构**: for、while、do-while 循环
- **跳转语句**: break、continue、return
- **异常处理**: try-catch-finally 结构

**数据结构**:
- **数组**: 固定大小和动态数组
- **元组**: 异构数据组合
- **结构体**: 命名字段的记录类型
- **枚举**: 代数数据类型和联合类型

### 🚀 **高级语言特性**

**函数式编程**:
- **高阶函数**: 函数作为一等公民
- **闭包**: 词法作用域和变量捕获
- **Lambda 表达式**: 匿名函数定义
- **柯里化**: 部分应用和函数组合
- **尾递归**: 尾调用优化支持

**模式匹配**:
- **结构化匹配**: 对复杂数据结构的解构
- **守卫条件**: 模式匹配中的额外条件
- **通配符模式**: 忽略不关心的值
- **变量绑定**: 匹配过程中的变量捕获
- **嵌套模式**: 深层结构的模式匹配

**代数效应系统**:
- **效应声明**: 定义可恢复的计算效应
- **效应处理**: handle 表达式和处理器
- **效应组合**: 多个效应的组合和交互
- **效应多态**: 泛型效应和效应约束

**类型系统**:
- **静态类型**: 编译时类型检查
- **类型推导**: Hindley-Milner 类型推导
- **泛型**: 参数化类型和类型参数
- **约束**: 类型类和接口约束
- **子类型**: 协变和逆变关系

### 🌐 **现代语言特性**

**异步编程**:
- **async/await**: 异步函数和等待表达式
- **Promise/Future**: 异步计算的抽象
- **协程**: 可暂停和恢复的函数
- **并发原语**: 通道、锁、原子操作

**模块系统**:
- **命名空间**: 层次化的名称管理
- **导入导出**: 模块间的依赖关系
- **可见性控制**: public、private、internal
- **条件编译**: 基于特性的代码选择

**元编程**:
- **宏系统**: 编译时代码生成
- **反射**: 运行时类型信息
- **属性**: 元数据和编译器指令
- **代码生成**: 程序化的代码构造

## 从前端语言到 AST 的转换

### 转换流程

```
源语言语法树
    ↓
语法适配层
    ↓
特性映射
    ↓
标准化处理
    ↓
Nyar AST
```

### 语言特性映射策略

#### **Valkyrie 语言映射**

```rust
// Valkyrie 代数效应
effect Http {
    fn get(url: String) -> String
}

// 映射到 Nyar AST
EffectDecl {
    name: "Http",
    operations: [
        OperationDecl {
            name: "get",
            params: [Param { name: "url", ty: StringType }],
            return_type: StringType
        }
    ]
}
```

#### **TypeScript 映射**

```typescript
// TypeScript 接口
interface User {
    name: string;
    age: number;
}

// 映射到 Nyar AST
StructDecl {
    name: "User",
    fields: [
        Field { name: "name", ty: StringType },
        Field { name: "age", ty: IntType }
    ]
}
```

#### **Python 映射**

```python
# Python 列表推导
result = [x * 2 for x in numbers if x > 0]

# 映射到 Nyar AST
CallExpr {
    func: "map",
    args: [
        LambdaExpr {
            params: ["x"],
            body: BinaryExpr { op: Mul, left: "x", right: 2 }
        },
        CallExpr {
            func: "filter",
            args: [
                LambdaExpr {
                    params: ["x"],
                    body: BinaryExpr { op: Gt, left: "x", right: 0 }
                },
                "numbers"
            ]
        }
    ]
}
```

### 核心 AST 节点类型

#### **程序结构节点**
```rust
pub enum ASTNode {
    // 程序和模块
    Program { items: Vec<Item> },
    Module { name: String, items: Vec<Item> },
    
    // 声明
    FunctionDecl { name: String, params: Vec<Param>, body: Block },
    TypeDecl { name: String, definition: TypeDef },
    ConstDecl { name: String, value: Expr },
    EffectDecl { name: String, operations: Vec<Operation> },
    
    // 表达式
    Literal { value: LiteralValue },
    Identifier { name: String },
    Call { func: Box<Expr>, args: Vec<Expr> },
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    Unary { op: UnOp, operand: Box<Expr> },
    If { cond: Box<Expr>, then_branch: Block, else_branch: Option<Block> },
    Match { expr: Box<Expr>, arms: Vec<MatchArm> },
    Lambda { params: Vec<Param>, body: Box<Expr> },
    Handle { expr: Box<Expr>, handlers: Vec<Handler> },
    
    // 语句
    ExprStmt { expr: Expr },
    Let { pattern: Pattern, init: Option<Expr> },
    Return { value: Option<Expr> },
    Break, Continue,
    
    // 类型
    PrimitiveType { kind: PrimitiveKind },
    FunctionType { params: Vec<Type>, return_type: Box<Type> },
    TupleType { elements: Vec<Type> },
    GenericType { name: String, args: Vec<Type> },
    
    // 模式
    WildcardPattern,
    LiteralPattern { value: LiteralValue },
    IdentPattern { name: String },
    TuplePattern { elements: Vec<Pattern> },
    ConstructorPattern { name: String, args: Vec<Pattern> },
}
```

#### **位置信息和元数据**
```rust
pub struct Span {
    pub file: FileId,
    pub start: Position,
    pub end: Position,
}

pub struct Position {
    pub line: u32,
    pub column: u32,
    pub offset: u32,
}

pub struct ASTNodeWithSpan<T> {
    pub node: T,
    pub span: Span,
    pub attributes: Vec<Attribute>,
}
```

### 语言特性适配

#### **类型系统适配**

不同语言的类型系统映射到统一的 Nyar 类型表示：

| 源语言类型 | Nyar AST 类型 | 说明 |
|-----------|---------------|------|
| `int` (Python) | `PrimitiveType::Int32` | 32位整数 |
| `number` (TypeScript) | `PrimitiveType::Float64` | 双精度浮点 |
| `String` (Valkyrie) | `PrimitiveType::String` | 字符串类型 |
| `List[T]` (Python) | `GenericType { name: "Array", args: [T] }` | 泛型数组 |
| `Promise<T>` (TypeScript) | `EffectType { effect: "Async", result: T }` | 异步效应 |

#### **控制流适配**

不同语言的控制流结构统一映射：

```rust
// Python for 循环
for item in items:
    process(item)

// 映射为 Nyar AST
CallExpr {
    func: "for_each",
    args: [
        "items",
        LambdaExpr {
            params: ["item"],
            body: CallExpr { func: "process", args: ["item"] }
        }
    ]
}

// JavaScript async/await
async function fetchData() {
    const result = await fetch(url);
    return result.json();
}

// 映射为 Nyar AST
FunctionDecl {
    name: "fetchData",
    params: [],
    effects: ["Async"],
    body: Block {
        stmts: [
            Let {
                pattern: "result",
                init: HandleExpr {
                    expr: CallExpr { func: "fetch", args: ["url"] },
                    handlers: [AsyncHandler]
                }
            },
            Return {
                value: CallExpr {
                    func: "json",
                    receiver: "result"
                }
            }
        ]
    }
}
```

### 错误处理和恢复

#### **语法错误处理**
```rust
pub enum ASTError {
    SyntaxError { message: String, span: Span },
    UnsupportedFeature { feature: String, span: Span },
    ConversionError { from: String, to: String, span: Span },
}

// 错误恢复节点
pub struct ErrorNode {
    pub error: ASTError,
    pub recovery_strategy: RecoveryStrategy,
    pub partial_ast: Option<Box<ASTNode>>,
}
```

#### **特性兼容性检查**
```rust
pub fn check_feature_compatibility(
    source_lang: &Language,
    feature: &LanguageFeature
) -> Result<(), CompatibilityError> {
    match (source_lang, feature) {
        (Language::Python, LanguageFeature::AlgebraicEffects) => {
            Err(CompatibilityError::UnsupportedFeature)
        },
        (Language::TypeScript, LanguageFeature::PatternMatching) => {
            Ok(()) // 可以通过库模拟
        },
        (Language::Valkyrie, _) => Ok(()), // 原生支持所有特性
        _ => Ok(())
    }
}
```

## 性能优化

### **内存效率**
- **节点共享**: 相同的子树在多处复用
- **字符串驻留**: 标识符和字面量去重存储
- **紧凑布局**: 使用枚举减少内存占用
- **惰性加载**: 按需构建复杂的 AST 子树

### **构建效率**
- **增量解析**: 只重新解析修改的代码部分
- **并行构建**: 独立模块的并行 AST 构建
- **缓存机制**: 缓存频繁访问的 AST 节点
- **流式处理**: 大文件的流式 AST 构建

### **转换优化**
- **批量转换**: 一次性转换多个相关节点
- **模式识别**: 识别常见的语法模式进行优化
- **预处理**: 在转换前进行语法规范化
- **并行映射**: 独立节点的并行转换处理

## 总结

Nyar AST 层通过支持丰富的编程语言特性和灵活的转换机制，为不同的前端语言提供了统一的入口点。主要优势包括：

1. **特性完整性**: 支持现代编程语言的所有主要特性
2. **转换灵活性**: 适配不同语言的语法差异
3. **性能优化**: 高效的内存使用和构建速度
4. **错误处理**: 优雅的错误恢复和诊断
5. **扩展性**: 易于添加新的语言特性支持

通过这种设计，语言实现者只需要关心如何将自己的语言转换为 Nyar AST，就能自动获得 Nyar 平台提供的所有后续优化和后端支持。