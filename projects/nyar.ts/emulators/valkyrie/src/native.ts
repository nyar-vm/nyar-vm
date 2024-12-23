import {createRequire} from 'node:module';
import {existsSync} from 'node:fs';
import {arch, platform} from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

/** Valkyrie 编译器 CLI 产品名。 */
export type ValkyrieProduct = 'legion' | 'asgard';

/** `valkyrie-napi` 导出的产品面。 */
type ValkyrieNativeAddon = {
    runValkyrieCli(product: string, argv: string[]): number;
};

const pkgRoot = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);

/** Host triple used for platform-named native binaries (`valkyrie.<triple>.node`). */
export function platformTriple(): string {
    const os = platform();
    const cpu = arch();
    if (os === 'win32' && cpu === 'x64') return 'win32-x64-msvc';
    if (os === 'linux' && cpu === 'x64') return 'linux-x64-gnu';
    if (os === 'darwin' && cpu === 'arm64') return 'darwin-arm64';
    if (os === 'darwin' && cpu === 'x64') return 'darwin-x64';
    return `${os}-${cpu}`;
}

/** npm platform short id (`win32-x64`, …) from host OS. */
export function platformShort(triple: string = platformTriple()): string {
    if (triple.startsWith('win32-')) return 'win32-x64';
    if (triple.startsWith('linux-')) return 'linux-x64';
    if (triple === 'darwin-arm64') return 'darwin-arm64';
    if (triple === 'darwin-x64') return 'darwin-x64';
    return triple;
}

/** Host triple used for platform-named native binaries (`valkyrie.<triple>.node`). */
export function nativeBinaryName(triple: string = platformTriple()): string {
    return `valkyrie.${triple}.node`;
}

/** 当前宿主 OS/arch 的 npm 平台标识。 */
export function hostPlatformKey(): string {
    return `${platform()}-${arch()}`;
}

/**
 * Resolve native `.node` via the platform optionalDependency (`@nyar/valkyrie-<short>`).
 */
export function resolveNativePath(): string {
    const triple = platformTriple();
    const short = platformShort(triple);
    const packageName = `@nyar/valkyrie-${short}`;
    const binary = nativeBinaryName(triple);
    const candidates: string[] = [];

    try {
        const pkgJson = require.resolve(`${packageName}/package.json`);
        const pkgDir = path.dirname(pkgJson);
        candidates.push(path.join(pkgDir, 'lib', binary));
        candidates.push(path.join(pkgDir, binary));
    } catch {
        /* optional dep not installed */
    }

    candidates.push(path.join(pkgRoot, '..', '..', '..', 'platforms', `valkyrie-${short}`, 'lib', binary));
    candidates.push(path.join(pkgRoot, '..', '..', '..', 'platforms', `valkyrie-${short}`, binary));
    candidates.push(path.join(pkgRoot, '..', 'node_modules', packageName, 'lib', binary));
    candidates.push(path.join(pkgRoot, 'node_modules', packageName, 'lib', binary));

    for (const candidate of candidates) {
        if (existsSync(candidate)) return candidate;
    }

    throw new Error(
        `valkyrie native addon not found for ${packageName}. Run: node scripts/build.mjs napi\n` +
        `Looked in:\n${candidates.map((c) => `  - ${c}`).join('\n')}\n` +
        `(host ${hostPlatformKey()})`,
    );
}

let nativeAddon: ValkyrieNativeAddon | undefined;

/** 加载当前平台的 valkyrie-napi 绑定。 */
export function loadNative(): ValkyrieNativeAddon {
    if (nativeAddon) return nativeAddon;
    nativeAddon = require(resolveNativePath()) as ValkyrieNativeAddon;
    return nativeAddon;
}

/**
 * 通过 valkyrie-napi 执行 Valkyrie CLI，原样返回退出码。
 *
 * argv 不解析、不改写；`--help` / `--version` 等语义全部由 Rust CLI 承担。
 */
export async function runValkyrieCli(product: ValkyrieProduct, argv: string[]): Promise<number> {
    return loadNative().runValkyrieCli(product, argv);
}
