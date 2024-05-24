import {arch, platform} from 'node:os';

/** npm platform short id (`win32-x64`, …) from host OS. */
export function platformShort(triple?: string): string {
    const t = triple ?? platformTriple();
    if (t.startsWith('win32-')) return 'win32-x64';
    if (t.startsWith('linux-')) return 'linux-x64';
    if (t === 'darwin-arm64') return 'darwin-arm64';
    if (t === 'darwin-x64') return 'darwin-x64';
    return t;
}

/** Host triple used for platform-named native binaries (`nyar.<triple>.node`). */
export function platformTriple(): string {
    const {platform: os, arch: cpu} = {platform: platform(), arch: arch()};
    if (os === 'win32' && cpu === 'x64') return 'win32-x64-msvc';
    if (os === 'linux' && cpu === 'x64') return 'linux-x64-gnu';
    if (os === 'darwin' && cpu === 'arm64') return 'darwin-arm64';
    if (os === 'darwin' && cpu === 'x64') return 'darwin-x64';
    return `${os}-${cpu}`;
}

export function nativeBinaryName(triple?: string): string {
    return `nyar.${triple ?? platformTriple()}.node`;
}
