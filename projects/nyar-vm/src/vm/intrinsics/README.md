# VM Intrinsics

此目錄包含 Nyar VM 內建的核心算子實現。

## 定位 (Positioning)

`intrinsics` 模組負責虛擬機的**血肉 (Flesh)** 與**計算邏輯 (Computation Logic)**。它定義了各種數據類型的底層運算規則。

## 主要職責

- **算術運算 (Arithmetic)**: 實現整數 (`arith/i32.rs`, `i64.rs`) 和浮點數 (`arith/float.rs`) 的基礎運算。
- **複雜類型**: 處理大整數 (`bigint.rs`)、字符串 (`string.rs`) 和字節數組 (`bytes.rs`) 的核心操作。
- **語義統一**: 確保跨平台的計算結果一致性，例如在 `string.rs` 中強制執行的 UTF-8 邊界檢查。
- **性能優化**: 提供熱路徑上的直接調用函數，避免通過哈希表查表調用外部 FFI。

## 與 Operations 的關係

`intrinsics` 關注的是**「具體算子怎麼算」**。它被 `operations` 模組調用，為字節碼指令提供實際的計算能力。它通常不感知 VM 的指令流或調用棧結構，僅專注於數據本身的變換。
