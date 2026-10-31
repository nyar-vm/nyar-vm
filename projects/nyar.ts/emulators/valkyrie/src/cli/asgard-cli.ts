import {createRequire} from 'node:module';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

import {createCli} from '@vmz/commander';

import {ASGARD_CLI_CATALOG} from './catalog.ts';
import {asgardBuildStatic, runValkyrieCli} from '../native.ts';

const require = createRequire(import.meta.url);
const pkg = require('../../package.json') as {version: string};
const localesRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../locales');

const META_COMMANDS = new Set(['help', '--help', '-h', 'version']);

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

/** Asgard：help/version 走 @vmz/commander；`build --static` 走正交 N-API；其余 argv 透传 Rust `asgard` CLI。 */
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
    if (cmd === 'build' && argv.includes('--static')) {
        return runStaticBuild(argv.slice(1));
    }
    return runValkyrieCli('asgard', argv);
}
