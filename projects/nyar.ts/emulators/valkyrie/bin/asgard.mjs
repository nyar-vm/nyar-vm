#!/usr/bin/env node
/**
 * `asgard` 命令入口。
 *
 * 参数解析与执行由本包的 `runValkyrieCli`（`valkyrie-napi` 绑定）承担；本文件只
 * 转发命令名与 argv，不复制 `--help` / `--version` 等 CLI 语义。
 */

import {runValkyrieCli} from '../dist/index.js';

process.exit(await runValkyrieCli('asgard', process.argv.slice(2)));
