# JVM 后端维护指南

JVM 后端负责将 Valkyrie 编译为 Java 类文件格式。

## 编译流水线

`Source -> AST -> HIR -> CFG -> JVM Bytecode`

## 设计考量

### 栈机架构
JVM 是一个基于栈的虚拟机。使用基于寄存器的 LIR 会引入冗余的 `iload`/`istore` 指令。因此，JVM 后端跳过了 SSA 和 LIR 阶段，直接从 **CFG** (控制流图) 生成代码。

### 线性化
CFG 的基本块被线性化为扁平的指令流。跳转指令（`Goto`, `SwitchInt`）通过在第二遍扫描（标签修复）期间计算相对偏移量来处理。

### 类型系统
Valkyrie 类型映射到 JVM 描述符：
- `i32` -> `I`
- `i64` -> `J`
- `f32` -> `F`
- `f64` -> `D`
- `string` -> `Ljava/lang/String;`
- `bool` -> `Z`
- `unit` -> `V`
- `class/unity` -> `Lpath/to/Class;`
- `Pointer` -> `J` (目前暂定映射为 64 位整数)

### 表达式生成
- **算术运算**：直接映射到 `iadd`, `ladd`, `fadd`, `dadd` 以及 `irem`, `lrem`, `frem`, `drem` 等指令。
- **比较运算**：
    - 对于 `i32`：使用 `if_icmp<cond>` 跳转并推送 0 或 1。
    - 对于 `i64/f32/f64`：使用 `lcmp/fcmpl/dcmpl` 指令，随后配合 `if<cond>` 指令生成布尔值。
    - 对于对象类型（class, unity, string）：使用 `if_acmp<cond>` 实现引用相等性比较。
- **位运算与逻辑运算**：
    - `And`, `Or`, `Xor` 映射到 `iand/land`, `ior/lor`, `ixor/lxor`。
    - `Shl`, `Shr` 映射到 `ishl/lshl`, `ishr/lshr`。
- **一元运算**：
    - `Neg` 映射到 `ineg/lneg/fneg/dneg`。
    - `Not`：对于 `bool` 和 `i32` 使用 `iconst_1/iconst_m1` 与 `ixor` 实现；对于 `i64` 使用常量池中的 `-1L` 与 `lxor` 实现。

### 方法与调用
- 每个 Valkyrie 函数都被发射为类中的一个 `static` 方法。
- **名称重整 (Mangling)**：
    - 全局函数保持原名。
    - 类/Unity 方法：`ClassName$MethodName`。
    - Trait 方法：`TraitName$MethodName`。
    - Impl 方法：`[TraitName$]TargetName_MethodName`。
- 方法调用（`Call`）目前使用 `invokestatic`。后端会自动从 `UIR` 或 `CfgProgram` 中检索被调用者的签名以生成正确的描述符。
- 对于动态调用或函数指针，后端利用 `java/lang/invoke/MethodHandle` 的 `invoke` 方法实现。

### 泛型支持
- **类型擦除**：所有泛型类型在描述符中被擦除为 `Ljava/lang/Object;`。
- **Signature 属性**：为了保留泛型信息，后端会为泛型函数和字段生成 JVM `Signature` 属性。
- **函数类型**：Valkyrie 的函数类型映射为 `Ljava/lang/invoke/MethodHandle;`。

### 代数效应 (Algebraic Effects)
- Valkyrie 的 `raise` 被视为一种 Effect，受 AE 机制管控。
- **Raise 实现**：在非 resume 情况下等价于 Java 异常，映射为 `athrow` 指令。
- **处理器 (Handlers)**：通过 JVM `Code` 属性中的 `exception_table` 实现 `PushHandler`/`PopHandler`。
- 进入 Handler 块时，后端会自动将 `current_stack` 初始化为 1 以匹配 JVM 规范（Effect 对象会被推入栈顶）。
- **恢复 (Resume)**：目前仅支持非恢复路径，完整 Continuation 支持计划在未来通过栈帧捕获或字节码重写实现。

### 控制流优化
- **跳转指令**：
    - 默认使用 `goto_w` (4 字节偏移量) 以支持超大基本块的跳转。
    - `SwitchInt` 会根据跳转范围和稀疏程度自动选择 `tableswitch` 或 `lookupswitch`，且均使用 4 字节偏移量。

## 当前进度

- [x] 基本 Class 文件结构生成
- [x] 常量池管理 (UTF8, Integer, Float, Long, Double, Class, String, FieldRef, MethodRef, NameAndType, MethodHandle, MethodType)
- [x] 基本类型映射 (bool, i32, i64, f32, f64, string, unit)
- [x] 算术运算 (Add, Sub, Mul, Div, Rem) 支持 i32, i64, f32, f64
- [x] 位运算与逻辑运算 (And, Or, Xor, Shl, Shr)
- [x] 一元运算 (Neg, Not)
- [x] 比较运算 (Eq, Ne, Lt, Le, Gt, Ge) 支持 i32, i64, f32, f64
- [x] 类与 Unity 的基础实例化与字段访问 (限 1 层)
- [x] 数组基础支持 (newarray, anewarray, iastore/iaload 等)
- [x] 静态方法调用 (invokestatic)
- [x] 代数效应基础 (Raise, Handlers)
- [x] 泛型 Signature 基础支持

## 路线图 (Roadmap)

### 短期目标 (Short-term)
1. **代码重构**：重构 `emitter.rs` 以减少字节码发射逻辑的冗余。
2. **调试支持**：实现 `LineNumberTable` 属性，支持源码行号映射。
3. **字段访问增强**：支持多层深度的字段/索引访问。
4. **方法调用完善**：支持 `invokevirtual` 和 `invokeinterface`。

### 中期目标 (Mid-term)
1. **闭包支持**：利用 `invokedynamic` 实现 Lambda 和闭包。
2. **性能优化**：实现更智能的栈平衡优化，减少不必要的 `dup` 和 `pop`。
3. **反射与内省**：支持 Valkyrie 运行时反射。

### 长期目标 (Long-term)
1. **增量编译**：支持基于类文件的增量编译。
2. **原生互操作**：优化与 Java 标准库的交互性能。
