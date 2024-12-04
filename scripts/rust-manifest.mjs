import { join } from "node:path";

/** Rust workspace lives under `projects/nyar.rs/`, not the repo root. */
export const RUST_WS_DIR = "projects/nyar.rs";
export const RUST_MANIFEST = join(RUST_WS_DIR, "Cargo.toml");
