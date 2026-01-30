# ValkyrieVM Maintenance Guide

ValkyrieVM is the internal reference interpreter for Valkyrie.

## Compilation Pipeline

`Source -> AST -> HIR -> CFG -> SSA -> LIR -> VM Execution`

### 1. AST -> HIR
- **Responsible for**: Desugaring, scope resolution, and handling shadowing.
- **Implementation**: [ast_to_hir](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-compiler/src/transform/ast_to_hir/mod.rs).

### 2. HIR -> CFG
- **Responsible for**: Converting structured control flow (`if`, `while`, `loop`) into basic blocks and jumps.
- **Implementation**: [hir_to_cfg](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-compiler/src/transform/hir_to_cfg/mod.rs).

### 3. CFG -> SSA
- **Responsible for**: Constructing Static Single Assignment form and inserting Phi nodes.
- **Implementation**: [cfg_to_ssa](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-compiler/src/transform/cfg_to_ssa/mod.rs).

### 4. SSA -> LIR
- **Responsible for**: Phi elimination, register allocation, and instruction lowering.
- **Implementation**: [ssa_to_lir](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-compiler/src/transform/ssa_to_lir/mod.rs).
- **Current Status**:
    - [x] Basic Phi elimination (via Move insertion at the end of predecessor blocks).
    - [x] Basic instruction lowering.
    - [ ] Optimized Phi elimination (handling parallel move issues).
    - [x] Basic register allocation (simple reuse based on liveness analysis).
    - [ ] Optimized register allocation (requires linear scan or coloring algorithms).
    - [ ] Stack frame size calculation.

### 5. VM Execution
- **Responsible for**: Executing LIR instructions, managing the call stack, heap, and effect handlers.
- **Implementation**: [valkyrie-runtime](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-runtime/src/runtime/mod.rs).
- **Current Status**:
    - [x] Basic arithmetic and logic instructions.
    - [x] Jumps and branches (Jmp, JmpIf).
    - [x] Function calls and returns (supporting Native and FFI).
    - [x] Object and array operations (Alloc, Load, Store).
    - [x] Structured error handling (Effect, Continuation).
    - [x] Code coverage statistics.

### 6. FFI Mechanism
- **Design**: FFI markers are attached to `micro` function declarations via annotations (e.g., `↯import`).
- **Advantages**:
    - **Type Safety**: Using the function's `parameters` and `returns` definitions, the compiler can accurately generate marshaling code.
    - **Multi-backend Adaptation**: Interpreted and executed by different backends via the `target` parameter (e.g., `wasm`, `jvm`, `clr`, `dll`).
- **Example**:
    ```valkyrie
    ↯import(target: wasm, "wasi:random/insecure", "get-insecure-random-u64")
    micro get_random_u64() -> u64
    ```

### 7. Deep JIT Optimization
- **Trigger Level**: The JIT compiler works at the **SSA / HIR** level, not LIR.
- **Core Logic**:
    - **Semantic Retention**: Retains full type information, object boundaries, and explicit effect flows at the SSA level.
    - **Optimization Capabilities**:
        - **Cross-function Inlining**: Deep inlining based on hotspots at the SSA level.
        - **Type Specialization**: Generating type-specific fast paths at the SSA level using runtime feedback.
        - **Memory Optimization**: Performing more accurate escape analysis to scalarize objects and allocate them to registers.

### Register Machine
The LIR of ValkyrieVM is a register-based instruction set, optimized for interpretation speed and easy to map from SSA.

### Phi Elimination
SSA is lowered to LIR by eliminating Phi nodes and performing register allocation.
