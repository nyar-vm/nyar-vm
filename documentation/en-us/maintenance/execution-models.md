# Valkyrie Execution Model

## 1. Overview

Valkyrie is designed to support multiple execution environments, ranging from high-performance production environments to highly interactive development environments.

## 2. Multi-Export Backend Architecture

Valkyrie employs a **Multi-Export Backend Architecture**. While the compiler frontend (AST -> HIR -> CFG -> SSA -> LIR) descends linearly, not all backends must start their transformation from LIR.

Each IR layer has its unique structural advantages, making it suitable for interfacing with different target platforms:

- **SSA (Static Single Assignment) -> LLVM IR**: 
    - LLVM itself is SSA-based. Converting directly from Valkyrie SSA preserves the richest data flow information, facilitating deep optimization by LLVM.
- **CFG (Control Flow Graph) -> WASM**: 
    - WASM is a structured control flow format. Converting directly from CFG makes it easier to recover the `block`, `loop`, `if`, and other structures required by WebAssembly, avoiding expensive control flow recovery after LIR flattening.
- **HIR (High-level IR) -> C / Source-to-Source**: 
    - HIR preserves high-level syntax information (e.g., Traits, type definitions). If exporting to C code or performing source-to-source translation is required, HIR is the best entry point.
- **LIR (Low-level IR) -> Interpretive Execution / AOT**:
    - LIR is designed for high-performance virtual machines, suitable for direct interpretive execution or simple AOT compilation into native machine code.

---

## 3. LIR Register Model Decisions

Valkyrie's LIR chooses a **Register Machine** model over a stack model, based on the following core considerations:

### 3.1 Why Choose a Register Model?

1.  **Reduced Dispatch Overhead**:
    - Stack models typically require numerous `push`/`pop` operations, which significantly increase the number of instructions for a virtual machine.
    - Register models allow a single instruction to complete multiple operations (e.g., `add r1, r2, r3`), reducing the frequency of instruction decoding and dispatching.
2.  **Algebraic Effects and State Snapshots**:
    - Valkyrie supports Algebraic Effects, which require frequent snapshotting and restoration of the execution state (Frame).
    - In a register model, the state is laid out flat in a `Vec<Value>`. Creating a snapshot requires only a single `clone()`, and restoring state involves only replacing a pointer.
    - In contrast, snapshots in stack models require handling complex stack frame offsets and deep copying, which is less efficient.

### 3.2 Why Not Infinite Registers?

While a virtual machine can simulate infinite registers (SSA style), Valkyrie LIR chooses **Dynamically Finite Registers**:

1.  **Memory Footprint and Snapshot Performance**:
    - Infinite registers would lead to exceptionally large register arrays in the `Frame`, filled with holes (sparse).
    - Every `Yield` instruction generates a snapshot. If the number of registers is excessive, the memory overhead and copying cost of snapshots become unacceptable.
2.  **Locality Optimization**:
    - Finite registers force the compiler to perform simple register allocation, making data more compact in memory and improving CPU cache hit rates.

### 3.3 Why Not a Fixed 256 Registers?

1.  **Avoiding Spilling**:
    - Fixed 256 registers (as in LuaVM or early virtual machines) can easily run out when handling very large functions or deeply nested expressions, forcing data to spill into stack memory and leading to a sharp drop in performance.
2.  **Allocation on Demand**:
    - Each function in Valkyrie LIR has its own `register_count`.
    - **Small Functions**: Occupy very few registers, making snapshots extremely fast.
    - **Complex Functions**: Automatically expand the number of registers, ensuring no spilling occurs.
    - This flexibility offers significant advantages when handling a large number of micro-closures and coroutines.

### 2.1 Interpreted Execution

During development and debugging phases, Valkyrie uses its built-in virtual machine (`ValkyrieVM`) to directly interpret and execute LIR.

- **Advantages**:
  - **Rapid Startup**: No lengthy machine code generation process.
  - **Hot Reloading**: Allows replacing functions without recompiling the entire program.
  - **Deep Debugging**: The interpreter can directly access all runtime states.

### 2.2 Static Compilation

During the production deployment phase, LIR can be further lowered into target machine code (e.g., WASM or native instructions).

- **Target Platforms**:
  - **WebAssembly (WASM)**: Targeted at modern browsers and server-side WASM runtimes.
  - **Native Binary**: Generates high-performance machine code via backend generators (planned).

## 3. Runtime Features

Whether through interpretive execution or compiled execution, Valkyrie provides unified runtime support:

- **Algebraic Effects**: Achieves efficient control flow jumps through stack-based handlers.
- **Automatic Memory Management**: Memory management based on garbage collection (depending on configuration).
- **Metaprogramming Support**: Runtime reflection and dynamic type checking.
