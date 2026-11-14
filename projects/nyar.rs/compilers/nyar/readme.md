# nyar

Nyar orchestrates reusable transformations, analysis, optimization, specialization, backend selection, partitioning and artifact assembly. Each language retains its own AST, IR/MIR, interpreter and semantic validation.

Object algebras and E-Graphs are optional tools with explicit interfaces and legality supplied by their users. They are not a mandatory shared semantic pipeline. Neutral identity, import, link, partition and artifact contracts permit independent language-to-target adapters.

For dynamic languages, Futamura's first projection specializes a language interpreter with respect to a known program. Unknown operations remain in the residual program with explicit runtime requirements. Target lowering is a separate step, owned by Wasm, CLR, JVM, OVM or native adapters and code services.

Planning consumes `LinkedSemanticProgram` — neutral identity, fragment and capability facts submitted by a language frontend after its own semantic validation. Nyar owns no language MIR; do not reintroduce a canonical or opt-in shared semantic bus. OdinVM is only the OVM runtime, not a required stage for other targets.
