#!/usr/bin/env node --experimental-strip-types
/**
 * `asgard` 命令入口。
 *
 * 参数解析与执行由 `@nyar/nyar` 暴露的 `runValkyrieCli` 承担；本文件只转发
 * 命令名与 argv，不复制 `--help` / `--version` 等 CLI 语义。
 */

import {runValkyrieCli} from '@nyar/nyar';

process.exit(await runValkyrieCli('asgard', process.argv.slice(2)));
