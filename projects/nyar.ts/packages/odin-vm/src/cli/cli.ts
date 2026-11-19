import {spawnSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import path from 'node:path';

import type {Cli, Command, ParsedOptions} from '@vmz/commander';
import {createCli} from '@vmz/commander';

import {NYAR_CLI_CATALOG} from './catalog.ts';
import {
    getHostInfo,
    invokeOdinVmExport,
    listOdinVmExports,
    listRuntimeFamilies,
    prepareRuntimeCommand,
    runPreparedCommand,
} from '../native.ts';
import {hostPlatformKey, resolveCollect} from '../platform.ts';

const require = createRequire(import.meta.url);
const pkg = require('../../package.json') as { version: string };

function buildCli(): Cli {
    const cli = createCli('odin-vm').catalog(() => NYAR_CLI_CATALOG).intro('cli.intro');

    cli.command('version', 'cli.cmd.version').action(() => {
        const info = getHostInfo();
        console.log(`@nyar/odin-vm ${pkg.version}`);
        console.log(`host ${info.hostProtocol} (${info.binding})`);
        return 0;
    });

    cli.command('host-info', 'cli.cmd.host-info').action(() => {
        const collect = resolveCollect();
        const info = getHostInfo();
        console.log(
            JSON.stringify(
                {
                    version: pkg.version,
                    host: hostPlatformKey(),
                    hostProtocol: info.hostProtocol,
                    binding: info.binding,
                    collect,
                },
                null,
                2,
            ),
        );
        return 0;
    });

    const runtime = cli.command('runtime', 'cli.cmd.runtime');

    runtime.command('list', 'cli.cmd.runtime.list').action(() => {
        for (const family of listRuntimeFamilies()) {
            console.log(family);
        }
        return 0;
    });

    const runtimeOpts = (cmd: Command) =>
        cmd
            .option('--family <id>', 'cli.opt.family')
            .option('--artifact <path>', 'cli.opt.artifact')
            .option('--classpath <path>', 'cli.opt.classpath')
            .option('--entry <name>', 'cli.opt.entry')
            .option('--wasi-p3', 'cli.opt.wasi-p3');

    runtimeOpts(runtime.command('prepare', 'cli.cmd.runtime.prepare')).action((options: ParsedOptions) => {
        const family = requireString(options, 'family');
        const artifact = requireString(options, 'artifact');
        const entry = requireString(options, 'entry');
        const prepared = prepareRuntimeCommand({
            family,
            artifact: path.resolve(artifact),
            classpath: typeof options.classpath === 'string' ? options.classpath : undefined,
            entry,
            wasiP3: Boolean(options['wasi-p3']),
        });
        console.log(JSON.stringify(prepared, null, 2));
        return 0;
    });

    runtimeOpts(runtime.command('run', 'cli.cmd.runtime.run'))
        .option('--cwd <dir>', 'cli.opt.cwd')
        .action((options: ParsedOptions) => {
            const family = requireString(options, 'family');
            const artifact = requireString(options, 'artifact');
            const entry = requireString(options, 'entry');
            const prepared = prepareRuntimeCommand({
                family,
                artifact: path.resolve(artifact),
                classpath: typeof options.classpath === 'string' ? options.classpath : undefined,
                entry,
                wasiP3: Boolean(options['wasi-p3']),
            });
            const cwd = typeof options.cwd === 'string' ? path.resolve(options.cwd) : process.cwd();
            const code = runPreparedCommand(prepared, cwd);
            return code;
        });

    const ovm = cli.command('ovm', 'cli.cmd.ovm');

    ovm.command('exports', 'cli.cmd.ovm.exports')
        .option('--module <path>', 'cli.opt.module')
        .action((options: ParsedOptions) => {
            const modulePath = path.resolve(requireString(options, 'module'));
            for (const symbol of listOdinVmExports(modulePath)) {
                console.log(symbol);
            }
            return 0;
        });

    ovm.command('invoke', 'cli.cmd.ovm.invoke')
        .option('--module <path>', 'cli.opt.module')
        .option('--entry <name>', 'cli.opt.entry')
        .option('--args <json>', 'cli.opt.args')
        .action((options: ParsedOptions) => {
            const modulePath = path.resolve(requireString(options, 'module'));
            const entry = requireString(options, 'entry');
            const raw = typeof options.args === 'string' ? JSON.parse(options.args) : [];
            if (!Array.isArray(raw)) {
                throw new Error('--args must be a JSON array');
            }
            const result = invokeOdinVmExport(modulePath, entry, raw);
            console.log(JSON.stringify(result, null, 2));
            return 0;
        });

    return cli;
}

function requireString(options: ParsedOptions, key: string): string {
    const value = options[key];
    if (typeof value !== 'string' || !value) {
        throw new Error(`missing required --${key}`);
    }
    return value;
}

/** 解析 argv 并执行 OdinVM CLI（@vmz/commander 编排；能力调用走 nyar-napi）。 */
export async function runCli(argv: string[] = process.argv.slice(2)): Promise<number> {
    const [cmd] = argv;
    if (cmd === 'wasm' && resolveCollect().kind === 'wasm') {
        return forwardWasm(argv.slice(1));
    }
    return buildCli().parse(argv);
}

function forwardWasm(rest: string[]): number {
    const collect = resolveCollect();
    if (collect.kind !== 'wasm') {
        console.error('odin-vm: wasm forward requires wasm collect');
        return 1;
    }
    if (!existsArtifact(collect.entry)) {
        console.error(`odin-vm: missing wasm collect at ${collect.entry}`);
        return 1;
    }
    const result = spawnSync('wasmtime', [collect.entry, ...rest], {stdio: 'inherit', shell: false});
    return result.status ?? 1;
}

function existsArtifact(filePath: string): boolean {
    try {
        readFileSync(filePath);
        return true;
    } catch {
        return false;
    }
}
