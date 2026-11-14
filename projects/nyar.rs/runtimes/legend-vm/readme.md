# legend-vm

Language-neutral guest execution and specialization substrate. Languages own AST/IR, semantics and interpreters in `languages/rusty-<lang>`. LegendVM provides registration, dispatch, thin value adapters and specialization seams. It must not duplicate complete language tree-walk interpreters or depend conceptually on OdinVM as its execution foundation.

Futamura's first projection specializes a language interpreter with respect to a known program: `R_L,P = S(I_L, P)`. The residual program preserves unknown runtime operations and explicit language/runtime dependencies. Nyar provides reusable transformation and analysis tools, while separate language-to-target adapters lower residual programs into Wasm, CLR, JVM, OVM or native representations.

This is an ownership contract, not a delivered support matrix. Current Lua specialization is wired to a limited `NativeResidualModule` path and Windows PE wrapping. Full per-operation native frame/stack code generation remains incomplete: integer exit-code folding or a stub is not full compilation. Other guest specialization paths can remain incomplete. Python and TypeScript directories are not active workspace members.

Current `.odin` or stack-residual probes do not prove production specialization. Future OVM support must implement its own adapter and runtime contract rather than route every target through OdinVM. Second/third projections remain future work.

Architecture guards forbid language parser copies, shared host-script semantic models and complete evaluator copies in this crate. Use guest-local interpreters and register thin adapters. Validate interpreter equivalence with runtime unknown inputs, not only compile-time constants.
