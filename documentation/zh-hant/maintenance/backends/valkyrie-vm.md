# ValkyrieVM 維護指南

ValkyrieVM 是 Valkyrie 的內部參考解釋器。

## 編譯流水線

`Source -> AST -> HIR -> CFG -> SSA -> LIR -> VM Execution`

### 1. AST -> HIR
- 負責：消除語法糖，解析作用域，處理陰影（Shadowing）。
- 實現：[ast_to_hir](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-compiler/src/transform/ast_to_hir/mod.rs)。

### 2. HIR -> CFG
- 負責：將結構化控制流（if, while, loop）轉換為基本塊和跳轉。
- 實現：[hir_to_cfg](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-compiler/src/transform/hir_to_cfg/mod.rs)。

### 3. CFG -> SSA
- 負責：構造靜態單賦值形式，插入 Phi 節點。
- 實現：[cfg_to_ssa](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-compiler/src/transform/cfg_to_ssa/mod.rs)。

### 4. SSA -> LIR
- 負責：Phi 消除，寄存器分配，指令降級。
- 實現：[ssa_to_lir](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-compiler/src/transform/ssa_to_lir/mod.rs)。
- 當前狀態：
    - [x] 基礎 Phi 消除（通過在前驅塊末尾插入 Move）。
    - [x] 基礎指令降級。
    - [ ] 優化 Phi 消除（處理并行 Move 問題）。
    - [x] 基礎寄存器分配（基於活躍變數分析的簡單重用）。
    - [ ] 優化寄存器分配（需引入線性掃描或著色算法）。
    - [ ] 棧幀大小計算。

### 5. VM Execution
- 負責：執行 LIR 指令，管理運行棧、堆和 Effect 處理器。
- 實現：[valkyrie-runtime](file:///e:/RustroverProjects/nyar-framework/valkyrie.rs/projects/valkyrie-runtime/src/runtime/mod.rs)。
- 當前狀態：
    - [x] 基礎算術與邏輯指令。
    - [x] 跳轉與分支（Jmp, JmpIf）。
    - [x] 函數調用與返回（支持 Native 與 FFI）。
    - [x] 對象與數組操作（Alloc, Load, Store）。
    - [x] 結構化錯誤處理（Effect, Continuation）。
    - [x] 代碼覆蓋率統計。

### 6. FFI Mechanism
- **設計**：FFI 標記通過 Annotation (如 `↯import`) 掛載於 `micro` 函數聲明之上。
- **優勢**：
    - **類型安全**：利用函數的 `parameters` 和 `returns` 定義，編譯器能準確生成參數封送（Marshaling）代碼。
    - **多後端適配**：通過 `target` 參數（如 `wasm`, `jvm`, `clr`, `dll`）由不同後端解釋執行。
- **示例**：
    ```valkyrie
    ↯import(target: wasm, "wasi:random/insecure", "get-insecure-random-u64")
    micro get_random_u64() -> u64
    ```

### 7. Deep JIT Optimization
- **觸發層級**：JIT 編譯器工作在 **SSA / HIR** 層面，而非 LIR。
- **核心邏輯**：
    - **語義保留**：在 SSA 層面保留了完整的類型信息、對象邊界和顯式的效應（Effects）流向。
    - **優化能力**：
        - **跨函數內聯**：在 SSA 層面根據熱點進行深度內聯。
        - **類型特化**：利用運行時反饋，在 SSA 層面生成特定類型的快速路徑。
        - **內存優化**：進行更精確的逃逸分析，將對象標量化並分配至寄存器。

### 寄存器機
ValkyrieVM 的 LIR 是一個基於寄存器的指令集，優化了解釋速度，並且易於從 SSA 映射。

### Phi 消除
通過消除 Phi 節點並進行寄存器分配，將 SSA 降低為 LIR。
