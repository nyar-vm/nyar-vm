import assert from 'node:assert/strict';
import {describe, it} from 'node:test';

import {assertCatalogCoverage, createCli, parseOptionDef, parseOptions} from '../src/index.ts';

describe('@nyar/commander', () => {
    it('parseOptions maps flags and positionals', () => {
        const def = parseOptionDef('--family <id>', 'cli.opt.family');
        const parsed = parseOptions(['--family', 'wasi', 'rest'], [def]);
        assert.equal(parsed.family, 'wasi');
        assert.deepEqual(parsed._, ['rest']);
    });

    it('createCli + assertCatalogCoverage accept product catalog ids', () => {
        const cli = createCli('nyar')
            .catalog(() => ({
                'cli.intro': 'intro',
                'cli.cmd.ping': 'ping',
            }))
            .intro('cli.intro')
            .command('ping', 'cli.cmd.ping')
            .action(() => 0);
        assertCatalogCoverage(cli, {
            'cli.intro': 'intro',
            'cli.cmd.ping': 'ping',
        });
    });
});
