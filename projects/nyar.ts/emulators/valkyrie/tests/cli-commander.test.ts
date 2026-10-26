import {describe, it} from 'node:test';
import assert from 'node:assert/strict';

import {runLegionCli} from '../src/cli/legion-cli.ts';

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
