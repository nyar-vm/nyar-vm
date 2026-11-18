# nyar-package-registry

Pluggable registry adapters for the Nyar package manager.

## Overview

The `Registry` trait, the third-party drivers (npm, JSR, NuGet, Maven, Conda) and the artifact
helpers are provided by Boost's `package-registry`. This crate is the product facade: it re-exports
that surface and adds the product-owned parts.

## Built-in registries

| Key        | Source             | Default endpoint constant         |
|:-----------|:-------------------|:----------------------------------|
| `npm`      | Boost              | `NpmRegistry::DEFAULT_ENDPOINT`   |
| `jsr`      | Boost              | `JsrRegistry::DEFAULT_ENDPOINT`   |
| `nuget`    | Boost              | `NugetRegistry::DEFAULT_ENDPOINT` |
| `maven`    | Boost              | `MavenRegistry::DEFAULT_ENDPOINT` |
| `conda`    | Boost              | `CondaRegistry::DEFAULT_ENDPOINT` |
| `valhalla` | this crate         | `ValhallaRegistry::DEFAULT_ENDPOINT` |

`default_registries()` builds the standard set. `registries_with_endpoints` overrides URLs from a
`BTreeMap<String, String>`.

## Product-owned parts

- `ValhallaRegistry` and `PublisherKey`: the product's own channel and Ed25519 publisher keys.
- `credentials`: discovery and sync against official CLI stores (`.npmrc`, Deno config,
  NuGet.Config, Maven settings, Anaconda nucleus). Credentials are host policy, so they stay here
  and Boost never reads user secrets.
- `RegistryCredentials`: extends `Registry` with `discover_credential`, `sync_credential` and
  `uses_external_credentials` for host-side orchestration.

`nyar-package-manager` consumes these adapters through dependency injection; this crate stays free of
lockfile parsing and dependency solving.

## Usage

```rust
use nyar_package_registry::{default_registries, Registry};

let registries = default_registries()?;
let npm = registries.get("npm").expect("npm adapter");
let pkg = npm.get_package("@scope/name", "1.0.0")?;
```

## Boundaries

- No manifest parsing (layout and filenames belong to the package manager and its callers).
- No dependency graph resolution or lockfile IO.
- `MockRegistry` is provided for unit tests in the manager crate.
