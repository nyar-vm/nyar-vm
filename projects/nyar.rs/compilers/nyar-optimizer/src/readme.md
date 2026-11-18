# nyar-optimizer

Reusable object-algebraic transformations, equivalence rewriting, E-Graph sessions, extraction and specialization tools. The E-Graph core (hashcons, congruence rebuild, pattern saturation, cost-model extraction) lives in the language-neutral `compiler-optimize` crate; this crate owns the `Object Algebraic` program boundary, the `core.*` rewrite theory, the constant-folding interpretation and the projection planning surface. Languages supply semantic operations and legality, targets supply relevant costs and representation constraints. Using these tools does not require a shared language MIR or one mandatory node enum.

Futamura's first projection specializes an interpreter with respect to a program: `R_L,P = S(I_L, P)`. Unknown input-dependent operations and their runtime dependencies remain in the residual program. Target-family selection and target lowering are distinct from interpreter specialization, even when current APIs use `futa_*` names for projection families.

The current combination and projection interfaces are scaffolding. They do not prove complete interpreter specialization, Python/Lua support across targets, or second/third projection delivery. Concrete encoders belong to target code services, and language semantics remain in language crates.
