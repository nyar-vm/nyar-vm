import {describe, it} from 'node:test';
import assert from 'node:assert/strict';

import {runCli} from '../src/cli/cli.ts';

describe('nyar CLI host', () => {
    it('runCli routes argv through @vmz/commander (unknown command exits 1)', async () => {
        const code = await runCli(['no-such-command']);
        assert.equal(code, 1);
    });

    it('runCli help exits 0 without native', async () => {
        const code = await runCli(['help']);
        assert.equal(code, 0);
    });
});
