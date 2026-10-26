import {createRequire} from 'node:module';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

import type {Cli, ParsedOptions} from '@vmz/commander';
import {createCli} from '@vmz/commander';

import {LEGION_CLI_CATALOG} from './catalog.ts';
import {legionBuild, legionPlan, legionTest} from '../native.ts';

const require = createRequire(import.meta.url);
const pkg = require('../../package.json') as {version: string};
const localesRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../locales');

function projectDir(options: ParsedOptions): string {
    const positional = options._;
    const first = Array.isArray(positional) ? positional[0] : undefined;
    return path.resolve(typeof first === 'string' && first.length > 0 ? first : process.cwd());
}

function optionalTarget(options: ParsedOptions): string | undefined {
    const value = options.target;
    return typeof value === 'string' && value.length > 0 ? value : undefined;
}

function buildLegionCli(): Cli {
    const cli = createCli('legion').catalog(() => LEGION_CLI_CATALOG).intro('cli.intro');
    try {
        cli.locales(localesRoot);
    } catch {
        /* locales optional in dev */
    }

    cli.command('version', 'cli.cmd.version').action(() => {
        console.log(`@nyar/valkyrie ${pkg.version}`);
        return 0;
    });

    cli.command('plan', 'cli.cmd.plan')
        .option('-t, --target <triple>', 'cli.opt.target')
        .action((options: ParsedOptions) => {
            const outcome = legionPlan({projectDir: projectDir(options), target: optionalTarget(options)});
            if (!outcome.ok) {
                console.error(outcome.error ?? 'plan failed');
                return 1;
            }
            console.log(`workspace: ${outcome.workspaceRoot}`);
            console.log(`project: ${outcome.project}`);
            console.log(`target: ${outcome.target}`);
            console.log(`sources: ${outcome.sourceCount}`);
            console.log(`semantic groups: ${outcome.semanticGroupCount}`);
            console.log(`dependencies: ${outcome.dependencyCount}`);
            for (const group of outcome.groups) {
                console.log(`  group ${group.dependencyKey}: ${group.fileCount} file(s)`);
            }
            console.log('plan: ok');
            return 0;
        });

    cli.command('build', 'cli.cmd.build')
        .option('-t, --target <triple>', 'cli.opt.target')
        .option('-o, --output <dir>', 'cli.opt.output')
        .action((options: ParsedOptions) => {
            const output = options.output;
            const outcome = legionBuild({
                projectDir: projectDir(options),
                target: optionalTarget(options),
                outputDir: typeof output === 'string' ? output : undefined,
            });
            if (!outcome.ok) {
                console.error(outcome.error ?? 'build failed');
                return 1;
            }
            console.log(`project: ${outcome.project}`);
            console.log(`target: ${outcome.target}`);
            console.log(`output: ${outcome.outputDir}`);
            for (const artifact of outcome.artifacts) {
                console.log(`artifact: ${artifact}`);
            }
            return 0;
        });

    cli.command('test', 'cli.cmd.test')
        .option('-t, --target <triple>', 'cli.opt.target')
        .action((options: ParsedOptions) => {
            const outcome = legionTest(projectDir(options), optionalTarget(options));
            if (!outcome.ok) {
                console.error(outcome.error ?? 'legion test failed');
                return 1;
            }
            console.log(`passed=${outcome.passed} failed=${outcome.failed} skipped=${outcome.skipped}`);
            return 0;
        });

    return cli;
}

/** 解析 argv 并执行 Legion CLI（@vmz/commander；能力走 valkyrie-napi）。 */
export async function runLegionCli(argv: string[] = process.argv.slice(2)): Promise<number> {
    return buildLegionCli().parse(argv);
}
