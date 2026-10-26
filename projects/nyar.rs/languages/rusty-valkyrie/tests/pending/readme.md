# 待迁移的 Semantic / Emitter 合同测试

## `semantic_mir_paired_contracts.rs`（21 个 `#[test]`）

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

## 其它

`tests/frontend` 与 `tests/backend` 目录内仍有同类旧 API 用例，与 `nyar-emitter/tests/pending/` 同属迁移队列，未删逻辑，只是暂未编入默认 `cargo test`。
