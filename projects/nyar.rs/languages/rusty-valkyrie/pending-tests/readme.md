# 待接线的测试源

本目录存放**已存在但当前无法从 harness `mod` 声明到达**的测试源，目录形状与 `tests/` 一致，便于逐块迁回。
它们**不参与 `cargo test`**：`tests/legion/cache/` 曾整目录漏接线且长期无人发现（「没接线的测试 = 永远绿的测试」）。

`tests/` 现在由 [`tests/legion/legion_test_discoverability.rs`](../tests/legion/legion_test_discoverability.rs) 强制守卫：任何 `tests/**/*.rs` 无法从 `tests/{legion,asgard,frontend,backend}/main.rs` 或 `tests/smir003_tests.rs` 的 `mod` 声明到达即失败并点名。测试源要么接线，要么放在本目录。

## 迁回要求

1. 把 `super::*` / 父作用域导入改为公开路径（`rusty_valkyrie::…`），补齐 `json!` 等父级导入。
2. 移回 `tests/` 对应目录，并在对应 `main.rs` / `mod.rs` 新增 `mod` 声明。
3. 跑守卫与目标测试；成功后再删除本目录中的对应源。

## 1. 旧 `src` 内嵌单测（52 个文件）

这些是 `src/**` 内嵌 `#[cfg(test)]` 模块的迁移副本（文件头注释 `Migrated from src/...`），依赖原父模块作用域，直接接线会产生大量 `cannot find ... in super` / 宏未导入错误。

| 目录 | 文件数 | 来源 |
|------|--------|------|
| `legion/`（`forward.rs`、`unity_export.rs`、`cmds/`） | 17 | `src/legion/{forward,unity_export}.rs`、`src/legion/cmds/**` |
| `asgard/`（`deploy.rs`、`host.rs`、`host_backend.rs`、`host_validation.rs`） | 4 | `src/asgard/{deploy,host,host_backend,host_validation}.rs` |
| `asgard/awsl/` | 5 | `src/asgard/awsl/**` |
| `asgard/codegen/` | 19 | `src/asgard/codegen/**` |
| `asgard/package/` | 3 | `src/asgard/package/**` |
| `asgard/platform_contract/` | 2 | `src/asgard/platform_contract/**` |
| `asgard/tailwind/` | 2 | `src/asgard/tailwind/**` |

## 2. `frontend/semantic_closure/semantic_mir_paired_contracts.rs`（21 个 `#[test]`）

**不是废弃样本。** `9d163dac` 从 `frontend/semantic_closure` 删掉的是 Rust 可执行 MIR 与 Valkyrie `MirModule` 的**配对观测**套件（`semantic_mir_observation` ↔ `validate_semantic_module`），覆盖例如：

- nominal sum / payload（SMIR006）
- array intrinsic（SMIR005）
- control-flow（SMIR007）
- static / external call、arity、signature
- text encoding / conversion / compare
- aggregate field / layout（SMIR010）
- missing entry（SMIR009）与各 backend fail-closed

删除后 **没有等价替代**：`semantic_mir_conformance_guards` 只做源码禁词扫描，不跑上述观测；`nyar-emitter/tests/lowering_features_physical_contract.rs` 是 **physical** 层，不是同一套 SMIR 配对。

### 仍有效的 API（迁移锚点）

- `nyar_emitter::testing::semantic_mir_observation(&FragmentSubmission, case_id)`
- `rusty_valkyrie::compiler::mir::validation::{validate_semantic_module, semantic_observation}`
- `FragmentSubmission::from_backend_plan(TargetExecutionPlan::…)` — 见 `compilers/nyar-emitter/tests/lowering_features_physical_contract.rs` 的 `submission()` 辅助函数

### 迁移要点（旧 → 新）

| 旧写法 | 新写法 |
|--------|--------|
| `submission.sum_types.push(...)` | `TargetExecutionPlan` / `sum_representations`（`validate_nominal_sums` 读 `backend_plan`） |
| `submission.executable = MirFunctionMapProvider::…` | `TargetExecutionPlan::from_functions` + `FragmentSubmission::from_backend_plan` |
| `Instruction { output, kind: Call { dispatch, intrinsic_opcode, … } }` | `Instruction { id, results, kind: InstructionKind::Call { callee, arguments }, provenance }` |
| `MirInstruction { output, kind: MirInstructionKind::… }` | `MirInstruction { id, results, kind: MirOperation::…, provenance }` |

### 目标

1. 按上表改写 `semantic_mir_paired_contracts.rs` 中的构造辅助函数。
2. 迁回 `tests/frontend/semantic_closure/` 并在 `mod.rs` 重新 `mod semantic_mir_paired_contracts;`。
3. 恢复 `frontend` 集成测试目标（或拆成独立 `[[test]]`）使 **legion cov / integration** 再次执行这 21 个用例。

## 3. 其它

`tests/frontend` 与 `tests/backend` 目录内仍有同类旧 API 用例，与 `nyar-emitter/tests/pending/` 同属迁移队列，未删逻辑，只是暂未编入默认 `cargo test`；它们仍在 `tests/` 内且可从 harness 到达，因此不受本目录影响。
