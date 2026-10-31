import {describe, it} from 'node:test';
import assert from 'node:assert/strict';

import {runLegionCli} from '../src/cli/legion-cli.ts';
import {runAsgardCli} from '../src/cli/asgard-cli.ts';

describe('valkyrie Legion CLI', () => {
    it('runLegionCli help exits 0', async () => {
        const code = await runLegionCli(['help']);
        assert.equal(code, 0);
    });

    it('unknown command exits non-zero', async () => {
        const code = await runLegionCli(['no-such-command']);
        assert.ok(code !== 0);
    });
});

describe('valkyrie Asgard CLI', () => {
    it('runAsgardCli help exits 0', async () => {
        const code = await runAsgardCli(['help']);
        assert.equal(code, 0);
    });

    it('build --static without entry exits non-zero', async () => {
        const code = await runAsgardCli(['build', '--static']);
        assert.ok(code !== 0);
    });

    it('build --static with unknown option exits non-zero', async () => {
        const code = await runAsgardCli(['build', '--static', 'source/index.w', '--bogus']);
        assert.ok(code !== 0);
    });
});
