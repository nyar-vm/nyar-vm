#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { platform, arch } from 'node:os';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));

function getBinaryPath() {
    const p = platform();
    const a = arch();

    let packageName = '';
    let binaryName = 'nyar';

    if (p === 'win32' && a === 'x64') {
        packageName = '↯nyar-vm/nyar-win32-x64';
        binaryName = 'nyar.exe';
    } else if (p === 'linux' && a === 'x64') {
        packageName = '↯nyar-vm/nyar-linux-x64';
    } else if (p === 'darwin' && a === 'x64') {
        packageName = '↯nyar-vm/nyar-darwin-x64';
    } else if (p === 'darwin' && a === 'arm64') {
        packageName = '↯nyar-vm/nyar-darwin-arm64';
    }

    if (packageName) {
        try {
            // In a standard npm install, the platform package is a sibling to this package
            const pkgPath = join(__dirname, '..', '..', packageName, binaryName);
            return pkgPath;
        } catch (e) {
            // ignore
        }
    }
    return null;
}

const binaryPath = getBinaryPath();

if (binaryPath) {
    const result = spawnSync(binaryPath, process.argv.slice(2), { stdio: 'inherit' });
    process.exit(result.status ?? 0);
} else {
    // Fallback to WASM
    import('↯nyar-vm/nyar-wasm32-wasi').then(({ run }) => {
        run.run();
    }).catch(err => {
        console.error('Failed to run nyar:', err);
        process.exit(1);
    });
}
