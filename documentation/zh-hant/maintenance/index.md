# Valkyrie 虛擬機維護指南

本指南面向 Valkyrie 虛擬機項目的**內部維護者和核心開發團隊**，介紹項目架構、模塊職責、內部維護流程和系統級設計決策。

> **目標讀者**: 項目維護者、核心開發團隊成員、系統架構師
> **內容重點**: 內部架構、維護流程、系統設計、代碼組織

## 項目架構概覽

Valkyrie 採用 Rust Monorepo (Workspace) 架構，每個組件封裝在獨立的 crate 中，提供清晰的依賴關係、獨立的測試環境和高效的並行編譯。

```
valkyrie/
├── Cargo.toml         # Workspace 根配置
└── projects/
    ├── valkyrie-types/    # 統一中間表示類型定義 (HIR, CFG, SSA, LIR)
    ├── valkyrie-compiler/ # 基於 Chomsky 的現代編譯器框架
    ├── valkyrie-runtime/  # 編譯器遍 (Pass) 與虛擬機執行核心
    ├── valkyrie-error/    # 基於 miette 的診斷系統
    └── legion/            # 命令行工具 (Valkyrie 工具鏈入口)
```

外部依賴:
- `oak-valkyrie`: 新版前端實現 (Lexer, Parser, AST)，位於 `../oaks`
- `ProjectChomsky`: 編譯器後端優化框架，位於 `../ProjectChomsky`

## 核心設計哲學

Valkyrie 的架構基於五大設計支柱：

### 1. 五階段流水線 (5-Stage Pipeline)

編譯器是對程序信息進行一系列保義變換的過程，建模為多個精心設計的中間表示串聯的流水線：

#### 階段 1: AST (Abstract Syntax Tree)
- **職責**: 忠實於源代碼的語法結構，保存完整的原始信息（如 SourceSpan）用於診斷。
- **當前實現**: 使用 `oak-valkyrie` 作為統一的前端實現。
- **關鍵處理**:
  - **宏展開 (Macro Expansion)**: 執行 `@macro` 和注記 `@.annotation`，生成新的 AST 節點。
  - **模版字符串解析**: 將 `f"hello {name}"` 等語法結構解析為 AST 表達式。
- **優化**: 
  - 極早期的常數折疊（如字面量拼接）。
  - 語法脫糖（如 `a += b` -> `a = a + b`）。

#### 階段 2: HIR (High-level IR)
- **職責**: 全局語義圖，包含完整的類型信息、作用域和名稱解析結果。
- **關鍵處理**:
  - **符號解析 (Symbol Resolution)**: 
    - 處理 `using` 導入，構建跨模塊的符號引用。
    - 建立命名空間層級結構，管理變量作用域（Scope）和影子變量（Shadowing）。
    - 綁定函數調用到具體的函數定義，處理重載解析。
  - **宏展開與屬性處理**:
    - 執行 `@macro` 和注記 `@.annotation`，在語義層面對 AST 進行結構化重組。
    - 處理內建屬性（如 `@inline`, `@tail_rec`）。
  - **類型檢查與推導 (Type Checking & Inference)**: 基於 C3 線性化算法的 Trait 解析和類型推導，確保類型安全性。
  - **模式匹配解糖 (Pattern Matching Desugaring)**: 
    - 將複雜的 `match` 結構轉換為決策樹（Decision Tree）。
    - 生成排他性檢查，確保模式匹配的完備性（Exhaustiveness Check）。
  - **代數效應脫糖 (Effect Handling Desugaring)**: 將 `try/handle` 結構映射到底層的控制流原語。
- **優化**:
  - **高層函數內聯 (Inlining)**: 基於啟發式算法減少小型函數的調用開銷。
  - **泛型特化 (Monomorphization)**: 針對具體類型生成專門的代碼路徑。
  - **死定義消除**: 移除從未被使用的導入和局部變量定義。

#### 階段 3: CFG (Control Flow Graph)
- **職責**: 以基本塊（BasicBlock）形式表示程序執行邏輯，控制流顯式化。
- **關鍵處理**:
  - **控制流線性化**: 將結構化的控制流（if/while/loop）轉換為 Goto 和 SwitchInt。
  - **解糖複雜的控制流**: 
    - 將模式匹配產生的決策樹展開為一系列的基本塊和條件跳轉。
    - 將代數效應的 `yield`/`resume` 映射到顯式的狀態保存和恢復。
  - **基本塊構造**: 識別領導語句（Leaders）並劃分基本塊。
- **優化**:
  - **死代碼刪除 (DCE)**: 移除不可達的基本塊。
  - **跳轉線程化 (Jump Threading)**: 簡化連續的條件跳轉，直接跳到最終目的地。
  - **基本塊合併**: 合併只有一個前驅且只有一個後繼的相鄰塊。

#### 階段 4: SSA (Static Single Assignment)
- **職責**: 變量版本化，確保每個變量僅被賦值一次，通過 Phi 節點處理控制流匯合。
- **關鍵處理**:
  - **支配關係計算 (Dominance Analysis)**: 計算支配樹和支配前沿。
  - **Phi 節點插入**: 在支配前沿處插入 Phi 函數，處理變量合併。
  - **變量重命名**: 對變量進行版本化編號，消除偽依賴。
- **優化**:
  - **全局數值編號 (GVN)**: 識別並消除計算上等價的冗餘表達式。
  - **稀疏條件常數傳播 (SCCP)**: 結合控制流分析的常數傳播，能夠折疊那些由於常數分支導致的死路徑。
  - **循環不變代碼外提 (LICM)**: 將循環無關計算移出循環體，減少重複計算。
  - **公共子表達式消除 (CSE)**。

#### 階段 5: LIR (Low-level IR)
- **職責**: 接近目標機器的線性指令集。
- **關鍵處理**:
  - **SSA 銷毀**: 通過複製或寄存器重分配消除 Phi 節點。
  - **指令選擇 (Instruction Selection)**: 根據目標平台（如 WASM 或自定義字節碼）選擇最優指令。
  - **寄存器分配 (Register Allocation)**: 線性掃描或圖著色算法。
- **雙重角色**:
  - **編譯目標**: 作為生成 WebAssembly 等外部目標的中間表示。
  - **虛擬機目標**: `ValkyrieVM` 作為一個參考解釋器，直接運行 LIR 指令集，確保在所有平台上行為一致。
- **優化**:
  - **窺孔優化 (Peephole Optimization)**: 局部指令序列替換。
  - **棧幀優化**: 減少不必要的入棧出棧。
  - **指令調度**: 優化執行流水線以減少延遲。

### 2. 開發者體驗的終極追求 (Uncompromising Developer Experience)

- **診斷即對話**: 使用 `miette` 框架提供 IDE 級別的診斷體驗
- **心流不被打斷**: 通過高效的編譯流水線實現亞秒級響應
- **直覺且強大的語言**: 提供代數效應、強大的模式匹配等高級抽象

### 3. 抽象的統一與對稱 (Unity and Duality of Abstractions)

基於數據與控制的對偶性：
- `match` 表達式：對數據的分解和模式匹配
- `handle` 表達式：對控制流（代數效應）的分解和模式匹配

### 4. 執行模型的二元性 (Duality of Execution Models)

- **動態解釋執行**: 專為開發、調試和交互式環境設計，內建完整運行時
- **靜態編譯執行**: 專為生產部署設計，編譯為輕量、高效的 WebAssembly 模塊

### 5. 零成本抽象的最終承諾 (Zero-Cost Abstraction)

高級抽象在編譯後應與手寫的最優底層代碼同樣高效。

## 核心模塊詳解

### oak-valkyrie: 編譯器前端實現
**職責**: 提供 Lexer, Parser 和 AST 定義，將源文本解析為抽象語法樹。

### valkyrie-types: 中間表示類型定義
**職責**: 集中管理 HIR, CFG, SSA, LIR 等各階段的中間表示類型定義。

### valkyrie-vm: 虛擬機與編譯器核心
**職責**: 實現各階段之間的轉換 (Pass) 與最終執行。

### valkyrie-error: 統一錯誤處理
**職責**: 提供集中的錯誤定義和診斷信息輸出。

## 維護流程

### 代碼審查標準
1. **架構一致性**: 確保新代碼符合五大設計支柱
2. **錯誤處理**: 使用統一的 `valkyrie-error` 系統
3. **性能考慮**: 避免不必要的分配和拷貝

### 調試指南
1. **編譯器錯誤**: 檢查 `valkyrie-error` 的診斷輸出
2. **代碼生成問題**: 使用 `--dump-{ast,hir,cfg,ssa,lir}` 選項轉儲中間層輸出

---

## 設計與實現專題

- [項目架構設計](project-architecture.md)
- [執行模型 (解釋與編譯)](execution-models.md)
- [對象降低 (Object Lowering)](object-lowering.md)
- [包管理與符號解析](package-management.md)
- [基於 Miette 的錯誤處理](error-handling.md)
- [性能優化策略](optimization-strategies.md)
- [後端實現與考量](backends/index.md)

---
本維護指南將隨著項目的發展持續更新。
