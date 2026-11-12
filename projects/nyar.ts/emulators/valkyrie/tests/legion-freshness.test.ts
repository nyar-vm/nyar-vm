import {describe, it} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

import {freshnessFromNativeOutcome, isArtifactCacheFresh} from '../src/legion/testing.ts';

describe('legion artifact freshness authority', () => {
    it('rejects a bare run-contracts.txt without a verifiable manifest', () => {
        const dir = mkdtempSync(join(tmpdir(), 'legion-fresh-'));
        writeFileSync(join(dir, 'run-contracts.txt'), 'not a manifest');
        const outcome = isArtifactCacheFresh(dir, 'ovm');
        assert.equal(outcome.fresh, false, `bare manifest file must not be trusted: ${outcome.reason}`);
        assert.ok(outcome.reason, 'stale verdict must carry a reason');
    });

    it('only lets fully-verified pass', () => {
        assert.deepEqual(freshnessFromNativeOutcome({fresh: true, reason: null, level: 'fully-verified'}), {fresh: true});
    });

    it('rejects artifact-self-consistent verdicts (inputs unverifiable)', () => {
        const outcome = freshnessFromNativeOutcome({
            fresh: false,
            reason: 'artifact self-consistent only: inputs not verified against current workspace; pass projectDir for formal reuse',
            level: 'artifact-self-consistent',
        });
        assert.equal(outcome.fresh, false);
        assert.match(outcome.reason ?? '', /inputs not verified/);
    });

    it('rejects a self-consistent outcome even if the validator mislabels it fresh', () => {
        const outcome = freshnessFromNativeOutcome({fresh: true, reason: null, level: 'artifact-self-consistent'});
        assert.equal(outcome.fresh, false);
        assert.match(outcome.reason ?? '', /not fully-verified/);
    });

    it('rejects stale verdicts with their reason', () => {
        const outcome = freshnessFromNativeOutcome({fresh: false, reason: 'artifact `app.odin` digest mismatch', level: 'stale'});
        assert.equal(outcome.fresh, false);
        assert.equal(outcome.reason, 'artifact `app.odin` digest mismatch');
    });

    it('rejects reuse when the native validator is missing', () => {
        const outcome = freshnessFromNativeOutcome(undefined);
        assert.equal(outcome.fresh, false);
        assert.match(outcome.reason ?? '', /legionCheckArtifactFresh unavailable/);
    });
});
