# Target lanes

Each lane consumes explicit partition requirements plus the input produced by its language-to-target adapter. It owns its target plan/LIR and runtime requirements.

- Native uses machine/object/link/container services.
- OVM uses its own instruction plan and `.odin` encoding for OdinVM.
- CLR uses CLI types, MSIL and assemblies.
- JVM uses class/method models and bytecode.
- Wasm uses Wasm module and host contracts.

Native and OVM are separate lanes. Other targets do not transit OVM. Optional additional targets must define their own contracts and evidence rather than inherit support by sharing a name.

Do not resolve language traits, rows, nominal types or handlers here. Explicit dynamic calls and effects belong to a validated language/runtime adaptation, and must not be disguised as closed static calls.
