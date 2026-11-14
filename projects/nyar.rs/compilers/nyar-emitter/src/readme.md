# nyar-emitter

Target code services and artifact assembly for Wasm, CLR, JVM, OVM and native. The driver routes validated partition requirements and target-specific inputs, then returns artifacts and execution contracts. Backend selection belongs to the shared selector rather than duplicated family algorithms.

Each language-to-target adapter consumes its language IR or specialized residual program and produces target-owned plans/LIR. Target wrappers may implement explicit runtime services. Encoders must not parse source, infer missing language facts or require a common semantic MIR.

Every entrance goes through `AdaptedFragmentInput`: a language-to-target adapter prepares `TargetExecutionPlan` (via `TargetPlanInput`) and submits it. There is no shared semantic MIR entrance and no `SharedSemanticSubset` capability; the semantic MIR now lives in the owning language crate. Cross-lane plan splitting and representation-ownership cleanup remain open work, tracked in the execution ledger.

OVM emits `.odin` for OdinVM. Other targets do not transit OdinVM. Native machine/object/link/container services are distinct from Node-API `.node` and CPython `.pyd` / Python `.so` ABI construction. Compiler-host FFI bindings do not demonstrate guest extension support.

Current family implementations and plan wiring do not establish complete language-to-target coverage. Validate from source through language semantics and target adaptation to an actual artifact and runtime result. Hand-filled fragments, encoded-image wrappers, exit-code folding and stubs are not equivalent evidence.
