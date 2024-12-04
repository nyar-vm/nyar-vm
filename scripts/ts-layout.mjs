import { join } from "node:path";

export const TS_ROOT = join("projects", "nyar.ts");

const PLATFORM_DIRS = new Set([
    "nyar-win32-x64",
    "nyar-linux-x64",
    "nyar-darwin-x64",
    "nyar-darwin-arm64",
    "nyar-unknown-wasm32",
]);

const EMULATOR_DIRS = new Set(["bash", "lua", "tcl", "valkyrie"]);

/**
 * @param {string} dirName
 * @returns {string}
 */
export function resolveTsPackageDir(dirName) {
    if (PLATFORM_DIRS.has(dirName)) {
        return join(TS_ROOT, "platforms", dirName);
    }
    if (EMULATOR_DIRS.has(dirName)) {
        return join(TS_ROOT, "emulators", dirName);
    }
    return join(TS_ROOT, "packages", dirName);
}
