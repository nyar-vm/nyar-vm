#!/usr/bin/env node
/**
 * 为 npm 包生成可发布的 JS 入口。
 *
 * Node 拒绝对 `node_modules` 内的 `.ts` 剥离类型
 * （ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING），因此 `main` / `exports` / `bin`
 * 不能直接指向 TypeScript 源。这里用 Node 内置的 `stripTypeScriptTypes` 把 `src/`
 * 镜像到 `dist/`（保持目录深度，源码里的 `../../package.json` 等相对路径不变），
 * 并把相对说明符的 `.ts` 扩展名改写为 `.js`。
 *
 * 用法：
 *   node scripts/build-npm-package-js.mjs                       # 构建全部发布包
 *   node scripts/build-npm-package-js.mjs projects/nyar.ts/packages/nyar
 */

import {mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

/** 需要生成 JS 发布入口的包（相对仓库根）。 */
const PACKAGE_DIRS = [
    'projects/nyar.ts/packages/nyar',
    'projects/nyar.ts/emulators/valkyrie',
];

/** 只改写相对说明符的扩展名：`'./x.ts'`、`"../x.ts"` → `.js`。 */
const TS_SPECIFIER = /(['"])(\.{1,2}\/[^'"]*?)\.ts\1/g;

/** 递归列出目录下的所有文件。 */
function listFiles(dir) {
    const out = [];
    for (const entry of readdirSync(dir, {withFileTypes: true})) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) out.push(...listFiles(full));
        else if (entry.isFile()) out.push(full);
    }
    return out;
}

/**
 * 把单个包的 `src/` 编译为 `dist/`。
 *
 * @param {string} packageDir 相对仓库根的包目录
 */
export function buildNpmPackageJs(packageDir) {
    if (typeof stripTypeScriptTypes !== 'function') {
        throw new Error(`需要 Node >= 22.13 的 stripTypeScriptTypes（当前 ${process.version}）`);
    }
    const srcDir = path.join(ROOT, packageDir, 'src');
    const outDir = path.join(ROOT, packageDir, 'dist');
    if (!statSync(srcDir).isDirectory()) {
        throw new Error(`missing ${path.relative(ROOT, srcDir)}`);
    }

    rmSync(outDir, {recursive: true, force: true});
    const written = [];
    for (const file of listFiles(srcDir)) {
        if (!file.endsWith('.ts')) continue;
        const relative = path.relative(srcDir, file).replace(/\\/g, '/');
        const target = path.join(outDir, relative.replace(/\.ts$/, '.js'));
        const source = readFileSync(file, 'utf8');
        const stripped = stripTypeScriptTypes(source, {mode: 'strip'}).replace(TS_SPECIFIER, '$1$2.js$1');
        mkdirSync(path.dirname(target), {recursive: true});
        writeFileSync(target, stripped);
        written.push(path.relative(ROOT, target).replace(/\\/g, '/'));
    }
    return written;
}

/** 生成所有 npm 包的 JS 入口，返回写入的文件列表。 */
export function buildAllNpmPackageJs() {
    const written = [];
    for (const dir of PACKAGE_DIRS) written.push(...buildNpmPackageJs(dir));
    return written;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
    const requested = process.argv.slice(2);
    const targets = requested.length > 0 ? requested : PACKAGE_DIRS;
    for (const dir of targets) {
        for (const file of buildNpmPackageJs(dir)) console.log(`built ${file}`);
    }
}
