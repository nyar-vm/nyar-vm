# valkyrie-napi

Node-API (**N-API**) native binding for the Valkyrie product surface: Legion `legionPlan` /
`legionBuild` (structured API) and Asgard argv forwarding. Build with
`cargo build -p valkyrie-napi --release`.

Compilation semantics stay in `rusty-valkyrie`. Legion **does not** parse argv in Rust;
`legionRun` / `legionRunCaptured` / `runValkyrieCli('legion', …)` return errors. Use
`legionPlan` / `legionBuild` or the thin `legion` npm bin (`plan` / `build` only).
