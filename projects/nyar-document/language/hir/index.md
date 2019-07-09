# HIR (高级中间表示)

InHIR (High-level Intermediate Representation) 是 Nyar 虚拟机平台中的第二层中间表示，负责从 AST 进行语义分析和类型检查，生成类型安全且语义明确的程序表示。HIR 消除了语法歧义，为后续的 MIR 转换提供清晰的语义基础。

## 概述

HIR 作为 Nyar 平台的语义分析层，接收来自 AST 的语法树，通过名称解析、类型检查和语法糖消除，生成语义完整的中间表示。HIR 保留了高级语言的语义特性，同时为后续优化提供了类型安全的基础。

## 从 AST 到 HIR 的转换

### 转换流程

```
AST (语法树)
    ↓
名称解析
    ↓
类型推导
    ↓
语义检查
    ↓
语法糖消除
    ↓
HIR (语义树)
```

### 核心转换过程

#### **1. 名称解析阶段**

将 AST 中的标识符绑定到具体定义：

```rust
// AST 中的标识符引用
Identifier { name: "foo" }

// 转换为 HIR 中的路径引用
Path {
    resolution: DefId("module::foo"),
    segments: ["foo"],
    def_kind: Function,
    ty: FunctionType { params: [Int], return_type: String }
}
```

**名称解析处理**：
- **符号表构建**: 建立分层的符号表结构
- **作用域分析**: 确定标识符的可见性范围
- **路径解析**: 解析复杂的模块路径和限定名
- **重载解析**: 处理函数和方法的重载消歧义
- **可见性检查**: 验证访问权限和模块边界

#### **2. 类型推导阶段**

使用 Hindley-Milner 算法进行类型推导：

```rust
// AST 中的函数调用
CallExpr {
    func: Identifier { name: "map" },
    args: [
        Identifier { name: "numbers" },
        LambdaExpr {
            params: ["x"],
            body: BinaryExpr { op: Add, left: "x", right: Literal(1) }
        }
    ]
}

// 转换为 HIR 中的类型化调用
Call {
    func: Path {
        def_id: DefId("std::iter::map"),
        ty: FunctionType {
            params: [Array(T), Function([T], U)],
            return_type: Array(U),
            constraints: [T: Clone, U: Clone]
        }
    },
    args: [
        Path {
            def_id: DefId("local::numbers"),
            ty: Array(Int32)
        },
        Closure {
            params: [Param { name: "x", ty: Int32 }],
            body: Binary {
                op: Add,
                left: Local { id: 0, ty: Int32 },
                right: Literal { value: 1, ty: Int32 },
                ty: Int32
            },
            captures: [],
            ty: Function([Int32], Int32)
        }
    ],
    ty: Array(Int32)
}
```

**类型推导处理**：
- **约束收集**: 收集表达式间的类型约束
- **约束求解**: 使用统一算法求解类型方程
- **泛型实例化**: 将泛型类型绑定到具体类型
- **类型检查**: 验证类型兼容性和约束满足
- **错误诊断**: 生成详细的类型错误信息

#### **3. 语义检查阶段**

验证程序的语义正确性：

```rust
// AST 中的模式匹配
MatchExpr {
    expr: Identifier { name: "value" },
    arms: [
        MatchArm {
            pattern: ConstructorPattern { name: "Some", args: ["x"] },
            body: Identifier { name: "x" }
        },
        MatchArm {
            pattern: ConstructorPattern { name: "None", args: [] },
            body: Literal(0)
        }
    ]
}

// 转换为 HIR 中的完整性检查匹配
Match {
    scrutinee: Local { id: 0, ty: Option(Int32) },
    arms: [
        Arm {
            pattern: Constructor {
                def_id: DefId("std::option::Some"),
                subpatterns: [
                    Binding { name: "x", mode: ByValue, ty: Int32 }
                ]
            },
            guard: None,
            body: Local { id: 1, ty: Int32 }
        },
        Arm {
            pattern: Constructor {
                def_id: DefId("std::option::None"),
                subpatterns: []
            },
            guard: None,
            body: Literal { value: 0, ty: Int32 }
        }
    ],
    ty: Int32,
    exhaustiveness: Complete
}
```

**语义检查处理**：
- **完整性检查**: 验证模式匹配的完整性
- **可达性分析**: 检测不可达的代码分支
- **借用检查**: 分析引用的生命周期和所有权
- **效应验证**: 检查代数效应的正确使用
- **控制流分析**: 验证控制流的合法性

#### **4. 语法糖消除阶段**

将高级语法结构转换为基础形式：

```rust
// AST 中的 for 循环语法糖
ForExpr {
    pattern: "item",
    iter: Identifier { name: "items" },
    body: CallExpr {
        func: "process",
        args: ["item"]
    }
}

// 转换为 HIR 中的迭代器调用
Call {
    func: Path {
        def_id: DefId("std::iter::Iterator::for_each"),
        ty: FunctionType {
            params: [Self, Function([Self::Item], ())],
            return_type: ()
        }
    },
    args: [
        Call {
            func: Path { def_id: DefId("std::iter::IntoIterator::into_iter") },
            args: [Path { def_id: DefId("local::items") }]
        },
        Closure {
            params: [Param { name: "item", ty: T }],
            body: Call {
                func: Path { def_id: DefId("local::process") },
                args: [Local { id: 0 }]
            }
        }
    ]
}
```

**语法糖消除处理**：
- **操作符降糖**: 将操作符转换为方法调用
- **控制流简化**: 将复杂控制流转换为基本形式
- **语法便利性消除**: 移除各种语法糖和简写
- **标准化表示**: 统一相似语义的不同语法形式

## HIR 核心数据结构

### **程序结构节点**

```rust
pub struct HIRCrate {
    pub modules: Vec<HIRModule>,
    pub items: Vec<HIRItem>,
    pub def_map: DefMap,
    pub type_map: TypeMap,
}

pub struct HIRModule {
    pub name: Symbol,
    pub items: Vec<ItemId>,
    pub visibility: Visibility,
    pub span: Span,
}

pub enum HIRItem {
    Function(HIRFunction),
    Struct(HIRStruct),
    Enum(HIREnum),
    Trait(HIRTrait),
    Impl(HIRImpl),
    Const(HIRConst),
    Effect(HIREffect),
}
```

### **表达式节点**

```rust
pub struct HIRExpr {
    pub kind: HIRExprKind,
    pub ty: Ty,
    pub span: Span,
}

pub enum HIRExprKind {
    // 基础表达式
    Literal(Literal),
    Path(HIRPath),
    
    // 函数调用
    Call { func: Box<HIRExpr>, args: Vec<HIRExpr> },
    MethodCall { receiver: Box<HIRExpr>, method: DefId, args: Vec<HIRExpr> },
    
    // 控制流
    If { cond: Box<HIRExpr>, then_branch: HIRBlock, else_branch: Option<HIRBlock> },
    Match { scrutinee: Box<HIRExpr>, arms: Vec<HIRArm> },
    Loop { body: HIRBlock, label: Option<Label> },
    
    // 函数式特性
    Closure { params: Vec<HIRParam>, body: HIRBlock, captures: Vec<Capture> },
    
    // 代数效应
    Handle { expr: Box<HIRExpr>, handlers: Vec<HIRHandler> },
    Perform { effect: DefId, operation: Symbol, args: Vec<HIRExpr> },
    
    // 数据操作
    Struct { def_id: DefId, fields: Vec<HIRFieldExpr> },
    Tuple { elements: Vec<HIRExpr> },
    Array { elements: Vec<HIRExpr> },
    Index { base: Box<HIRExpr>, index: Box<HIRExpr> },
    Field { base: Box<HIRExpr>, field: Symbol },
    
    // 引用和解引用
    AddrOf { mutability: Mutability, expr: Box<HIRExpr> },
    Deref { expr: Box<HIRExpr> },
    
    // 类型转换
    Cast { expr: Box<HIRExpr>, target_ty: Ty },
    
    // 赋值
    Assign { lhs: Box<HIRExpr>, rhs: Box<HIRExpr> },
    AssignOp { op: BinOp, lhs: Box<HIRExpr>, rhs: Box<HIRExpr> },
}
```

### **类型系统**

```rust
pub enum Ty {
    // 基础类型
    Bool,
    Int(IntTy),
    Float(FloatTy),
    Char,
    Str,
    
    // 复合类型
    Tuple(Vec<Ty>),
    Array(Box<Ty>, usize),
    Slice(Box<Ty>),
    
    // 用户定义类型
    Adt(DefId, Vec<Ty>), // 代数数据类型
    
    // 函数类型
    Function(FnSig),
    Closure(ClosureSig),
    
    // 引用类型
    Ref(Region, Box<Ty>, Mutability),
    RawPtr(Box<Ty>, Mutability),
    
    // 泛型和关联类型
    Param(ParamTy),
    Projection(ProjectionTy),
    
    // 效应类型
    Effect(EffectTy),
    
    // 特殊类型
    Never,
    Infer(InferTy),
    Error,
}

pub struct FnSig {
    pub params: Vec<Ty>,
    pub return_ty: Box<Ty>,
    pub effects: Vec<EffectTy>,
    pub abi: Abi,
}
```

### **模式匹配**

```rust
pub struct HIRPat {
    pub kind: HIRPatKind,
    pub ty: Ty,
    pub span: Span,
}

pub enum HIRPatKind {
    Wild,
    Binding { name: Symbol, mode: BindingMode, subpat: Option<Box<HIRPat>> },
    Literal(Literal),
    Range { start: Option<Literal>, end: Option<Literal>, inclusive: bool },
    Tuple { elements: Vec<HIRPat> },
    Struct { def_id: DefId, fields: Vec<HIRFieldPat>, etc: bool },
    Constructor { def_id: DefId, subpats: Vec<HIRPat> },
    Or { pats: Vec<HIRPat> },
    Slice { prefix: Vec<HIRPat>, slice: Option<Box<HIRPat>>, suffix: Vec<HIRPat> },
}
```

## 语言特性在 HIR 中的表示

### **代数效应系统**

```rust
// 效应声明的 HIR 表示
pub struct HIREffect {
    pub name: Symbol,
    pub operations: Vec<HIROperation>,
    pub generics: Generics,
    pub span: Span,
}

pub struct HIROperation {
    pub name: Symbol,
    pub sig: FnSig,
    pub span: Span,
}

// 效应处理的 HIR 表示
pub struct HIRHandler {
    pub effect: DefId,
    pub operation: Symbol,
    pub params: Vec<HIRParam>,
    pub body: HIRBlock,
    pub resume_ty: Ty,
}
```

### **模式匹配编译**

```rust
// 决策树表示
pub enum DecisionTree {
    Leaf { arm: ArmId },
    Switch {
        scrutinee: Place,
        targets: Vec<(ConstructorId, DecisionTree)>,
        otherwise: Option<Box<DecisionTree>>,
    },
    Guard {
        guard: HIRExpr,
        success: Box<DecisionTree>,
        failure: Box<DecisionTree>,
    },
}
```

### **闭包捕获分析**

```rust
pub struct Capture {
    pub var: DefId,
    pub mode: CaptureMode,
    pub ty: Ty,
}

pub enum CaptureMode {
    ByValue,
    ByRef(Mutability),
    ByMove,
}
```

## HIR 的优化机会

### **高级优化**

HIR 阶段可以进行一些保持语义的高级优化：

#### **常量传播和折叠**
```rust
// 优化前
Binary {
    op: Add,
    left: Literal { value: 2, ty: Int32 },
    right: Literal { value: 3, ty: Int32 },
    ty: Int32
}

// 优化后
Literal { value: 5, ty: Int32 }
```

#### **死代码消除**
```rust
// 优化前
If {
    cond: Literal { value: false, ty: Bool },
    then_branch: Block { /* 永远不执行 */ },
    else_branch: Some(Block { /* 总是执行 */ })
}

// 优化后
Block { /* 总是执行的分支 */ }
```

#### **内联决策**
```rust
// 标记小函数为内联候选
HIRFunction {
    name: "small_helper",
    body: /* 简单的函数体 */,
    attributes: [Inline(Always)],
    complexity: Low
}
```

### **特化优化**

针对特定语言特性的优化：

- **效应优化**: 消除不必要的效应处理开销
- **模式匹配优化**: 生成高效的决策树
- **闭包优化**: 减少不必要的变量捕获
- **泛型特化**: 为常用类型生成特化版本

## 错误处理和诊断

### **语义错误类型**

```rust
pub enum HIRError {
    // 类型错误
    TypeMismatch { expected: Ty, found: Ty, span: Span },
    UnresolvedName { name: Symbol, span: Span },
    
    // 模式匹配错误
    NonExhaustiveMatch { missing_patterns: Vec<Pattern>, span: Span },
    UnreachablePattern { pattern: Pattern, span: Span },
    
    // 效应错误
    UnhandledEffect { effect: DefId, operation: Symbol, span: Span },
    EffectMismatch { expected: Vec<EffectTy>, found: Vec<EffectTy>, span: Span },
    
    // 借用检查错误
    BorrowError { kind: BorrowErrorKind, span: Span },
    LifetimeError { kind: LifetimeErrorKind, span: Span },
}
```

### **错误恢复策略**

- **部分类型推导**: 在类型错误时继续推导其他部分
- **错误类型插入**: 使用错误类型占位符保持 HIR 结构
- **最佳努力分析**: 尽可能完成语义分析提供更多诊断
- **上下文保留**: 保留错误上下文用于高质量诊断

## 性能优化

### **构建效率**
- **增量分析**: 只重新分析修改的代码部分
- **并行类型检查**: 独立模块的并行语义分析
- **缓存机制**: 缓存类型推导和名称解析结果
- **惰性求值**: 按需进行复杂的语义分析

### **内存优化**
- **类型驻留**: 相同类型的共享存储
- **符号表优化**: 高效的符号查找和存储
- **HIR 压缩**: 紧凑的 HIR 节点表示
- **生命周期管理**: 及时释放不需要的分析数据

## 总结

HIR 作为 Nyar 平台的语义分析层，通过系统化的转换过程将 AST 转换为类型安全、语义明确的中间表示。主要特点包括：

1. **语义完整性**: 完整的类型信息和名称绑定
2. **语法糖消除**: 统一的语义表示形式
3. **错误恢复**: 优雅的错误处理和诊断
4. **优化基础**: 为后续优化提供语义保证
5. **特性支持**: 全面支持现代语言特性

通过 HIR，Nyar 平台能够为不同的前端语言提供统一的语义分析和类型检查，确保程序的正确性和安全性，为后续的 MIR 转换和优化奠定坚实基础。