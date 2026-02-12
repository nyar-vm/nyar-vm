# VM Operations

此目錄包含 Nyar VM 指令集的核心執行邏輯。

## 定位 (Positioning)

`operations` 模組負責虛擬機的**骨架 (Skeleton)** 與**控制流 (Control Flow)**。它定義了 VM 如何解釋字節碼、管理調用棧以及處理指令分發。

## 主要職責

- **指令分發 (Dispatch)**: 在 `mod.rs` 的 `dispatch_instruction` 中將字節碼映射到具體的執行函數。
- **控制流 (Control Flow)**: 處理跳轉 (`control.rs`)、函數調用 (`call.rs`) 和返回。
- **棧管理 (Stack Management)**: 處理操作數棧的壓棧、彈棧和變量存取 (`stack.rs`)。
- **高級特性**: 實現閉包 (`closure.rs`)、代數效應 (`effects.rs`) 和元編程 (`metaprogramming.rs`) 指令。

## 與 Intrinsics 的關係

`operations` 關注的是**「如何執行指令」**。當涉及到具體數據類型的運算邏輯（如 `I32Add` 或 `StringSubstr`）時，它會調用 `intrinsics` 模組中的具體實現。
