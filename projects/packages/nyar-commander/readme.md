# @nyar/commander

i18n-first TypeScript CLI framework for Nyar host tools. Forked from the `@vmz/commander` contract:

- Command and option help strings are **catalog message ids**, not inline prose.
- Products supply catalogs via `.catalog()` or `.locales(root)`.
- Framework chrome (`commander.*`) ships tiny English fallbacks only.

`@nyar/nyar` registers product commands on top of this package; native capabilities stay in `nyar-napi`.
