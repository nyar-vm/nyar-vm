# LIR (低级中间表示)

LIR (Low-level Intermediate Representation) 是 Nyar 虚拟机平台中的最终中间表示层，负责从 MIR 进行指令选择和寄存器分配准备，生成接近目标平台的低级程序表示。LIR 为多种后端（WebAssembly、JavaScript、原生代码）的代码生成提供统一的抽象基础。

## 概述

LIR 作为 Nyar 平台的代码生成准备层，接收来自 MIR 的控制流图，通过指令选择、寄存器虚拟化和平台抽象，生成适合多后端代码生成的线性指令表示。LIR 采用虚拟寄存器和线性指令序列，在保持平台无关性的同时提供接近机器码的抽象层次。

## 从 MIR 到 LIR 的转换

### 转换流程

```
MIR (控制流图)
    ↓
指令选择
    ↓
寄存器虚拟化
    ↓
控制流线性化
    ↓
平台抽象
    ↓
LIR (线性指令)
```

### 核心转换过程

#### **1. 指令选择阶段**

将 MIR 的高级操作转换为 LIR 的低级指令：

```rust
// MIR 中的二元操作
Statement {
    kind: Assign(
        Place { local: 2 },
        Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Copy(Place { local: 0 }),
            Operand::Copy(Place { local: 1 })
        )
    )
}

// 转换为 LIR 指令序列
LIRInstructions [
    // 加载操作数到虚拟寄存器
    LoadInst {
        dest: VReg(100),
        src: LocalSlot(0),
        ty: Int32
    },
    LoadInst {
        dest: VReg(101),
        src: LocalSlot(1),
        ty: Int32
    },
    // 执行加法运算
    ArithmeticInst {
        op: Add,
        dest: VReg(102),
        lhs: VReg(100),
        rhs: VReg(101),
        ty: Int32
    },
    // 存储结果
    StoreInst {
        src: VReg(102),
        dest: LocalSlot(2),
        ty: Int32
    }
]
```

**指令选择处理**：
- **操作分解**: 将复杂操作分解为基本指令
- **寻址模式**: 选择合适的内存寻址方式
- **类型特化**: 根据操作数类型选择特化指令
- **优化机会**: 识别指令合并和优化机会

#### **2. 寄存器虚拟化阶段**

将 MIR 的 Place 转换为 LIR 的虚拟寄存器：

```rust
// MIR 中的复杂 Place 访问
Place {
    local: 5,
    projection: [
        PlaceElem::Field(Field(0), StructTy),
        PlaceElem::Index(Local(3))
    ]
}

// 转换为 LIR 地址计算序列
LIRInstructions [
    // 计算结构体字段地址
    AddressInst {
        dest: VReg(200),
        base: LocalSlot(5),
        offset: FieldOffset(0),
        ty: Pointer
    },
    // 加载索引值
    LoadInst {
        dest: VReg(201),
        src: LocalSlot(3),
        ty: Int32
    },
    // 计算数组元素地址
    IndexInst {
        dest: VReg(202),
        base: VReg(200),
        index: VReg(201),
        element_size: 4,
        ty: Pointer
    },
    // 加载最终值
    LoadIndirectInst {
        dest: VReg(203),
        addr: VReg(202),
        ty: Int32
    }
]
```

**寄存器虚拟化处理**：
- **无限寄存器**: 使用无限数量的虚拟寄存器
- **类型标注**: 每个虚拟寄存器都有明确类型
- **生命周期**: 记录虚拟寄存器的定义和使用
- **干扰分析**: 为后续寄存器分配准备干扰信息

#### **3. 控制流线性化阶段**

将 MIR 的基本块转换为 LIR 的线性指令流：

```rust
// MIR 中的条件跳转
Terminator {
    kind: SwitchInt {
        discr: Operand::Move(Place { local: 1 }),
        targets: SwitchTargets {
            values: [0],
            targets: [bb2, bb1]
        }
    }
}

// 转换为 LIR 线性指令
LIRInstructions [
    // 加载判别值
    LoadInst {
        dest: VReg(300),
        src: LocalSlot(1),
        ty: Bool
    },
    // 条件分支
    BranchInst {
        condition: VReg(300),
        true_target: Label("bb1"),
        false_target: Label("bb2")
    },
    
    // bb1 的指令
    LabelInst { label: Label("bb1") },
    // ... bb1 的其他指令 ...
    JumpInst { target: Label("bb3") },
    
    // bb2 的指令
    LabelInst { label: Label("bb2") },
    // ... bb2 的其他指令 ...
    JumpInst { target: Label("bb3") },
    
    // bb3 的指令
    LabelInst { label: Label("bb3") },
    // ... 后续指令 ...
]
```

**控制流线性化处理**：
- **标签生成**: 为基本块生成唯一标签
- **跳转指令**: 插入适当的跳转和分支指令
- **顺序排列**: 将基本块按合理顺序排列
- **优化布局**: 优化代码布局减少跳转开销

#### **4. 平台抽象阶段**

为不同后端提供统一的抽象接口：

```rust
// 函数调用的平台抽象
CallInst {
    func: FunctionRef("print"),
    args: [VReg(400), VReg(401)],
    dest: Some(VReg(402)),
    calling_convention: Platform,
    cleanup: None
}

// WebAssembly 后端转换
WasmInstructions [
    local.get 400,
    local.get 401,
    call $print,
    local.set 402
]

// JavaScript 后端转换
JavaScriptCode {
    "let temp402 = print(temp400, temp401);"
}

// 原生代码后端转换（x86-64）
X86Instructions [
    mov rdi, [rbp-16],  // 第一个参数
    mov rsi, [rbp-20],  // 第二个参数
    call print,
    mov [rbp-24], rax   // 返回值
]
```

**平台抽象处理**：
- **调用约定**: 抽象不同平台的调用约定
- **数据类型**: 统一不同平台的数据类型表示
- **内存模型**: 抽象不同平台的内存访问模式
- **异常处理**: 统一异常处理机制

## LIR 核心数据结构

### **程序结构节点**

```rust
pub struct LIRModule {
    pub functions: Vec<LIRFunction>,
    pub globals: Vec<GlobalVariable>,
    pub types: TypeTable,
    pub metadata: ModuleMetadata,
}

pub struct LIRFunction {
    pub name: Symbol,
    pub signature: FunctionSignature,
    pub instructions: Vec<LIRInstruction>,
    pub virtual_registers: VRegAllocator,
    pub stack_slots: StackAllocator,
    pub labels: LabelTable,
}

pub struct FunctionSignature {
    pub params: Vec<LIRType>,
    pub returns: Vec<LIRType>,
    pub calling_convention: CallingConvention,
    pub is_variadic: bool,
}
```

### **指令类型**

```rust
pub enum LIRInstruction {
    // 数据移动指令
    Move { dest: VReg, src: Operand },
    Load { dest: VReg, src: MemoryOperand },
    Store { dest: MemoryOperand, src: VReg },
    LoadImmediate { dest: VReg, value: Constant },
    
    // 算术指令
    Arithmetic { op: ArithOp, dest: VReg, lhs: VReg, rhs: Operand },
    Compare { op: CompareOp, dest: VReg, lhs: VReg, rhs: Operand },
    
    // 控制流指令
    Jump { target: Label },
    Branch { condition: VReg, true_target: Label, false_target: Label },
    Call { func: FunctionRef, args: Vec<VReg>, dest: Option<VReg> },
    Return { value: Option<VReg> },
    
    // 内存指令
    Alloca { dest: VReg, size: u32, align: u32 },
    Address { dest: VReg, base: MemoryOperand, offset: i32 },
    
    // 类型转换指令
    Cast { dest: VReg, src: VReg, from_ty: LIRType, to_ty: LIRType },
    Bitcast { dest: VReg, src: VReg, ty: LIRType },
    
    // 特殊指令
    Label { label: Label },
    Nop,
    Unreachable,
}
```

### **操作数和寄存器**

```rust
pub enum Operand {
    Register(VReg),
    Immediate(Constant),
    Memory(MemoryOperand),
    Label(Label),
}

pub struct VReg {
    pub id: u32,
    pub ty: LIRType,
    pub class: RegisterClass,
}

pub enum RegisterClass {
    Integer,
    Float,
    Vector,
    Pointer,
    Condition,
}

pub enum MemoryOperand {
    Stack { slot: StackSlot, offset: i32 },
    Global { symbol: Symbol, offset: i32 },
    Indirect { base: VReg, offset: i32, scale: u8 },
    Indexed { base: VReg, index: VReg, scale: u8, offset: i32 },
}
```

### **类型系统**

```rust
pub enum LIRType {
    // 基本类型
    Int8, Int16, Int32, Int64,
    UInt8, UInt16, UInt32, UInt64,
    Float32, Float64,
    Bool,
    
    // 指针类型
    Pointer(Box<LIRType>),
    
    // 聚合类型
    Struct { fields: Vec<LIRType>, packed: bool },
    Array { element: Box<LIRType>, length: u64 },
    
    // 函数类型
    Function { params: Vec<LIRType>, returns: Vec<LIRType> },
    
    // 平台特定类型
    Platform(PlatformType),
}

pub struct TypeInfo {
    pub size: u32,
    pub align: u32,
    pub is_pod: bool,
}
```

## 语言特性在 LIR 中的表示

### **闭包和高阶函数**

```rust
// 闭包的 LIR 表示
LIRInstructions [
    // 分配闭包对象
    Alloca {
        dest: VReg(500),
        size: 16, // 函数指针 + 环境指针
        align: 8
    },
    
    // 设置函数指针
    LoadImmediate {
        dest: VReg(501),
        value: FunctionPointer("closure_impl")
    },
    Store {
        dest: Memory::Indirect { base: VReg(500), offset: 0 },
        src: VReg(501)
    },
    
    // 设置环境指针
    Address {
        dest: VReg(502),
        base: Memory::Stack { slot: StackSlot(10) },
        offset: 0
    },
    Store {
        dest: Memory::Indirect { base: VReg(500), offset: 8 },
        src: VReg(502)
    }
]
```

### **代数效应处理**

```rust
// 效应执行的 LIR 表示
LIRInstructions [
    // 保存当前栈帧
    Call {
        func: FunctionRef("save_stack_frame"),
        args: [],
        dest: Some(VReg(600))
    },
    
    // 查找效应处理器
    Call {
        func: FunctionRef("find_effect_handler"),
        args: [VReg(601)], // 效应类型ID
        dest: Some(VReg(602))
    },
    
    // 条件跳转到处理器
    Compare {
        op: NotEqual,
        dest: VReg(603),
        lhs: VReg(602),
        rhs: Operand::Immediate(Constant::Null)
    },
    Branch {
        condition: VReg(603),
        true_target: Label("call_handler"),
        false_target: Label("unhandled_effect")
    },
    
    // 调用效应处理器
    Label { label: Label("call_handler") },
    Call {
        func: FunctionRef("invoke_effect_handler"),
        args: [VReg(602), VReg(604)], // 处理器 + 参数
        dest: Some(VReg(605))
    }
]
```

### **模式匹配编译**

```rust
// 复杂模式匹配的决策树
LIRInstructions [
    // 加载判别值
    Load {
        dest: VReg(700),
        src: Memory::Indirect { base: VReg(701), offset: 0 } // 枚举标签
    },
    
    // 多路分支
    Compare {
        op: Equal,
        dest: VReg(702),
        lhs: VReg(700),
        rhs: Operand::Immediate(Constant::Int32(0))
    },
    Branch {
        condition: VReg(702),
        true_target: Label("variant_0"),
        false_target: Label("check_variant_1")
    },
    
    Label { label: Label("check_variant_1") },
    Compare {
        op: Equal,
        dest: VReg(703),
        lhs: VReg(700),
        rhs: Operand::Immediate(Constant::Int32(1))
    },
    Branch {
        condition: VReg(703),
        true_target: Label("variant_1"),
        false_target: Label("default_case")
    }
]
```

## LIR 的优化机会

### **指令级优化**

```rust
// 窥孔优化示例
// 优化前
LIRInstructions [
    LoadImmediate { dest: VReg(800), value: Constant::Int32(0) },
    Arithmetic { op: Add, dest: VReg(801), lhs: VReg(802), rhs: VReg(800) }
]

// 优化后
LIRInstructions [
    Move { dest: VReg(801), src: Operand::Register(VReg(802)) }
]
```

### **寄存器分配优化**

```rust
// 寄存器合并优化
// 优化前
LIRInstructions [
    Arithmetic { op: Add, dest: VReg(900), lhs: VReg(901), rhs: VReg(902) },
    Move { dest: VReg(903), src: Operand::Register(VReg(900)) }
]

// 优化后（直接使用目标寄存器）
LIRInstructions [
    Arithmetic { op: Add, dest: VReg(903), lhs: VReg(901), rhs: VReg(902) }
]
```

### **控制流优化**

```rust
// 分支消除优化
// 优化前
LIRInstructions [
    LoadImmediate { dest: VReg(1000), value: Constant::Bool(true) },
    Branch {
        condition: VReg(1000),
        true_target: Label("always_taken"),
        false_target: Label("never_taken")
    }
]

// 优化后
LIRInstructions [
    Jump { target: Label("always_taken") }
]
```

## 多后端代码生成

### **WebAssembly 后端**

```rust
// LIR 到 WebAssembly 的转换
LIRInstruction::Arithmetic {
    op: Add,
    dest: VReg(100),
    lhs: VReg(101),
    rhs: VReg(102)
}

// 转换为 WebAssembly
WasmInstructions [
    "local.get 101",
    "local.get 102",
    "i32.add",
    "local.set 100"
]
```

### **JavaScript 后端**

```rust
// LIR 到 JavaScript 的转换
LIRInstruction::Call {
    func: FunctionRef("fibonacci"),
    args: [VReg(200)],
    dest: Some(VReg(201))
}

// 转换为 JavaScript
JavaScriptCode {
    "let v201 = fibonacci(v200);"
}
```

### **原生代码后端**

```rust
// LIR 到 x86-64 的转换
LIRInstruction::Load {
    dest: VReg(300),
    src: Memory::Stack { slot: StackSlot(5), offset: 0 }
}

// 转换为 x86-64 汇编
X86Assembly {
    "mov rax, [rbp-40]"  // 假设 StackSlot(5) 映射到 rbp-40
}
```

## 错误处理和诊断

### **LIR 验证**

```rust
pub enum LIRError {
    // 指令错误
    InvalidInstruction { inst: LIRInstruction, reason: String },
    UndefinedRegister { reg: VReg, location: Location },
    
    // 类型错误
    TypeMismatch { expected: LIRType, found: LIRType, location: Location },
    InvalidCast { from: LIRType, to: LIRType, location: Location },
    
    // 控制流错误
    UndefinedLabel { label: Label, location: Location },
    UnreachableCode { location: Location },
    
    // 调用约定错误
    InvalidCallSignature { expected: FunctionSignature, found: FunctionSignature },
    MissingReturn { function: Symbol },
}
```

### **调试信息**

```rust
pub struct DebugInfo {
    pub source_map: SourceMap,
    pub variable_info: VariableDebugInfo,
    pub type_info: TypeDebugInfo,
    pub inlining_info: InliningInfo,
}

pub struct SourceMap {
    pub instruction_to_source: HashMap<InstructionId, SourceLocation>,
    pub register_to_variable: HashMap<VReg, VariableName>,
}
```

## 性能优化

### **编译时优化**
- **并行处理**: 函数级别的并行 LIR 生成和优化
- **增量编译**: 只重新生成修改函数的 LIR
- **缓存机制**: 缓存指令选择和优化结果
- **内存效率**: 使用紧凑的数据结构和内存池

### **代码质量优化**
- **指令调度**: 重排指令以提高流水线效率
- **寄存器压力**: 减少寄存器使用和溢出
- **分支预测**: 优化分支布局和预测友好性
- **缓存局部性**: 改善指令和数据的缓存局部性

### **目标代码优化**
- **平台特化**: 利用目标平台的特殊指令
- **调用约定**: 优化函数调用的参数传递
- **内存访问**: 减少不必要的内存读写
- **常量优化**: 编译时常量计算和传播

## 总结

LIR 作为 Nyar 平台的代码生成准备层，通过系统化的转换过程将 MIR 转换为接近目标平台的线性指令表示。主要特点包括：

1. **指令选择**: 将高级操作转换为低级指令序列
2. **寄存器虚拟化**: 使用无限虚拟寄存器简化代码生成
3. **平台抽象**: 为多种后端提供统一的抽象接口
4. **优化友好**: 支持多种低级优化技术
5. **多后端支持**: 统一支持 WebAssembly、JavaScript 和原生代码生成

通过 LIR，Nyar 平台能够高效地生成高质量的目标代码，为不同的运行环境提供优化的执行性能，确保虚拟机平台的通用性和高效性。