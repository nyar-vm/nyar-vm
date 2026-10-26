# hir

HIR structure definitions.

## Responsibilities

- Represent high-level program shape after basic semantic organization.
- Provide stable input for MIR (SSA) lowering and type-related analysis.

## Boundaries

- 不含目标专用编码。
- HIR 不作为跨所有编译阶段的万能载体。
