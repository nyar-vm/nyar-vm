# nyar source

Reusable transformation, analysis, optimization and specialization tools with neutral backend selection, partitioning and artifact orchestration. Each language owns its AST/IR/MIR and semantics. Each target owns plans, LIR, runtime wrappers and code/container services.

`abstractions` describes neutral interfaces, `planning` records identity/link/partition requirements, `selection` selects explicit capabilities, `lanes` adapts validated partitions to target inputs, and `packaging` delivers artifacts. No stage requires a language-independent semantic program.

Language-specific static closure rules do not prohibit dynamic runtime operations. Dynamic dispatch must be explicit and validated against a runtime contract. Interpreter specialization preserves unknown operations rather than pretending they are closed static calls.

Current Valkyrie canonical and common executable contracts are migration debt. They must be renamed and returned to the language or separated into target-owned plans. Nyar must not invent a CPU/VM shared IR or use OdinVM as an intermediate for unrelated targets.
