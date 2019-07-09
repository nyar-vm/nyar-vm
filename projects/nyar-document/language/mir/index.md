# MIR (中级中间表示)

MIR (Mid-level Intermediate Representation) 是 Nyar 虚拟机平台中的第三层中间表示，负责将 HIR 中富有表现力的、嵌套的表达式结构**线性化**为更接近机器执行模型的**控制流图 (CFG)**。它是为代数效应和异步等高级非局部控制流进行建模、分析和解释的核心。

## 概述

MIR 作为 Nyar 平台面向效应和解释的控制流图层，专门为支持代数效应、原生协程和树状栈而设计。与传统的中间表示不同，MIR 将延续作为一等公民，天然适合解释器执行，为 Nyar 虚拟机的高级控制流特性提供了坚实的基础。

## 从 HIR 到 MIR 的转换

### 转换流程

```
HIR (语义树)
    ↓
控制流显式化
    ↓
基本块构建
    ↓
延续建模
    ↓
MIR (控制流图)
```

### 核心转换过程

#### **1. 控制流显式化**

将 HIR 中的嵌套表达式结构转换为显式的控制流图：

```rust
// HIR 中的 if 表达式
If {
    cond: Binary {
        op: Lt,
        left: Local { id: 0 },
        right: Literal { value: 10 }
    },
    then_branch: Block { /* ... */ },
    else_branch: Some(Block { /* ... */ })
}

// 转换为 MIR 中的基本块序列
BasicBlocks {
    // 条件计算块
    bb0: BasicBlock {
        statements: [
            Statement {
                kind: Assign(
                    Place { local: 1 },
                    Rvalue::BinaryOp(
                        BinOp::Lt,
                        Operand::Copy(Place { local: 0 }),
                        Operand::Const(Const { value: 10 })
                    )
                )
            }
        ],
        terminator: SwitchInt {
            discr: Operand::Move(Place { local: 1 }),
            cases: vec![(0, bb3)], // false -> bb3
            default: bb2 // true -> bb2
        }
    },
    // then 分支块
    bb2: BasicBlock {
        statements: [/* then branch statements */],
        terminator: Goto { target: bb4 } // 汇合点
    },
    // else 分支块
    bb3: BasicBlock {
        statements: [/* else branch statements */],
        terminator: Goto { target: bb4 } // 汇合点
    },
    // 汇合块
    bb4: BasicBlock {
        statements: [],
        terminator: Return
    }
}
```

**控制流显式化的关键特点**：
- **基本块分解**: 将复杂表达式分解为多个基本块
- **跳转关系明确**: 每个块的控制流出口都明确指定
- **汇合点处理**: 为分支汇合创建专门的基本块
- **线性执行**: 每个基本块内的指令线性执行

#### **2. 延续建模**

MIR 的设计深刻体现了延续的概念，将"下一步做什么"作为显式的块 ID：

```rust
// 函数调用的延续建模
Call {
    func: Callable::Internal(function_id),
    args: vec![Operand::Copy(Place { local: 0 })],
    destination: Place { local: 1 },
    target: bb_continue // 函数返回后的延续
}

// 代数效应的延续建模
Perform {
    effect: effect_id,
    args: vec![Operand::Copy(Place { local: 2 })],
    destination: Place { local: 3 },
    resume_target: bb_resume // 效应恢复后的延续
}

// 效应处理的延续建模
Handle {
    body_target: bb_try_body,
    handlers: vec![
        (effect_id, handler_function_id)
    ],
    exit_target: bb_normal_exit // 正常完成的延续
}
```

**延续建模的核心价值**：
- **一等公民**: 延续作为显式的块 ID 存在
- **非局部控制流**: 支持效应、异常等非局部跳转
- **恢复机制**: 明确指定控制流的恢复点
- **组合性**: 延续可以嵌套和组合

## MIR 核心数据结构

### **函数表示**

```rust
/// MIR 中函数的核心表示，一个基本块的集合
pub struct Function {
    pub blocks: Arena<BasicBlock>,
    pub entry_block: BlockId,
    pub local_decls: IndexVec<LocalId, LocalDecl>,
    pub arg_count: usize,
    pub return_ty: Ty,
}

/// 一个基本块：包含一系列线性语句和一个唯一的控制流出口
pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}
```

### **语句和操作**

```rust
/// 语句是线性的、非控制流的指令，如赋值、算术运算等
pub enum Statement {
    Assign(Place, RValue),
    StorageLive(LocalId),
    StorageDead(LocalId),
    Nop,
}

/// Place 代表一个可被赋值的位置，如一个局部变量
pub struct Place {
    pub local: LocalId,
    pub projection: Vec<PlaceElem>,
}

/// RValue 代表一个可以产生值的计算
pub enum RValue {
    Use(Operand), // 使用一个常量或另一个变量
    BinaryOp(BinOp, Operand, Operand), // 底层操作，非重载
    UnaryOp(UnOp, Operand),
    Cast(CastKind, Operand, Ty),
    Aggregate(AggregateKind, Vec<Operand>),
}

/// Operand 代表一个值，可以是常量或变量
pub enum Operand {
    Constant(Constant),
    Place(Place),
}
```

### **终止符 - MIR 的灵魂**

```rust
/// 终止符定义了所有可能的控制流转移
pub enum Terminator {
    /// 无条件跳转到目标块
    Goto { target: BlockId },
    
    /// 条件分支，根据一个整数判别式跳转
    SwitchConditional {
        discr: Operand,
        cases: Vec<(u64, BlockId)>,
        default: BlockId,
    },
    
    /// 从当前函数返回
    Return,
    
    /// 函数调用
    Call {
        /// call 能区分内部调用和外部 Wasm 调用
        call: Callable,
        args: Vec<Operand>,
        /// 返回值写入这个位置
        destination: Place,
        /// 函数返回后，跳转到这个块继续执行
        target: BlockId,
    },
    
    /// 执行一个代数效应
    Perform {
        effect: EffectId,
        args: Vec<Operand>,
        /// resume(value) 中的 value 将被写入这个位置
        destination: Place,
        /// 效应被恢复后，跳转到这个块继续执行
        resume_target: BlockId,
    },
    
    /// handle 表达式的表示
    Handle {
        /// try { ... } 代码块的入口
        body_target: BlockId,
        /// 效应到其处理函数的映射
        handlers: Vec<(EffectId, FunctionId)>,
        /// 当 body 正常完成（没有 perform）时，跳转到这个块
        exit_target: BlockId,
    },
}

pub enum Callable {
    Internal(FunctionId),
    ExternalWasm(ExternalFuncId),
}
```

## 语言特性在 MIR 中的表示

### **代数效应系统**

```rust
// 效应定义
pub struct EffectDef {
    pub id: EffectId,
    pub name: Symbol,
    pub operations: Vec<EffectOperation>,
}

pub struct EffectOperation {
    pub name: Symbol,
    pub param_tys: Vec<Ty>,
    pub return_ty: Ty,
}

// 效应处理器
pub struct EffectHandler {
    pub effect: EffectId,
    pub operation: Symbol,
    pub handler_fn: FunctionId,
}
```

### **原生协程支持**

```rust
// 协程状态（无需状态机，使用树状栈）
pub struct CoroutineState {
    pub current_block: BlockId,
    pub locals: IndexVec<LocalId, Value>,
    pub effect_stack: EffectStack,
}

// 树状栈结构
pub struct EffectStack {
    pub frames: Vec<EffectFrame>,
}

pub struct EffectFrame {
    pub handler: EffectHandler,
    pub continuation: BlockId,
    pub locals_snapshot: IndexVec<LocalId, Value>,
}
```

### **模式匹配编译**

```rust
// 模式匹配决策树
pub enum MatchDecision {
    Switch {
        place: Place,
        cases: Vec<(ConstValue, BlockId)>,
        default: BlockId,
    },
    Guard {
        condition: Place,
        success: BlockId,
        failure: BlockId,
    },
    Leaf {
        bindings: Vec<(LocalId, Place)>,
        target: BlockId,
    },
}
```

## MIR 的优化机会

### **控制流优化**

```rust
// 基本块合并
// 优化前
bb0: BasicBlock {
    statements: [stmt1],
    terminator: Goto { target: bb1 }
}
bb1: BasicBlock {
    statements: [stmt2],
    terminator: Return
}

// 优化后
bb0: BasicBlock {
    statements: [stmt1, stmt2],
    terminator: Return
}
```

### **效应优化**

```rust
// 效应内联
// 优化前：简单效应可以内联
Perform {
    effect: simple_effect,
    args: [arg],
    destination: dest,
    resume_target: bb_resume
}

// 优化后：直接调用处理函数
Call {
    func: Callable::Internal(handler_fn),
    args: [arg],
    destination: dest,
    target: bb_resume
}
```

### **数据流优化**

```rust
// 死代码消除（基于活跃性分析）
// 优化前
bb0: BasicBlock {
    statements: [
        Assign(Place { local: 1 }, RValue::Use(Operand::Const(42))), // 死代码
        Assign(Place { local: 2 }, RValue::Use(Operand::Const(24)))
    ],
    terminator: Return
}

// 优化后
bb0: BasicBlock {
    statements: [
        Assign(Place { local: 2 }, RValue::Use(Operand::Const(24)))
    ],
    terminator: Return
}
```

## 解释器执行模型

### **VM 主循环**

```rust
pub struct Interpreter {
    pub current_function: FunctionId,
    pub current_block: BlockId,
    pub locals: IndexVec<LocalId, Value>,
    pub effect_stack: EffectStack,
}

impl Interpreter {
    pub fn run(&mut self) -> InterpResult<Value> {
        loop {
            let block = self.get_current_block();
            
            // 执行当前块的所有语句
            for statement in &block.statements {
                self.execute_statement(statement)?;
            }
            
            // 根据终止符更新状态
            match &block.terminator {
                Terminator::Goto { target } => {
                    self.current_block = *target;
                }
                Terminator::Return => {
                    return Ok(self.get_return_value());
                }
                Terminator::Call { func, args, destination, target } => {
                    let result = self.call_function(func, args)?;
                    self.write_place(*destination, result);
                    self.current_block = *target;
                }
                Terminator::Perform { effect, args, destination, resume_target } => {
                    self.perform_effect(*effect, args, *destination, *resume_target)?;
                }
                // ... 其他终止符处理
            }
        }
    }
}
```

### **效应处理机制**

```rust
impl Interpreter {
    fn perform_effect(
        &mut self,
        effect: EffectId,
        args: &[Operand],
        destination: Place,
        resume_target: BlockId
    ) -> InterpResult<()> {
        // 查找效应处理器
        if let Some(handler) = self.find_effect_handler(effect) {
            // 保存当前状态到效应栈
            let frame = EffectFrame {
                handler: handler.clone(),
                continuation: resume_target,
                locals_snapshot: self.locals.clone(),
            };
            self.effect_stack.push(frame);
            
            // 调用处理函数
            let handler_args = self.eval_operands(args)?;
            self.call_function(&Callable::Internal(handler.handler_fn), &handler_args)?;
        } else {
            return Err(InterpError::UnhandledEffect(effect));
        }
        
        Ok(())
    }
    
    fn resume_effect(&mut self, value: Value) -> InterpResult<()> {
        if let Some(frame) = self.effect_stack.pop() {
            // 恢复状态
            self.locals = frame.locals_snapshot;
            self.current_block = frame.continuation;
            
            // 写入恢复值
            // destination 需要从 frame 中获取
            
            Ok(())
        } else {
            Err(InterpError::NoEffectToResume)
        }
    }
}
```

## 错误处理和诊断

### **MIR 验证**

```rust
pub enum MIRError {
    // 控制流错误
    UnreachableBlock { block: BlockId },
    InvalidJumpTarget { from: BlockId, to: BlockId },
    MissingTerminator { block: BlockId },
    
    // 类型错误
    TypeMismatch { place: Place, expected: Ty, found: Ty },
    InvalidOperand { operand: Operand, context: String },
    
    // 效应错误
    UnhandledEffect { effect: EffectId, location: Location },
    InvalidEffectHandler { effect: EffectId, handler: FunctionId },
    
    // 局部变量错误
    UseOfUninitializedLocal { local: LocalId, location: Location },
    InvalidLocalAccess { local: LocalId, reason: String },
}
```

### **调试支持**

```rust
pub struct DebugInfo {
    pub source_map: IndexVec<BlockId, SourceInfo>,
    pub local_names: IndexVec<LocalId, Option<Symbol>>,
    pub effect_trace: Vec<EffectTraceEntry>,
}

pub struct EffectTraceEntry {
    pub effect: EffectId,
    pub operation: Symbol,
    pub location: Location,
    pub stack_depth: usize,
}
```

## 性能优化

### **构建效率**
- **增量构建**: 只重新构建修改的函数的 MIR
- **并行构建**: 独立函数的 MIR 构建可以并行进行
- **缓存机制**: 缓存控制流分析结果
- **惰性分析**: 按需进行复杂的程序分析

### **解释器优化**
- **字节码缓存**: 缓存编译后的基本块
- **热点检测**: 识别频繁执行的代码路径
- **内联优化**: 内联简单的函数调用
- **效应栈优化**: 优化效应栈的管理开销

### **内存管理**
- **GC 集成**: 与垃圾收集器紧密集成
- **对象池**: 使用对象池管理 MIR 节点
- **紧凑表示**: 使用紧凑的数据结构
- **共享数据**: 共享不可变的类型信息

## 总结

MIR 作为 Nyar 平台面向效应和解释的控制流图层，专门为支持代数效应、原生协程和高级控制流而设计。主要特点包括：

1. **控制流显式化**: 将嵌套表达式转换为显式的基本块图
2. **延续一等公民**: 将延续作为显式的块 ID 建模
3. **效应原生支持**: 内建对代数效应的支持
4. **解释器优化**: 天然适合解释器执行
5. **原生协程**: 支持原生协程而非状态机
6. **树状栈**: 使用树状栈而非传统调用栈

通过 MIR，Nyar 平台能够高效地支持现代编程语言的高级特性，为开发者提供强大的抽象能力，同时保持优秀的运行时性能。MIR 的设计哲学体现了对延续、效应和协程的深刻理解，为构建下一代编程语言运行时奠定了坚实基础。