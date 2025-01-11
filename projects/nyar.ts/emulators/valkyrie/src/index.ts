import {createRequire} from 'node:module';

/**
 * `@nyar/valkyrie` —— Valkyrie npm 包装层。
 *
 * 本包只声明命令与导出身份；CLI 解析、编译与平台绑定选择全部委托给
 * `valkyrie-napi`，Rust 实现位于 `projects/nyar.rs/languages/rusty-valkyrie`。
 */

const require = createRequire(import.meta.url);
const pkg = require('../package.json') as {name: string; version: string};

/** npm 包名（与 `package.json` `name` 一致）。 */
export const PACKAGE_NAME = pkg.name;

/** 包装层版本（与 `package.json` `version` 一致）。 */
export const PACKAGE_VERSION = pkg.version;

/** 本包安装后提供的命令。 */
export const COMMANDS = ['legion', 'asgard'] as const;

/** 命令名。 */
export type Command = (typeof COMMANDS)[number];

export {runValkyrieCli, type ValkyrieProduct} from './native.ts';
