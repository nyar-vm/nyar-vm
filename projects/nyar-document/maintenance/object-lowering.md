# Nyar 编译器：权威性降低指南 (The Definitive Lowering Guide)

**文档版本**: 3.0
**目标读者**: Nyar 核心编译器工程师、高级语言设计者、对现代编译器内部机制有深度兴趣的研究者

## 导论：渐进式降低的哲学

Nyar 编译器的核心架构建立在一个简单而强大的哲学之上：**渐进式降低 (Progressive Lowering)**。编译过程并非一次性的“大爆炸”式转换，而是一场精心编排的、从高度抽象到极端具体的旅程。这段旅程被划分为多个阶段，每个阶段由一个专门的**中间表示 (Intermediate Representation, IR)** 来定义。

本指南的核心原则是：**每一层 IR 都与其下一层解耦**。一个 IR 层的输出，经过其专属的“降低”过程，会生成一组更基础、更具体的数据结构。下一层 IR 的任务，就是接收这些结构，并用它们来构建自己独有的、更接近机器的表示，而无需（也不应该）关心上一层 IR 的高级语义。

这个过程就像一个多级精炼厂：
1.  **`nyar-ast` (抽象语法树)**: 接收源代码的原油，进行初步的语法结构化。
2.  **`nyar-hir` (高层 IR)**: 对 AST 进行语义精炼，处理类型、作用域和高级抽象，产出带有完整语义的“中间产品”。
3.  **`nyar-mir` (中层 IR)**: 将 HIR 的语义结构转化为纯粹的计算和控制流图 (CFG)，这是性能优化的核心反应炉。
4.  **`nyar-lir` (低层 IR)**: 将优化后的 CFG 映射到接近硬件的线性指令，处理寄存器和内存布局。
5.  **代码生成 (CodeGen)**: 将 LIR 指令最终转化为特定目标（Wasm, x86）的字节码或汇编。

```mermaid
graph TD
    subgraph Frontend
        A[源代码] -->|解析| B(<b>nyar-ast</b><br><i>语法结构</i>);
    end

    subgraph Mid-end (The Lowering Pipeline)
        B -->|<b>HIR Lowering</b><br><i>语义分析, 抽象消除</i>| C(<b>nyar-hir</b><br><i>类型, 作用域, Traits</i>);
        C -->|<b>MIR Lowering</b><br><i>CFG 生成, 优化</i>| D(<b>nyar-mir</b><br><i>基本块, 控制流</i>);
        D -->|<b>LIR Lowering</b><br><i>栈化, 指令选择</i>| E(<b>nyar-lir</b><br><i>虚拟指令, 寄存器</i>);
    end

    subgraph Backend
        E -->|<b>CodeGen</b><br><i>指令编码</i>| F[目标代码];
    end
```

本指南将逐一剖析 Nyar 支持的最强大的语言特性，详细展示它们在这条精炼管道中是如何被逐步分解、转换和具体化的。

---

## 第 1 章：模式匹配 —— 从声明式解构到最优跳转表

模式匹配是现代静态语言的标志性特性，它允许开发者以优雅、安全的方式解构复杂数据。编译器的挑战在于，如何将这种高度声明式的语法，转化为最高效的条件分支代码。

### 1.1 抽象之源：一个复杂的 `match`

我们将追踪一个包含多种模式、绑定、守卫和嵌套的例子。

```nyar
enum NetworkEvent {
    Packet { id: u32, payload: Vec<u8> },
    Connect(String),
    Disconnect,
}

struct AppState { last_id: u32 }

fn handle_event(event: NetworkEvent, state: &mut AppState) {
    match event {
        NetworkEvent::Packet { id, payload } if payload.len() > 0 => {
            println!("Received data packet {} with {} bytes.", id, payload.len());
            state.last_id = id;
        },
        NetworkEvent::Packet { id, .. } => {
            println!("Received empty packet {}.", id);
        },
        NetworkEvent::Connect(addr) => {
            println!("Connection from {}.", addr);
        },
        // Disconnect is intentionally omitted to show exhaustiveness check
    }
}
```

**编译器面临的挑战**:
1.  如何验证所有 `NetworkEvent` 的变体都被覆盖了？（穷尽性）
2.  如何处理变量绑定（`id`, `payload`, `addr`）？
3.  如何高效地处理分支守卫（`if payload.len() > 0`）？
4.  如何生成比一长串 `if-else` 更高效的分支代码？

### 1.2 `nyar-ast`: 语法的忠实镜像

在 AST 层面，`match` 表达式被直接表示为一个节点，忠实地反映了源代码的结构。

*   **核心节点**: `ast::Expr::Match { scrutinee: Box<Expr>, arms: Vec<MatchArm> }`
*   **`ast::MatchArm`**: 包含 `pattern: Pattern`, `guard: Option<Box<Expr>>`, 和 `body: Box<Expr>`。
*   **`ast::Pattern`**: 这是一个 `enum`，可以表示 `Wildcard` (`_`), `Literal`, `Identifier` (`id`), `Struct` (`NetworkEvent::Packet { .. }`) 等。
*   **知识水平**: AST 对语义一无所知。它不知道 `NetworkEvent` 是一个 `enum`，不知道 `Packet` 是它的一个变体，更不知道 `match` 是否穷尽。它只是一个语法合法的结构。

### 1.3 `nyar-hir`: 语义的革命与决策树的诞生

HIR 阶段是模式匹配的第一次，也是最重要的一次降低。它从纯粹的语法结构中提炼出完整的语义，并将其转化为一种更基础的逻辑形式。

**HIR Lowering 过程**:

1.  **语义分析**:
    *   **类型检查**: 编译器首先确认 `scrutinee`（被匹配的表达式 `event`）的类型是 `NetworkEvent`。
    *   **模式验证**: 对每个 `MatchArm` 中的 `Pattern` 进行类型检查，确保它们是 `NetworkEvent` 的有效模式。
    *   **穷尽性检查 (Exhaustiveness Checking)**: 这是最关键的静态安全保证。编译器获取 `NetworkEvent` 的完整 `enum` 定义，并将其视为一个**类型空间**。它会逐个处理 `match` 的分支，从这个空间中“减去”已被覆盖的部分。在处理完所有分支后，如果类型空间仍不为空（本例中 `Disconnect` 未被处理），编译器就会报错，强制开发者处理所有情况。
    *   **可达性检查**: 检查是否有分支被之前的分支完全覆盖，例如 `Packet { id, .. }` 在 `Packet { id, payload }` 之前。

2.  **核心降低：`match` 脱糖为决策树 (Decision Tree Desugaring)**
    `match` 的高级语法被消除，取而代之的是一个由更基础的 HIR 节点（主要是 `If` 和 `Let`）构成的逻辑决策树。这个过程由一个专门的算法（通常基于 "Compiling Pattern Matching to Good Decision Trees"）完成。

    *   **输入**: 一个经过验证的 `match` 表达式。
    *   **输出**: 一个 HIR 表达式，其结构等价于嵌套的 `if-let` 和 `if`。
    *   **过程**:
        1.  **选择分发列**: 算法首先查看所有模式，决定最佳的“测试”方式。对于 `enum`，最佳测试是其**判别式 (discriminant)**。
        2.  **生成根节点**: 创建一个测试 `event` 判别式的根节点。
        3.  **构建分支**:
            *   对于 `Packet` 变体，它会生成一个分支，该分支内部首先进行绑定（`let id = event.id`, `let payload = event.payload`），然后进一步测试（守卫）。
            *   对于 `Connect` 变体，生成另一个分支，绑定 `addr`。
        4.  **处理守卫**: `if payload.len() > 0` 守卫被转换为一个 `hir::Expr::If` 节点，嵌套在 `Packet` 的分支逻辑内部。
        5.  **处理通配符**: 像 `Packet { id, .. }` 这样的模式，其优先级低于更具体的模式，会成为前一个模式守卫失败时的 `else` 分支。

    *伪代码展示脱糖后的 HIR 结构*:
    ```rust
    // Conceptual HIR structure after desugaring
    let __scrutinee = event;
    if __scrutinee is NetworkEvent::Packet {
        let id = __scrutinee.id;
        let payload = __scrutinee.payload;
        if payload.len() > 0 {
            // Arm 1 body
        } else {
            // Arm 2 body
        }
    } else if __scrutinee is NetworkEvent::Connect {
        let addr = __scrutinee.addr;
        // Arm 3 body
    }
    // (HIR lowering would have already caught the non-exhaustive error)
    ```

**HIR 的产出**:
*   一个不再包含 `hir::Expr::Match` 的 HIR 子树。
*   所有模式匹配的逻辑都已显式化为基础的条件判断、绑定和分支。
*   这个结构对于下一层 MIR 来说，是极易处理的。

### 1.4 `nyar-mir`: 控制流图的具体化

MIR 构建器接收脱糖后的 HIR，其任务是将其转化为一个高效的控制流图 (CFG)。

**MIR Lowering 过程**:

1.  **CFG 构建**:
    *   HIR 的决策树被直接映射为一个 CFG。每个逻辑块（`if` 的 `then` 块, `else` 块）都成为一个或多个**基本块 (Basic Block)**。
2.  **核心降低：`SwitchInt` 终结符**
    *   MIR 构建器识别出决策树的根是基于 `enum` 判别式的分发。它不会生成一连串的 `if-else` 比较。
    *   取而代之，它生成一个高度优化的**`mir::Terminator::SwitchInt`** 指令。
    *   **步骤**:
        1.  创建一个**入口块 (Entry Block)**，其指令是加载 `event` 的判别式（一个整数，例如 `Packet`=0, `Connect`=1, `Disconnect`=2）到一个临时变量中。
        2.  入口块的终结符是 `SwitchInt { scrutinee: tmp_discriminant, targets: ... }`。
        3.  `targets` 是一个从整数值到目标基本块的映射。例如：`{ 0 => BB_Packet, 1 => BB_Connect, otherwise => BB_Panic }`。（`otherwise` 分支用于处理未覆盖的情况，尽管 HIR 的穷尽性检查会避免这种情况发生）。
3.  **构建分支 CFG**:
    *   **`BB_Packet`**: 这个基本块的指令会从 `event` 中提取 `id` 和 `payload` 字段的值并存入局部变量。它的终结符是一个 `If`，测试 `payload.len() > 0` 的条件，并根据结果跳转到 `BB_Packet_Arm1` 或 `BB_Packet_Arm2`。
    *   **`BB_Connect`**: 这个块提取 `addr` 并跳转到 `BB_Connect_Arm3`。
    *   每个 `Arm` 块包含相应分支体的 MIR 指令。

**MIR 的产出**:
*   一个由基本块构成的 CFG。
*   `match` 的核心逻辑被一个单一、高效的 `SwitchInt` 终结符表示。
*   所有的数据解构和守卫检查都已成为 CFG 中明确的指令和条件分支。

### 1.5 `nyar-lir` & CodeGen: 硬件的最优选择

LIR 接收的是一个纯粹的 CFG，它的任务是将其映射到目标机器的指令，并进行最终的硬件相关优化。

**LIR Lowering & CodeGen 过程**:

1.  **指令选择**: MIR 中的 `SwitchInt` 指令会被 LIR 的指令选择逻辑识别。
2.  **核心降低：跳转表的生成 (Jump Table Generation)**
    *   对于密集、连续的整数分支（`enum` 判别式是完美案例），代码生成器会选择使用**跳转表**，而不是一串 `cmp` 和 `jmp` 指令。
    *   **步骤**:
        1.  在只读数据段（`.rodata`）中创建一个静态数组，即跳转表。表的每个条目都是一个代码地址。
            ```assembly
            .section .rodata
            .align 8
            EVENT_JUMP_TABLE:
                .quad .L_BB_Packet      ; Entry for discriminant 0
                .quad .L_BB_Connect     ; Entry for discriminant 1
                .quad .L_BB_Panic       ; Entry for discriminant 2 (or others)
            ```
        2.  在代码中，`SwitchInt` 被编译为几条高效的指令：
            ```assembly
            ; rdi holds the event discriminant
            ; Range check (optional but good practice)
            cmp  rdi, 2
            ja   .L_BB_Panic
            
            ; The core jump table lookup
            lea  rax, [rip + EVENT_JUMP_TABLE] ; Get address of the jump table
            mov  rax, [rax + rdi * 8]          ; Load the target address from the table
            jmp  rax                           ; Indirect jump to the target
            ```
*   **性能**: 这个实现的分支时间复杂度是 **O(1)**，与分支数量无关。这远胜于 `if-else` 链的 O(n) 复杂度。

### 1.6 总结之旅

| IR Stage          | 表示形式                                                | 核心降低/转换                                                |
| ----------------- | ------------------------------------------------------- | ------------------------------------------------------------ |
| **`nyar-ast`**    | `ast::Expr::Match` 节点                                 | 语法解析                                                     |
| **`nyar-hir`**    | (脱糖为) 嵌套的 `If`/`Let` 决策树                       | **语义分析 (穷尽性)**, **脱糖为决策树**                      |
| **`nyar-mir`**    | 由 **`SwitchInt`** 终结符连接的 CFG                     | 将决策树根优化为 `SwitchInt`，构建显式控制流图               |
| **`nyar-lir`**    | `LIR_Switch` 虚拟指令                                   | 指令选择，准备生成跳转表                                     |
| **CodeGen**       | **跳转表** + 间接跳转 (`jmp rax`)                       | 将 `LIR_Switch` 实例化为最高效的硬件分支机制                 |

---

## 第 2 章：闭包 —— 从捕获环境到具体结构

闭包是函数式编程的灵魂，它允许函数捕获并“记住”其定义时的环境。编译器的任务是解开这个魔法，将一个看似单一的函数对象，分解为代码和数据的具体组合。

### 2.1 抽象之源：一个返回闭包的高阶函数

```nyar
fn create_logger(prefix: String) -> impl Fn(String) {
    let mut counter = 0;

    // The closure captures `prefix` by value (move)
    // and `counter` by mutable reference.
    move |message: String| {
        counter += 1;
        println!("[{}] #{}: {}", prefix, counter, message);
    }
}

fn main() {
    let http_logger = create_logger("[HTTP]".to_string());
    let db_logger = create_logger("[DB]".to_string());

    http_logger("Request received".to_string()); // counter becomes 1
    http_logger("Response sent".to_string());   // counter becomes 2
    db_logger("Query executed".to_string());      // its own counter becomes 1
}
```
**编译器面临的挑战**:
1.  如何表示一个同时包含代码和捕获数据的“函数”？
2.  如何区分不同的捕获模式（值捕获 `prefix`，可变引用捕获 `counter`）？
3.  如何确保 `http_logger` 和 `db_logger` 各自有独立的环境（特别是独立的 `counter`）？
4.  如何将闭包调用 `http_logger(...)` 翻译成一个常规的函数调用？

### 2.2 `nyar-ast`: 语法的起点

*   **核心节点**: `ast::Expr::Lambda { params: Vec<Param>, body: Box<Expr> }`
*   **知识水平**: AST 知道这是一个匿名函数定义。但它完全不知道“捕获”是什么。`prefix` 和 `counter` 在它看来只是在函数体中引用的普通标识符。`move` 关键字也被记录下来，但其语义尚未被解析。

### 2.3 `nyar-hir`: 闭包转换的魔法

HIR 阶段执行了整个过程中最神奇的转换，称为**闭包转换 (Closure Conversion)**。

**HIR Lowering 过程**:

1.  **捕获分析 (Capture Analysis)**:
    *   编译器遍历 `Lambda` 的函数体。对于遇到的每个不在其参数列表中的变量（自由变量），它会向外层作用域查找。
    *   **`prefix`**: 找到 `prefix`。由于闭包被标记为 `move`，编译器决定按**值**捕获它。
    *   **`counter`**: 找到 `counter`。由于 `counter` 在闭包内部被修改 (`+= 1`)，编译器确定必须按**可变引用**捕获。
2.  **核心降低：环境结构体的合成 (Environment Struct Synthesis)**
    *   这是闭包转换的核心。编译器意识到闭包并非纯代码，它需要一个地方来存储捕获的环境。
    *   因此，它为这个闭包**在内部合成（synthesize）一个唯一的、匿名的结构体**。
        ```rust
        // Conceptual, compiler-generated struct for the closure
        struct __Closure_create_logger_line4 {
            // Field for by-value capture
            prefix: String,
            // Field for by-mutable-reference capture
            counter: &mut i32, 
        }
        ```
        *注意：实际实现中，为了处理生命周期，对 `counter` 的捕获可能更复杂，可能会捕获一个指向栈上 `counter` 的指针，或者如果闭包生命周期超过栈，编译器会强制将 `counter` 提升到堆上。为简化，我们假设是捕获引用。*

3.  **`Fn` Trait 的实现合成**:
    *   闭包的可调用性是通过 Trait 实现的。编译器会为这个新合成的结构体自动实现 `Fn`, `FnMut`, 或 `FnOnce` trait。
    *   由于闭包修改了 `counter`，它需要 `&mut self`，因此编译器会实现 `FnMut`。
        ```rust
        // Conceptual, compiler-generated impl
        impl FnMut(String) for __Closure_create_logger_line4 {
            // The `call` method is the actual code of the closure.
            // It takes the environment as its first parameter (`&mut self`).
            fn call_mut(&mut self, message: String) {
                // Accessing captured variables is now struct field access.
                *self.counter += 1;
                println!("[{}] #{}: {}", self.prefix, *self.counter, message);
            }
        }
        ```
4.  **HIR 重写 (Rewriting)**:
    *   原始的 `ast::Expr::Lambda` 节点在 HIR 中被彻底替换。
    *   `create_logger` 函数中创建闭包的地方，被重写为一个**结构体实例化**的表达式。
        ```rust
        // The original `move |...|` is rewritten in HIR to:
        {
            // The mutable variable `counter` is on the stack of `create_logger`
            let mut counter = 0; 
            // The closure is now just an instance of our synthetic struct
            __Closure_create_logger_line4 {
                prefix: prefix, // `prefix` is moved into the struct
                counter: &mut counter, // A mutable reference to the stack variable is taken
            }
        }
        ```
    *   `create_logger` 的返回类型 `impl Fn(String)`，其具体类型被解析为这个合成的 `__Closure_...` 结构体。

**HIR 的产出**:
*   **程序中不再有任何“闭包”的概念。**
*   取而代之的是：
    *   一些编译器生成的、普通的结构体定义。
    *   这些结构体的一些 `impl FnMut` 块。
    *   原始闭包创建点变成了普通的结构体实例化。

### 1.4 `nyar-mir`: 具体化的函数调用

MIR 构建器接收的是一个已经没有闭包的 HIR。它只看到结构体、变量和函数调用。

**MIR Lowering 过程**:

*   **`create_logger` 的 MIR**:
    *   会包含指令在栈上分配 `counter`。
    *   然后分配 `__Closure_...` 结构体，并用 `prefix` 和 `&mut counter` 的值来初始化它的字段。
    *   最后 `Return` 这个结构体。
*   **闭包体的 MIR**:
    *   `__Closure_...::call_mut` 被编译成一个**独立的、具名的 MIR 函数**。
    *   这个函数有一个隐式的 `&mut self` 参数，它是一个指向 `__Closure_...` 实例的指针。
    *   函数体内的 `*self.counter += 1` 被编译成一系列 `Load`, `Add`, `Store` 指令，通过 `self` 指针操作环境。
*   **调用点的 MIR (`http_logger(...)`)**:
    *   这个调用被 MIR 识别为一个 `Trait` 方法调用。由于在 `main` 函数中，`http_logger` 的具体类型 `__Closure_...` 是已知的，这会被解析为一个**静态派发**。
    *   最终，它被降低为一个对具名函数 `__Closure_...::call_mut` 的直接 `mir::Terminator::Call` 指令。
    *   传递给这个 `Call` 的参数是：`&mut http_logger` (作为 `&mut self`) 和 `"Request received"` (作为 `message`)。

### 1.5 `nyar-lir` & CodeGen: 栈与寄存器

**LIR Lowering & CodeGen 过程**:

1.  **内存布局**:
    *   `main` 函数的栈帧中会为 `http_logger` 和 `db_logger` 这两个 `__Closure_...` 结构体分配空间。
    *   在 `create_logger` 被调用时，其栈帧上的 `counter` 变量的地址会被计算出来，并存入闭包结构体的 `counter` 字段。
2.  **函数调用**:
    *   `http_logger("Request received")` 的调用，会被编译成标准的函数调用机器指令。
    *   根据目标平台的调用约定 (ABI)，`&mut http_logger` 的地址和 `"Request received"` 的指针会被放入指定的参数寄存器（例如 x86-64 上的 `rdi` 和 `rsi`）。
    *   然后执行一条 `call __Closure_create_logger_line4::call_mut` 指令。
3.  **环境访问**:
    *   在 `call_mut` 函数内部，访问 `self.prefix` 或 `*self.counter` 会被编译成最高效的内存访问指令，即从第一个参数寄存器（`rdi`）中的地址加上一个固定的字段偏移量来加载数据。

### 1.6 总结之旅

| IR Stage          | 表示形式                                                | 核心降低/转换                                                |
| ----------------- | ------------------------------------------------------- | ------------------------------------------------------------ |
| **`nyar-ast`**    | `ast::Expr::Lambda` 节点                                 | 语法解析                                                     |
| **`nyar-hir`**    | **(被重写为)** 编译器合成的**环境结构体** + `Fn` Trait 实现 | **闭包转换**: 分析捕获，合成环境结构体和 `call` 方法         |
| **`nyar-mir`**    | 结构体实例 + 对具体函数的 `Call` 指令                     | 将闭包调用解析为对合成的 `call` 方法的直接函数调用           |
| **`nyar-lir`**    | 栈上的结构体 + `LIR_Call` 虚拟指令                      | 确定栈帧布局，将调用映射到标准调用约定                       |
| **CodeGen**       | 栈内存 + `mov`/`lea`/`call` 等具体指令                   | 生成具体的内存访问和函数调用指令                             |

---

## 第 3 章：高阶类型 (HKT) —— 编译期元编程的巅峰

高阶类型 (HKT) 是泛型编程的终极工具，它允许我们对“容器”本身进行抽象，例如 `Vec<_>`, `Option<_>`, `Result<_, E>`。HKT 的整个故事都发生在编译期，它的目标是在不产生任何运行时开销的情况下，提供最强的静态类型保证。

### 3.1 抽象之源：一个泛型 `Functor`

```nyar
// F<~> 是 Nyar 中表示 HKT 的语法
// 它表示 "F 是一个接受一个类型参数的类型构造器"
trait Functor<F<~>> {
    fn map<A, B>(container: F<A>, f: fn(A) -> B) -> F<B>;
}

impl<T> Functor<Option<~>> for Option<T> {
    fn map<A, B>(container: Option<A>, f: fn(A) -> B) -> Option<B> {
        // ... implementation
    }
}

impl<T> Functor<Vec<~>> for Vec<T> {
    fn map<A, B>(container: Vec<A>, f: fn(A) -> B) -> Vec<B> {
        // ... implementation
    }
}

fn process_data(data: Option<i32>) {
    // 编译器需要将这个泛型调用，转换为对 Option::map 的具体调用
    let result = Functor::map(data, |x| x.to_string());
}
```

**编译器面临的挑战**:
1.  如何理解和类型检查 `F<~>` 这种“类型的类型”？
2.  如何确保 `impl Functor<Option<~>>` 是合法的，而 `impl Functor<i32>` 是非法的？
3.  如何在 `process_data` 中，将对 `Functor::map` 的泛型调用，精确地解析到 `Option` 的具体实现上？
4.  最重要的是，如何确保这一切在最终的机器码中没有任何痕迹或开销？

### 3.2 `nyar-ast` & `nyar-hir`: Kind 系统的静态验证

HKT 的降低过程几乎完全发生在 HIR 阶段，通过一个比“类型”更高一层的概念——**种类 (Kind)**。

**AST 表示**:
*   `F<~>` 被解析为一个特殊的 `ast::GenericParam` 节点，带有一个标记表明它是一个高阶类型参数。

**HIR Lowering 过程**:

1.  **引入 Kind 系统**:
    *   HIR 的类型检查器不仅仅处理类型，它还处理类型的“形状”或“种类”。
    *   **`Type` (或写作 `*`)**: 这是具体类型的种类。例如，`i32`, `String`, `Vec<u8>` 的种类都是 `Type`。
    *   **`Type -> Type` (或写作 `* -> *`)**: 这是接受一个具体类型并返回一个具体类型的类型构造器的种类。例如，`Option`, `Vec` 的种类都是 `Type -> Type`。
    *   **`Type -> Type -> Type`**: 例如 `Result` 的种类。

2.  **Kind 检查**:
    *   当 HIR 分析 `trait Functor<F<~>>` 时，它推断出泛型参数 `F` 的种类必须是 `Type -> Type`。
    *   当分析 `impl<T> Functor<Option<~>> for Option<T>` 时，它会检查 `Option` 的种类。因为 `Option` 接受一个类型参数（如 `T`）来构成一个完整的类型（`Option<T>`），所以它的种类是 `Type -> Type`。Kind 匹配成功，`impl` 合法。
    *   如果有人尝试写 `impl Functor<i32>`，编译器会检查 `i32` 的种类，发现是 `Type`，与期望的 `Type -> Type` 不匹配，立即报错。

3.  **HKT 的“降低”是语义上的**:
    *   在 HIR 阶段，HKT 并没有被“降低”为任何更简单的结构。相反，它的高级语义被**完全理解和静态验证**。
    *   编译器现在拥有了所有必要的信息，可以正确地解析 HKT 相关的 `impl` 和方法调用。

**HIR 的产出**:
*   一个经过 Kind 检查的、语义上完全合法的 HIR。
*   HKT 仍然作为泛型参数存在，等待下一阶段被彻底消除。

### 3.3 `nyar-interpreter` (Staging): 单态化的终点站

HKT 的抽象生命在这里终结。Staging 引擎（一个编译时解释器）通过**单态化 (Monomorphization)** 来消除所有泛型，包括 HKT。

**Staging Lowering 过程**:

1.  **可达性分析**: Staging 引擎从 `main` 或其他入口点开始，分析所有可达的函数。
2.  **遇到泛型调用**: 当它分析 `process_data` 时，遇到了对 `Functor::map` 的调用。
3.  **类型实例化解析**:
    *   通过类型推导，它知道 `data` 是 `Option<i32>`。
    *   因此，它确定 `Functor::map` 的泛型参数被实例化为：
        *   `F` = `Option<~>`
        *   `A` = `i32`
        *   `B` = `String` (来自闭包的返回类型)
4.  **`impl` 查找**: 它在全局 `impl` 表中查找 `impl Functor<Option<~>>`，并成功找到。
5.  **核心降低：按需代码生成 (On-Demand Code Generation)**
    *   这是消除 HKT 的决定性步骤。Staging 引擎获取 `Option::map` 的**泛型 HIR/MIR 模板**。
    *   它将模板中所有的泛型参数（`A`, `B`）替换为具体的类型（`i32`, `String`）。
    *   它生成一个全新的、非泛型的、**具体的 MIR 函数**。这个函数会被赋予一个经过“名称修饰 (mangling)”的唯一内部名称，例如 `__nyar_impl_Functor_Option_map_i32_String`。
6.  **调用点重写**:
    *   `process_data` 中的泛型调用 `Functor::map(...)` 被**直接替换**为一个对新生成的具体函数 `__nyar_impl_...` 的 `Call` 指令。

**Staging 的产出**:
*   一个最终的、完全静态化的 MIR 程序。
*   **在这个 MIR 中，没有任何 HKT、泛型 trait 或泛型函数的痕迹。** 所有东西都变成了具体的结构体和对具体函数的直接调用。

### 3.4 `nyar-mir`, `nyar-lir` & CodeGen: HKT 从未存在过

*   从 MIR 阶段开始，后续的所有编译器后端都对 HKT 一无所知。
*   它们看到的只是一个普通的函数调用，例如 `call __nyar_impl_Functor_Option_map_i32_String`。
*   这个调用会像任何其他非泛型函数一样，被优化、分配寄存器并生成最高效的机器码。

### 3.5 总结之旅

| IR Stage          | 表示形式                                                | 核心降低/转换                                                |
| ----------------- | ------------------------------------------------------- | ------------------------------------------------------------ |
| **`nyar-ast`**    | 特殊的泛型语法 `F<~>`                                   | 语法解析                                                     |
| **`nyar-hir`**    | 带有**种类 (Kind)** 信息的类型系统                      | **Kind 检查**: 对 HKT 的使用进行严格的静态验证               |
| **Staging/MIR Gen** | **(被彻底消除)**                                        | **单态化**: 为每个具体调用点生成非泛型的 MIR 函数实例        |
| **`nyar-mir`**    | (不存在 HKT 概念)                                       | (无)                                                         |
| **`nyar-lir`**    | (不存在 HKT 概念)                                       | (无)                                                         |
| **CodeGen**       | (不存在 HKT 概念)                                       | (无)                                                         |

HKT 的故事完美地诠释了“零成本抽象”：所有复杂的元编程和类型体操都在编译前期完成，为开发者提供了无与伦比的静态保证，而运行时付出的代价为零。

---

*由于篇幅限制，接下来的 Effect 系统和 Trait/多重继承将遵循同样的深度和结构进行阐述。这是一个持续的过程，确保每个特性都得到同样详尽的分析。*

## 第 4 章：Effect 系统 —— 从代数效应到状态机

Effect 系统（或代数效应）是比异常处理更强大的控制流抽象。它允许函数“暂停”自己，将一个“效应”抛给调用者处理，并等待调用者“恢复 (resume)”它的执行。这对于实现生成器、异步/等待、状态管理等非常强大。

### 4.1 抽象之源：一个可中断的迭代器

```nyar
// 定义一个 effect, 它包含一个 Yield 操作
effect Io {
    Yield(String) -> (), // Yield a String, expect nothing back
}

// 一个 handler, 决定如何处理 Io effect
fn run_with_logging(f: fn() -> ()) {
    handler {
        f() -> (), // When f finishes normally
        effect Io::Yield(value) => {
            println!("Yielded: {}", value);
            resume (()); // Resume the paused function
        }
    }
}

// 一个使用 effect 的函数 (一个生成器)
fn name_generator() {
    perform Io::Yield("Alice".to_string());
    perform Io::Yield("Bob".to_string());
    perform Io::Yield("Charlie".to_string());
}

fn main() {
    run_with_logging(name_generator);
    // Output:
    // Yielded: Alice
    // Yielded: Bob
    // Yielded: Charlie
}
```

**编译器面临的挑战**:
1.  如何实现一个可以“暂停”和“恢复”的函数？常规的函数调用栈做不到这一点。
2.  `perform` 如何将控制权神奇地转移到 `handler`？
3.  `resume` 如何又将控制权和值传回 `perform` 的下一条语句？
4.  被暂停的函数（`name_generator`）的局部变量和状态（比如它执行到了哪一行）保存在哪里？

### 4.2 `nyar-ast` & `nyar-hir`: 语义的验证

*   **AST**: `effect`, `handler`, `perform`, `resume` 都是独特的 AST 节点。
*   **HIR**: 进行关键的语义分析。
    *   **Effect 检查**: 编译器会推断 `name_generator` 的函数类型不仅仅是 `fn() -> ()`，而是 `fn() -> () raises Io`。它会验证 `perform Io::Yield` 的使用是合法的。
    *   **Handler 检查**: 编译器验证 `handler` 块是否为所有它声称处理的 effect（这里是 `Io`）提供了处理分支。
    *   **类型检查**: 验证 `Yield` 的参数类型、`resume` 的返回值类型是否匹配 `effect` 定义。

### 4.3 `nyar-hir` & `nyar-mir`: 状态机变换的革命

这是 Effect 系统降低的核心。任何可能执行 `perform` 的函数，都不能再被编译成一个普通的函数。它必须被**转换为一个状态机**。

**核心降低：函数到状态机的转换 (State Machine Transformation)**

1.  **生成器结构体 (Generator Struct)**:
    *   编译器为 `name_generator` 函数合成一个结构体。这个结构体将成为这个函数被暂停时的状态容器。
        ```rust
        // Conceptual, compiler-generated struct for the generator
        struct __Generator_name_generator {
            // State field to track the execution point
            state: u32,

            // Fields to store all local variables that must survive across a suspension point (`perform`)
            // (In this case, there are no locals that need to be saved)
        }
        ```
2.  **函数体分裂 (Function Body Splitting)**:
    *   `name_generator` 的函数体在每个 `perform` 点被“切开”。
    *   整个函数被重写成一个单一的 `resume` 方法，这个方法作用于新生成的状态机结构体。
    *   `resume` 方法的主体是一个巨大的 `match` (或 `SwitchInt` in MIR)，根据当前的状态跳转到正确的代码片段。

    *伪代码展示重写后的 `resume` 方法*:
    ```rust
    // The original name_generator is transformed into this:
    impl Generator for __Generator_name_generator {
        fn resume(&mut self) -> GeneratorState {
            match self.state {
                0 => {
                    // Original code before the first perform
                    self.state = 1; // Set state for next resume
                    return GeneratorState::Yielded(Io::Yield("Alice".to_string()));
                },
                1 => {
                    // Original code after the first perform and before the second
                    self.state = 2;
                    return GeneratorState::Yielded(Io::Yield("Bob".to_string()));
                },
                2 => {
                    // ... and so on
                    self.state = 3;
                    return GeneratorState::Yielded(Io::Yield("Charlie".to_string()));
                },
                3 => {
                    // End of function
                    return GeneratorState::Complete(());
                }
            }
        }
    }
    ```
3.  **`perform` 和 `resume` 的降低**:
    *   `perform Io::Yield(...)` 在 MIR 中被降低为：
        1.  更新状态机中的 `state` 字段。
        2.  `Return` 一个特殊的值，表示“我暂停了，并产生了`Io::Yield`效应”。
    *   `handler` 的 `resume` 调用在 MIR 中被降低为：
        1.  调用状态机的 `resume` 方法。

4.  **`handler` 的降低**:
    *   `run_with_logging` 函数被编译成一个**循环**，它驱动状态机的执行。
    *伪代码展示 `run_with_logging` 的 MIR 逻辑*:
    ```mir
    // Create an instance of the state machine
    let mut gen = __Generator_name_generator::new();

    loop {
        // Run the state machine one step
        let result = gen.resume();

        match result {
            GeneratorState::Yielded(Io::Yield(value)) => {
                // Execute the handler arm
                println!("Yielded: {}", value);
                // The `resume` call inside the handler just continues the loop
            },
            GeneratorState::Complete(()) => {
                // Execute the return arm of the handler
                break;
            }
        }
    }
    ```

**MIR 的产出**:
*   **不再有 `effect`, `perform`, `resume`, `handler` 的概念。**
*   取而代之的是：
    *   一些编译器生成的状态机结构体。
    *   这些结构体的 `resume` 方法，其核心是一个巨大的 `SwitchInt`。
    *   原始的 handler 变成了驱动 `resume` 方法并处理其返回值的普通循环代码。

### 4.4 `nyar-lir` & CodeGen: 栈上的状态

*   **内存布局**:
    *   状态机结构体 (`__Generator_...`) 通常在栈上分配。它的 `state` 字段和保存的局部变量都成为普通的结构体字段。
*   **控制流**:
    *   `resume` 方法中的 `SwitchInt` 被高效地编译成一个**跳转表**，就像模式匹配一样。每次调用 `resume` 都会根据 `state` 字段的值，O(1) 地跳转到正确的代码块。
*   **最终结果**: 一个极其高级的、可暂停/恢复的控制流抽象，被降低为：一个在栈上分配的结构体，以及一个通过跳转表高效切换执行片段的函数。所有神奇的控制权转移，都变成了简单的函数返回和循环。

### 4.5 总结之旅

| IR Stage          | 表示形式                                                | 核心降低/转换                                                |
| ----------------- | ------------------------------------------------------- | ------------------------------------------------------------ |
| **`nyar-ast`**    | `Effect`, `Handler`, `Perform` 等 AST 节点              | 语法解析                                                     |
| **`nyar-hir`**    | 带有 effect 签名的函数类型                              | Effect 类型检查，验证 handler 穷尽性                         |
| **MIR Gen**       | **(被重写为)** 状态机结构体 + `resume` 方法             | **状态机变换**：将可暂停函数分裂，并用 `SwitchInt` 重组      |
| **`nyar-mir`**    | `SwitchInt` 驱动的 CFG + 驱动循环                        | 优化状态机的 CFG，优化驱动循环                               |
| **`nyar-lir`**    | 栈上的结构体 + `LIR_Switch`                             | 确定状态机内存布局，准备生成跳转表                           |
| **CodeGen**       | 栈内存 + 跳转表 + 间接跳转                              | 将 `resume` 方法编译为最高效的状态切换代码                   |


---
## 第 5 章：Traits 与多重继承 —— 从接口到虚表的构建

Traits 是 Nyar 实现代码复用和抽象的核心机制。它们提供了接口、泛型约束，并能通过 trait object 实现动态多态。编译器需要为这同一个特性，提供两条截然不同的降低路径：一条是零成本的静态派发，另一条是灵活但有微小开销的动态派发。

### 5.1 抽象之源：静态与动态的共存

```nyar
trait Log { fn log(&self, msg: &str); }
trait Serialize { fn as_json(&self) -> String; }

// A struct implementing multiple traits (a form of multiple inheritance)
struct User { name: String }
impl Log for User { /* ... */ }
impl Serialize for User { /* ... */ }

struct Device { id: u32 }
impl Log for Device { /* ... */ }

// Path 1: Static Dispatch via Generics
// Zero-cost, resolved at compile time.
fn log_and_serialize<T: Log + Serialize>(item: &T) {
    item.log("Serializing item");
    let json = item.as_json();
    // ...
}

// Path 2: Dynamic Dispatch via Trait Objects
// Flexible, resolved at runtime.
fn broadcast_log(items: Vec<&dyn Log>) {
    for item in items {
        item.log("Broadcast message"); // This call is dynamic
    }
}

fn main() {
    let user = User { name: "Alice".to_string() };
    let device = Device { id: 123 };

    log_and_serialize(&user); // Monomorphized static call

    let loggables: Vec<&dyn Log> = vec![&user, &device];
    broadcast_log(loggables); // Dynamic calls via VTable
}
```

**编译器面临的挑战**:
1.  **静态派发**: 如何在编译时就确定 `log_and_serialize` 中的 `item.log()` 调用的是 `User::log`？
2.  **动态派发**: `&dyn Log` 到底是什么？如何将不同类型（`User`, `Device`）的对象存入同一个 `Vec`？
3.  **VTable**: `item.log()` 在 `broadcast_log` 中是如何在运行时找到正确的 `log` 方法（有时是 `User::log`，有时是 `Device::log`）的？
4.  **多重继承**: `T: Log + Serialize` 这个约束是如何被验证和利用的？

### 5.2 `nyar-ast` & `nyar-hir`: Trait 的解析与验证

*   **AST**: `trait`, `impl`, `T: A + B`, `&dyn Trait` 都是不同的语法节点。
*   **HIR**: 进行核心的 Trait 解析。
    *   **构建 Impl Graph**: 编译器构建一个全局的数据结构，映射 `(Type, Trait) -> Impl`。例如，它会记录 `(User, Log)` 对应一个 `impl`，`(User, Serialize)` 对应另一个。
    *   **方法解析**: 在 HIR 中，`item.log()` 被解析为一个对 `Log::log` 的符号调用。编译器此时已经知道这个调用是与 `Log` trait 关联的。
    *   **约束验证**: 对于 `log_and_serialize`，编译器会检查传入 `&user` 时，`User` 类型是否同时实现了 `Log` 和 `Serialize`。

### 5.3 静态派发 (`log_and_serialize`) 的降低之旅

**Staging/MIR Gen**:
*   **核心降低：单态化 (Monomorphization)**
    *   Staging 引擎分析 `log_and_serialize(&user)` 的调用。
    *   它确定泛型参数 `T` 在此处的具体类型是 `User`。
    *   它生成一个 `log_and_serialize` 的全新、非泛型副本：`__monomorphized_log_and_serialize_User`。
    *   在这个副本中，`item` 的类型是 `&User`。
*   **`nyar-mir`**:
    *   在 `__monomorphized_..._User` 的 MIR 中，`item.log(...)` 不再是一个 trait 方法调用。它被直接解析为一个对具体实现的 `Call` 指令：`Call User::log(item, "...")`。
    *   `item.as_json()` 同理，被解析为 `Call User::as_json(item)`。
*   **`nyar-lir` & CodeGen**:
    *   `Call User::log` 被编译成一条**直接函数调用** (`call User::log`)。
    *   **优化**: 因为调用的目标地址在编译时是已知的，编译器可以进行最激进的优化，尤其是**函数内联 (Inlining)**，这可以完全消除函数调用的开销。
*   **结果**: 静态派发路径上的 trait 抽象，在单态化后被完全消除，变成了最高效的直接函数调用，实现了真正的**零成本**。

### 5.4 动态派发 (`broadcast_log`) 的降低之旅

**MIR Lowering**:
*   **核心降低：胖指针与虚表解析 (Fat Pointers & VTable Resolution)**
    1.  **胖指针 (Fat Pointer) 的诞生**: 在 `main` 中，当 `&user` 被转换为 `&dyn Log` 时，MIR Lowering pass 执行了一个关键转换。`&dyn Log` 不再是一个普通的指针，它被降级为一个**胖指针**，一个包含两个成员的内部结构体：
        *   `data_ptr`: 指向 `user` 实例的实际内存地址。
        *   `metadata_ptr`: 一个指向 **虚方法表 (VTable)** 的指针。
    2.  **VTable 布局解析**: 这是 **MIR Lowering Pass** 的核心职责。
        *   它遍历所有 `impl`，为每个 `(Type, Trait)` 组合（如 `(User, Log)`, `(Device, Log)`）**设计**一个 VTable 布局。
        *   VTable `(User, Log)` 的布局可能如下：
            *   `offset 0`: 指向 `User::log` 的函数指针。
            *   `offset 8`: 指向 `User` 的析构函数指针。
            *   `offset 16`: `User` 的大小。
            *   `offset 24`: `User` 的对齐。
        *   MIR Lowering pass **不构建** VTable 的实际内容，它只**解析**其布局，并生成一个全局的**VTable 布局清单**。
    3.  **动态调用点降级**: `item.log(...)` 在低级 MIR 中被重写为：
        ```mir
        // Low-level MIR for a dynamic call
        _data_ptr = item.0; // Extract data pointer from fat pointer
        _vtable_ptr = item.1; // Extract vtable pointer from fat pointer
        _fn_ptr_addr = _vtable_ptr + const_offset_of_log; // Calculate address of the log method slot
        _fn_ptr = Load(_fn_ptr_addr); // Load the actual function pointer
        CallIndirect { target: _fn_ptr, args: (_data_ptr, "...") }; // Perform indirect call
        ```

**`nyar-lir` & CodeGen**:
*   **核心降低：VTable 的构建与实例化**
    *   CodeGen 接收来自 MIR 的**VTable 布局清单**。
    *   它在目标文件的只读数据段 (`.rodata`) 中，为每个 VTable **分配内存并填充内容**。它会查找 `User::log` 等函数的最终地址，并将这些地址写入 VTable 的槽位。
    *   `VTABLE_USER_LOG: .quad User::log, ...`
    *   `VTABLE_DEVICE_LOG: .quad Device::log, ...`
*   **指令生成**:
    *   低级 MIR 中的指令序列被直接翻译成机器码。
    *   `item.log(...)` 最终变成：
        ```assembly
        ; rdi holds the fat pointer's data_ptr, rsi holds the vtable_ptr
        mov  rax, [rsi + 0]  ; Load function pointer from VTable's first slot
        ; rdi is already set as the first argument ('self')
        ; Set up other arguments...
        call rax             ; Indirect call
        ```

### 5.5 总结之旅

| 派发方式   | 核心降低技术                                                 | 最终实现机制                         | 性能特征                             |
| ---------- | ------------------------------------------------------------ | ------------------------------------ | ------------------------------------ |
| **静态派发** | **单态化 (Monomorphization)** 在 Staging/MIR Gen 阶段        | **直接函数调用** (`call <label>`)    | **零成本**，可被内联                 |
| **动态派发** | **VTable 解析** (MIR Lowering) + **VTable 构建** (CodeGen)   | **间接函数调用** (`call <register>`) | **微小开销** (内存读取+间接跳转)     |