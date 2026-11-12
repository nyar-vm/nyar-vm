import {existsSync, readFileSync, readdirSync} from 'node:fs';
import {join} from 'node:path';

import {loadNative, type LegionArtifactFreshness} from '../native.ts';

export const NODE_WASM_TARGET = 'wasm32-node-unknown-wasm';

/** OVM/Nyar 产物 manifest 里的 target 字段以 canonical triple 为准；freshness 以 manifest 为准。 */
export const OVM_BUILD_TARGET = 'ovm';

/** @deprecated 矩阵历史名；新鲜度请用 {@link isArtifactCacheFreshForBuild}。 */
export const NYAR_VM_TARGET = OVM_BUILD_TARGET;

export type FreshnessResult = {fresh: boolean; reason?: string};

export function resolveArtifactDir(buildOutputDir: string, _target: string): string {
    return buildOutputDir;
}

/**
 * 读取产物 execution manifest 摘要（pinned runner 消费合同）。
 *
 * `compiler_build_identity` 由 Rust 侧从 manifest `toolchain_fingerprint` 的
 * `compiler=` 段派生：与 {@link legionCompilerBuildIdentity} 交叉核对即可判定
 * 产物是否出自当前编译器构建。`undefined` 表示旧构建产物（指纹缺段）或 manifest
 * 不可读，消费者不得复用，也不得伪造身份放行。
 */
export function readExecutionManifest(artifactDir: string): {
    schema_version: number;
    target: string;
    compiler_build_identity?: string;
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
        compiler_build_identity: view.compilerBuildIdentity ?? undefined,
        compile_evidence: {
            semantic_closure_hash: view.semanticClosureHash ?? undefined,
            canonical_surface_hash: view.canonicalSurfaceHash ?? undefined,
            host_provider_binding_hash: view.hostProviderBindingHash ?? undefined,
        },
    };
}

/**
 * native 权威校验结果 → 消费者判定。唯一放行条件是 `fully-verified`：
 * 产物自洽（缺当前 workspace 输入上下文）与 stale 一律拒绝复用；
 * validator 结果缺失同样拒绝，不得降级成“target 相同 + hash 非空即 fresh”。
 */
export function freshnessFromNativeOutcome(outcome: LegionArtifactFreshness | null | undefined): FreshnessResult {
    if (!outcome) {
        return {fresh: false, reason: 'native validator legionCheckArtifactFresh unavailable; refusing artifact reuse'};
    }
    if (outcome.level !== 'fully-verified' || !outcome.fresh) {
        return {
            fresh: false,
            reason: outcome.reason ?? `native freshness verdict \`${outcome.level}\` is not fully-verified; refusing artifact reuse`,
        };
    }
    return {fresh: true};
}

/**
 * 权威 freshness 校验入口（pinned runner 消费合同）。
 *
 * `projectDir` 是正式复用的必需上下文：Rust 权威核心据此重算 workspace 输入摘要
 * 与 compile evidence，只有 `fully-verified` 才返回 fresh。不传 `projectDir` 时
 * 核心最高只到 `artifact-self-consistent`，本函数一律拒绝——消费者不得省略项目
 * 上下文求宽松判定，也不得在本层加任何降级放行。
 */
export function isArtifactCacheFresh(artifactDir: string, expectedTarget: string, projectDir?: string): FreshnessResult {
    let native: ReturnType<typeof loadNative>;
    try {
        native = loadNative();
    } catch (error) {
        return {fresh: false, reason: `native addon unavailable: ${error instanceof Error ? error.message : String(error)}`};
    }
    const validator = native.legionCheckArtifactFresh;
    if (!validator) {
        return {fresh: false, reason: 'native validator legionCheckArtifactFresh unavailable; refusing artifact reuse'};
    }
    return freshnessFromNativeOutcome(validator.call(native, artifactDir, expectedTarget, projectDir));
}

/** 用 manifest 内 target 做新鲜度校验（避免硬编码 canonical triple）。 */
export function isArtifactCacheFreshForBuild(artifactDir: string, projectDir?: string): FreshnessResult {
    const manifest = readExecutionManifest(artifactDir);
    if (!manifest?.target) {
        return {fresh: false, reason: 'missing execution manifest target'};
    }
    return isArtifactCacheFresh(artifactDir, manifest.target, projectDir);
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

/** 解析 package 产物里的 wasm artifact；与 `resolveNodeEntry` 同属装配层产物布局事实。 */
export function resolveWasmEntry(artifactDir: string): string | null {
    return findFileRecursive(artifactDir, (name) => name.endsWith('.wasm'));
}

/** Node 轨 package 产物入口：node glue 与 wasm artifact 必须同时存在。 */
export type NodeArtifactEntry = {
    legionMjs: string;
    legionWasm: string;
    physicalEntry: string;
};

/**
 * 解析 Node 轨产物入口。wasm 是 ABI schema（`nyar.library_invoke`）的载体，
 * node glue 是调用宿主；两者缺一即不是可 invoke 的 package 产物，不猜测路径。
 * physical entry 以执行 manifest 为准，缺失时回退 `main`（与 {@link resolveNyarEntry} 一致）。
 */
export function resolveArtifactEntry(artifactDir: string): NodeArtifactEntry | null {
    const legionMjs = resolveNodeEntry(artifactDir);
    const legionWasm = resolveWasmEntry(artifactDir);
    if (!legionMjs || !legionWasm) {
        return null;
    }
    const view = loadNative().legionReadExecutionManifest?.(artifactDir);
    return {legionMjs, legionWasm, physicalEntry: view?.primaryPhysicalEntry ?? 'main'};
}

export function resolveNyarEntry(artifactDir: string): {
    nyarPath: string;
    physicalEntry: string;
    logicalEntry: string;
} | null {
    const nyarPath = findFileRecursive(artifactDir, (name) => name.endsWith('.odin'));
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

