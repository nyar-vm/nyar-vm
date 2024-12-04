/**
 * `@nyar/valkyrie` —— Valkyrie npm 包装层。
 *
 * 本包只声明命令与导出身份；CLI 解析、编译与平台 collect 选择全部委托给
 * `@nyar/nyar`，Rust 实现位于 `examples/rusty-valkyrie/crates`。
 */

/** npm 包名。 */
export const PACKAGE_NAME = '@nyar/valkyrie';

/** 包装层版本。 */
export const PACKAGE_VERSION = '0.1.0';

/** 本包安装后提供的命令。 */
export const COMMANDS = ['legion', 'asgard'] as const;

/** 命令名。 */
export type Command = (typeof COMMANDS)[number];

export {runValkyrieCli} from '@nyar/nyar';
