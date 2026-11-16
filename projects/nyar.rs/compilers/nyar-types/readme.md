# nyar-types

Neutral identity, linkage, capability, partition, provenance and artifact contracts for independently owned language and target implementations.

The crate must not own a language AST/MIR, a universal executable representation, target encoding or interpreter state. Shared identity is not shared semantics.

The shared executable view (`executable`) has been removed from this crate: SSA/CFG executable contracts now belong to the target lane (`nyar-emitter`), and language program types belong to Valkyrie. `canonical_program` and the related layout contracts still contain cross-target assumptions and remain migration debt; do not advertise them as a universal or optional common MIR.

Host import contracts must retain explicit identity, signatures, calling conventions and ownership. Backend-private representation must not become the semantic bus between languages and targets.
