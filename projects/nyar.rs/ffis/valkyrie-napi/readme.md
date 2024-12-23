# valkyrie-napi

Node-API (**N-API**) native binding for the Valkyrie product surface: the `legion` / `asgard` CLI
and the programmatic `legionPlan` / `legionBuild` entry points. Build with
`cargo build -p valkyrie-napi --release`.

Compilation semantics stay in `rusty-valkyrie`; this crate only forwards argv, exit codes and
structured results. `legionRun` / `legionRunCaptured` / `legionSpyRun` / `legionParse` remain for
existing scripts.
