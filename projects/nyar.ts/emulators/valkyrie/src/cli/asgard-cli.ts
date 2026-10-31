import {createRequire} from 'node:module';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

import {createCli} from '@vmz/commander';

import {ASGARD_CLI_CATALOG} from './catalog.ts';
import {asgardBuildStatic} from '../native.ts';

const require = createRequire(import.meta.url);
const pkg = require('../../package.json') as {version: string};
const localesRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../locales');

const META_COMMANDS = new Set(['help', '--help', '-h', 'version']);

/** 已在产品面登记、但尚未接入配置求值与产物管线的子命令。 */
const UNWIRED_COMMANDS = new Set(['serve', 'dev', 'pack', 'fmt']);

/**
 * 设计归属说明：Asgard 的构建内容由项目根 `asgard.config.v` 最后表达式决定，
 * 有类型配置求值（设计 §9 的 C0/C2）尚未闭合，命令层不得猜测产物参数。
 */
const CONFIG_PIPELINE_NOTE =
    'Asgard 的入口/输出/产物类型由项目根 `asgard.config.v` 的有类型求值决定（设计 §1、§6），' +
    '该配置求值链路（C0/C2）尚未闭合，命令层也不接受产物覆盖参数。';

/**
 * `asgard build --static <entry>`：TS 侧解析 argv 后调用正交 N-API（与 legion 同型）。
 * 其余 flag（`--title` / `--output` / 位置参数项目目录）在此消费；
 * 未知 flag 失败闭合，不透传。
 */
function runStaticBuild(argv: string[]): number {
    let entry: string | undefined;
    let title: string | undefined;
    let outputDir: string | undefined;
    let projectDir = process.cwd();
    for (let i = 0; i < argv.length; i += 1) {
        const token = argv[i];
        if (token === undefined) continue;
        if (token === '--static') entry = argv[i += 1];
        else if (token === '--title') title = argv[i += 1];
        else if (token === '--output' || token === '-o') outputDir = argv[i += 1];
        else if (!token.startsWith('-')) projectDir = token;
        else {
            console.error(`asgard build --static: 未知选项 ${token}`);
            return 1;
        }
    }
    if (entry === undefined || entry.length === 0) {
        console.error('asgard build --static 需要显式入口 <file.w|file.awsl>（`.w` 与 `.awsl` 同格式同语义）');
        return 1;
    }
    const outcome = asgardBuildStatic({projectDir: path.resolve(projectDir), entry, outputDir, title});
    if (!outcome.ok) {
        console.error(outcome.error ?? 'asgard static build failed');
        return 1;
    }
    console.log('asgard build (static) complete');
    console.log(`component: ${outcome.componentName}`);
    console.log(`output: ${outcome.outputDir}`);
    console.log(`page: ${outcome.pagePath}`);
    return 0;
}

/**
 * Asgard 命令编排入口。
 *
 * help/version 走 `@vmz/commander`；`build --static` 是当前唯一可用的静态交付入口，
 * 调用结构化 N-API。其余命令在 TS 层明确诊断并以非 0 退出，不转发 argv 到 native，
 * 也不在 TS 层复刻配置解析。
 */
export async function runAsgardCli(argv: string[] = process.argv.slice(2)): Promise<number> {
    const [cmd] = argv;
    if (!cmd || META_COMMANDS.has(cmd)) {
        const cli = createCli('asgard').catalog(() => ASGARD_CLI_CATALOG).intro('cli.intro');
        try {
            cli.locales(localesRoot);
        } catch {
            /* optional */
        }
        cli.command('version', 'cli.cmd.version').action(() => {
            console.log(`@nyar/valkyrie ${pkg.version}`);
            return 0;
        });
        return cli.parse(cmd ? argv : ['help']);
    }
    if (cmd === 'build') {
        if (argv.includes('--static')) return runStaticBuild(argv.slice(1));
        console.error(`asgard build: ${CONFIG_PIPELINE_NOTE}`);
        console.error('当前可用入口：`asgard build --static <entry.w|entry.awsl>`（显式静态交付，不读取配置）。');
        return 1;
    }
    if (UNWIRED_COMMANDS.has(cmd)) {
        console.error(`asgard ${cmd}: 尚未接入。${CONFIG_PIPELINE_NOTE}`);
        return 1;
    }
    console.error(`asgard: unknown asgard command '${cmd}'`);
    return 1;
}
