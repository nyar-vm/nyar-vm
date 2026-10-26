# compiler / types

跨阶段共享的 **源位置、诊断壳、标识符路径**，以及 **暂存** 的 HIR 数据结构载体（`types/hir`）。

## 目标所有权（收敛方向）

| 事实 | 最终单一位置 |
|------|----------------|
| 语言类型/声明 **身份** | `semantic` |
| HIR 表达式/语句 **节点** | `hir/model`（自 `types/hir` 迁入） |
| MIR 指令/值 | `mir` |
| `SourceID` / `SourceSpan` | 本目录或 `compiler/input` 元数据 |

当前 `hir/mod.rs` 仍 `pub use crate::compiler::types::hir::*` 是为了过渡；新代码应明确依赖目标模块，避免再扩根级 `types` 行为。

## 边界

- 本层 **不** 编排编译阶段。
- 不把控制流、推断、evidence 算法塞进共享类型模块。

## 相关

- HIR 模型说明：[`../hir/readme.md`](../hir/readme.md)
- witness 子目录：[`witness/readme.md`](witness/readme.md)
