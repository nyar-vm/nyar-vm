# nyar-types

Neutral identity, linkage, capability, partition, provenance and artifact contracts for independently owned language and target implementations.

The crate must not own a language AST/MIR, a universal executable representation, target encoding or interpreter state. Shared identity is not shared semantics.

Current `canonical_program`, `executable` and related layout contracts still contain Valkyrie-specific semantics and cross-target assumptions. They are migration debt: rename and return Valkyrie program types to Valkyrie, then place target plans and LIR with their target owners. Do not advertise them as a universal or optional common MIR.

Host import contracts must retain explicit identity, signatures, calling conventions and ownership. Backend-private representation must not become the semantic bus between languages and targets.
