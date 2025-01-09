#!/usr/bin/env node
/**
 * Fail when published `@nyar/*` packages disagree on `version`.
 *
 * Usage:
 *   node scripts/check-nyar-workspace-versions.mjs
 */

import { readFileSync } from "node:fs";

import { listNpmPublishPackages } from "./list-npm-publish-packages.mjs";

/** @type {Map<string, string[]>} */
const byVersion = new Map();

for (const { name, packageJsonPath } of listNpmPublishPackages()) {
    const pkg = JSON.parse(readFileSync(packageJsonPath, "utf8"));
    const list = byVersion.get(pkg.version) ?? [];
    list.push(name);
    byVersion.set(pkg.version, list);
}

if (byVersion.size <= 1) {
    const [only] = byVersion.keys();
    console.log(`ok: ${only} (${byVersion.get(only)?.length} packages)`);
    process.exit(0);
}

console.error("published @nyar/* package versions are out of sync:");
for (const [ver, names] of [...byVersion.entries()].sort()) {
    console.error(`  ${ver}:`);
    for (const name of names) console.error(`    - ${name}`);
}
process.exit(1);
