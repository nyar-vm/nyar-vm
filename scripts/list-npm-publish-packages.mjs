#!/usr/bin/env node
/**
 * Published `@nyar/*` npm packages under `projects/nyar.ts/`.
 * Mirrors `nifty.config.ts` `publish.packages` (bash/lua/tcl emulators excluded).
 */

import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const TS_ROOT = join(ROOT, "projects", "nyar.ts");

/** @type {{ name: string, dir: string }[]} */
const PUBLISH_SPECS = [
    { name: "@nyar/nyar-win32-x64", dir: "platforms/nyar-win32-x64" },
    { name: "@nyar/nyar-linux-x64", dir: "platforms/nyar-linux-x64" },
    { name: "@nyar/nyar-darwin-x64", dir: "platforms/nyar-darwin-x64" },
    { name: "@nyar/nyar-darwin-arm64", dir: "platforms/nyar-darwin-arm64" },
    { name: "@nyar/nyar-unknown-wasm32", dir: "platforms/nyar-unknown-wasm32" },
    { name: "@nyar/nyar", dir: "packages/nyar" },
    { name: "@nyar/valkyrie-win32-x64", dir: "platforms/valkyrie-win32-x64" },
    { name: "@nyar/valkyrie-linux-x64", dir: "platforms/valkyrie-linux-x64" },
    { name: "@nyar/valkyrie-darwin-x64", dir: "platforms/valkyrie-darwin-x64" },
    { name: "@nyar/valkyrie-darwin-arm64", dir: "platforms/valkyrie-darwin-arm64" },
    { name: "@nyar/valkyrie", dir: "emulators/valkyrie" },
];

/** @returns {{ name: string, dir: string, packageJsonPath: string }[]} */
export function listNpmPublishPackages() {
    return PUBLISH_SPECS.map((spec) => {
        const absDir = join(TS_ROOT, spec.dir);
        const packageJsonPath = join(absDir, "package.json");
        if (!existsSync(packageJsonPath)) {
            throw new Error(`missing package.json for ${spec.name}: ${packageJsonPath}`);
        }
        const pkg = JSON.parse(readFileSync(packageJsonPath, "utf8"));
        if (pkg.name !== spec.name) {
            throw new Error(`package name mismatch in ${packageJsonPath}: expected ${spec.name}, got ${pkg.name}`);
        }
        return { ...spec, packageJsonPath };
    });
}
