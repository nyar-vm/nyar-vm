import {performance} from 'node:perf_hooks';

import {legionBuildProject, legionNativeReady, legionNativeSkipReason} from './host.ts';
import type {LegionSpawnResult} from './types.ts';

export const WASM_NODE_BENCH_TARGET = 'node';

export function median(values: number[]): number | null {
    if (values.length === 0) {
        return null;
    }
    const sorted = [...values].sort((a, b) => a - b);
    const mid = Math.floor(sorted.length / 2);
    if (sorted.length % 2 === 0) {
        return (sorted[mid - 1] + sorted[mid]) / 2;
    }
    return sorted[mid];
}

export type LegionBenchRow = {label: string; ms: number};

export function parseLegionBenchTable(_stdout: string): LegionBenchRow[] {
    return [];
}

export function formatLegionCliError(label: string, outcome: LegionSpawnResult): string {
    const detail = outcome.stderr || outcome.stdout || `exit ${outcome.status}`;
    return `${label} failed (${outcome.route}): ${detail}`;
}

export function createLegionBenchmarkRunner() {
    return {
        ready(): boolean {
            return legionNativeReady();
        },
        skipReason(): string | null {
            return legionNativeSkipReason();
        },
        benchProject(
            projectDir: string,
            opts: {runs: number; target: string},
        ): {outcome: LegionSpawnResult; rows: LegionBenchRow[]} {
            const samples: number[] = [];
            let lastOutcome: LegionSpawnResult = {
                status: 1,
                stdout: '',
                stderr: 'no runs',
                route: 'napi:legionBuild',
            };
            const outputDir = `${projectDir}/.legion-bench-out`;
            for (let run = 0; run < opts.runs; run += 1) {
                const started = performance.now();
                const outcome = legionBuildProject(projectDir, opts.target, outputDir);
                lastOutcome = outcome;
                if (outcome.status !== 0) {
                    break;
                }
                samples.push(performance.now() - started);
            }
            const rows = samples.length > 0 ? [{label: 'compile', ms: median(samples) ?? samples[0]}] : [];
            return {outcome: lastOutcome, rows};
        },
    };
}
