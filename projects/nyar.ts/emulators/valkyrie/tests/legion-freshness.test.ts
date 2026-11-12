import {describe, it} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

import {freshnessFromManifestView, isArtifactCacheFresh} from '../src/legion/testing.ts';

describe('legion artifact freshness fallback', () => {
    it('rejects a bare run-contracts.txt without a verifiable manifest', () => {
        const dir = mkdtempSync(join(tmpdir(), 'legion-fresh-'));
        writeFileSync(join(dir, 'run-contracts.txt'), 'not a manifest');
        const outcome = isArtifactCacheFresh(dir, 'ovm');
        assert.equal(outcome.fresh, false, `bare manifest file must not be trusted: ${outcome.reason}`);
        assert.ok(outcome.reason, 'stale verdict must carry a reason');
    });

    it('requires target match and complete compile evidence fields', () => {
        const view = {
            target: 'ovm',
            semanticClosureHash: 'semantic',
            canonicalSurfaceHash: 'surface',
            hostProviderBindingHash: 'provider',
        };
        assert.equal(freshnessFromManifestView(view, 'ovm').fresh, true);
        assert.equal(freshnessFromManifestView(view, 'wasm32-node-unknown-wasm').fresh, false);
        assert.equal(freshnessFromManifestView({...view, hostProviderBindingHash: null}, 'ovm').fresh, false);
        assert.equal(freshnessFromManifestView({...view, semanticClosureHash: ''}, 'ovm').fresh, false);
        assert.equal(freshnessFromManifestView(null, 'ovm').fresh, false);
    });
});
