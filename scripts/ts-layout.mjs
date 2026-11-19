import { join } from "node:path";

export const TS_ROOT = join("projects", "nyar.ts");

const PLATFORM_DIRS = new Set([
    "odin-vm-win32-x64",
    "odin-vm-linux-x64",
    "odin-vm-darwin-x64",
    "odin-vm-darwin-arm64",
    "odin-vm-unknown-wasm32",
]);

const EMULATOR_DIRS = new Set(["bash", "lua", "tcl"]);

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
