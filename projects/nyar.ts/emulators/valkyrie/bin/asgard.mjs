#!/usr/bin/env node --experimental-strip-types
/**
 * `asgard` 命令入口。
 *
 * 参数解析与执行由本包的 `runValkyrieCli`（`valkyrie-napi` 绑定）承担；本文件只
 * 转发命令名与 argv，不复制 `--help` / `--version` 等 CLI 语义。
 */

import {runValkyrieCli} from '../src/index.ts';

process.exit(await runValkyrieCli('asgard', process.argv.slice(2)));
