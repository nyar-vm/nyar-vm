import {existsSync, readFileSync, readdirSync} from 'node:fs';
import {join} from 'node:path';

import {loadNative} from '../native.ts';

export const NODE_WASM_TARGET = 'wasm32-node-unknown-wasm';

/** OVM/Nyar 产物 manifest 里的 target 字段以 canonical triple 为准；freshness 以 manifest 为准。 */
export const OVM_BUILD_TARGET = 'ovm';

/** @deprecated 矩阵历史名；新鲜度请用 {@link isArtifactCacheFreshForBuild}。 */
export const NYAR_VM_TARGET = OVM_BUILD_TARGET;

export type FreshnessResult = {fresh: boolean; reason?: string};

export function resolveArtifactDir(buildOutputDir: string, _target: string): string {
    return buildOutputDir;
}

export function readExecutionManifest(artifactDir: string): {
    schema_version: number;
    target: string;
    compile_evidence?: {
        semantic_closure_hash?: string;
        canonical_surface_hash?: string;
        host_provider_binding_hash?: string;
    };
} | null {
    const native = loadNative();
    const view = native.legionReadExecutionManifest?.(artifactDir);
    if (!view) {
        return null;
    }
    return {
        schema_version: view.schemaVersion,
        target: view.target,
        compile_evidence: {
            semantic_closure_hash: view.semanticClosureHash ?? undefined,
            canonical_surface_hash: view.canonicalSurfaceHash ?? undefined,
            host_provider_binding_hash: view.hostProviderBindingHash ?? undefined,
        },
    };
}

export function isArtifactCacheFresh(artifactDir: string, expectedTarget: string): FreshnessResult {
    const native = loadNative();
    if (native.legionCheckArtifactFresh) {
        const outcome = native.legionCheckArtifactFresh(artifactDir, expectedTarget);
        return {fresh: outcome.fresh, reason: outcome.reason ?? undefined};
    }
    const manifestPath = join(artifactDir, 'run-contracts.txt');
    if (!existsSync(manifestPath)) {
        return {fresh: false, reason: 'missing run-contracts.txt'};
    }
    return {fresh: true};
}

/** 用 manifest 内 target 做新鲜度校验（避免硬编码 canonical triple）。 */
export function isArtifactCacheFreshForBuild(artifactDir: string): FreshnessResult {
    const manifest = readExecutionManifest(artifactDir);
    if (!manifest?.target) {
        return {fresh: false, reason: 'missing execution manifest target'};
    }
    return isArtifactCacheFresh(artifactDir, manifest.target);
}

export function listWasmModuleExports(wasmPath: string): string[] {
    const bytes = readFileSync(wasmPath);
    const module = new WebAssembly.Module(bytes);
    return WebAssembly.Module.exports(module).map((item) => item.name);
}

function findFileRecursive(dir: string, predicate: (name: string) => boolean): string | null {
    if (!existsSync(dir)) {
        return null;
    }
    for (const entry of readdirSync(dir, {withFileTypes: true})) {
        const full = join(dir, entry.name);
        if (entry.isFile() && predicate(entry.name)) {
            return full;
        }
        if (entry.isDirectory()) {
            const nested = findFileRecursive(full, predicate);
            if (nested) {
                return nested;
            }
        }
    }
    return null;
}

export function resolveNodeEntry(artifactDir: string): string | null {
    return findFileRecursive(artifactDir, (name) => name.endsWith('.mjs') || name.endsWith('.js'));
}

export function resolveNyarEntry(artifactDir: string): {
    nyarPath: string;
    physicalEntry: string;
    logicalEntry: string;
} | null {
    const nyarPath = findFileRecursive(artifactDir, (name) => name.endsWith('.nyar'));
    if (!nyarPath) {
        return null;
    }
    const view = loadNative().legionReadExecutionManifest?.(artifactDir);
    return {
        nyarPath,
        physicalEntry: view?.primaryPhysicalEntry ?? 'main',
        logicalEntry: view?.primaryLogicalEntry ?? 'main',
    };
}

