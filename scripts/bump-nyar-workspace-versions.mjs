#!/usr/bin/env node
/**
 * Bump every published `@nyar/*` workspace package to the same semver.
 *
 * Usage:
 *   node scripts/bump-nyar-workspace-versions.mjs 0.0.15
 */

import { readFileSync, writeFileSync } from "node:fs";

import { listNpmPublishPackages } from "./list-npm-publish-packages.mjs";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(version)) {
    console.error("usage: node scripts/bump-nyar-workspace-versions.mjs <semver>");
    process.exit(1);
}

const touched = [];
for (const { name, packageJsonPath } of listNpmPublishPackages()) {
    const raw = readFileSync(packageJsonPath, "utf8");
    const pkg = JSON.parse(raw);
    if (pkg.version === version) continue;
    pkg.version = version;
    writeFileSync(packageJsonPath, `${JSON.stringify(pkg, null, 4)}\n`, "utf8");
    touched.push(name);
}

if (touched.length === 0) {
    console.log(`all publish packages already at ${version}`);
} else {
    console.log(`bumped ${touched.length} package(s) to ${version}:`);
    for (const name of touched) console.log(`  ${name}`);
}
