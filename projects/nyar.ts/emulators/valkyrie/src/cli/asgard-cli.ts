import {createRequire} from 'node:module';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

import {createCli} from '@vmz/commander';

import {ASGARD_CLI_CATALOG} from './catalog.ts';
import {runValkyrieCli} from '../native.ts';

const require = createRequire(import.meta.url);
const pkg = require('../../package.json') as {version: string};
const localesRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), '../../locales');

const META_COMMANDS = new Set(['help', '--help', '-h', 'version']);

/** Asgard：help/version 走 @vmz/commander，其余 argv 透传 Rust `asgard` CLI。 */
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
    return runValkyrieCli('asgard', argv);
}
