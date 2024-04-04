#!/usr/bin/env node
/**
 * Ensure npm `files` entries that list README.md exist beside each package readme.md.
 *
 * Usage:
 *   node scripts/prepare-npm-publish.mjs
 *   node scripts/prepare-npm-publish.mjs --check
 */

import { copyFileSync, existsSync, readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const PACKAGES_DIR = join(ROOT, "projects", "packages");
const CHECK_ONLY = process.argv.includes("--check");

function packageDirs() {
    return readdirSync(PACKAGES_DIR, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => join(PACKAGES_DIR, entry.name))
        .filter((dir) => existsSync(join(dir, "package.json")));
}

let missing = 0;

for (const dir of packageDirs()) {
    const readme = join(dir, "readme.md");
    const published = join(dir, "README.md");
    if (!existsSync(readme)) {
        console.error(`missing readme.md: ${dir}`);
        missing++;
        continue;
    }
    if (CHECK_ONLY) {
        if (!existsSync(published)) {
            console.error(`missing README.md (run prepare-npm-publish): ${dir}`);
            missing++;
        }
        continue;
    }
    copyFileSync(readme, published);
    console.log(`prepared ${published}`);
}

if (missing > 0) {
    process.exit(1);
}
