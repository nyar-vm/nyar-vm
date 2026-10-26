import {legionBuild, legionPlan, legionTest, loadNative, resolveNativePath, type LegionBuildOutcome} from '../native.ts';
import type {LegionSpawnResult} from './types.ts';

const ROUTE_BUILD = 'napi:legionBuild';
const ROUTE_TEST = 'napi:legionTest';

export function legionNativeReady(): boolean {
    try {
        loadNative();
        return true;
    } catch {
        return false;
    }
}

export function legionNativePath(): string | null {
    try {
        return resolveNativePath();
    } catch {
        return null;
    }
}

export function legionNativeSkipReason(): string | null {
    if (legionNativeReady()) {
        return null;
    }
    return 'Valkyrie N-API 未就绪：在 nyar-vm.rs/projects/nyar.rs 执行 cargo build -p valkyrie-napi';
}

export function legionBuildToSpawnResult(outcome: LegionBuildOutcome): LegionSpawnResult {
    return {
        status: outcome.ok ? 0 : 1,
        stdout: outcome.ok ? outcome.artifacts.join('\n') : '',
        stderr: outcome.error ?? '',
        route: ROUTE_BUILD,
    };
}

function parseTargetFlag(argv: string[]): string | undefined {
    for (let index = 0; index < argv.length; index += 1) {
        if (argv[index] === '--target' && argv[index + 1]) {
            return argv[index + 1];
        }
    }
    return undefined;
}

/** 解析 workspace 依赖并输出 build plan（不编译）。 */
export function legionPlanProject(projectDir: string, target?: string) {
    return legionPlan({projectDir, target});
}

/** 编译题解工程并落盘产物。 */
export function legionBuildProject(projectDir: string, target: string, outputDir: string): LegionSpawnResult {
    const outcome = legionBuild({projectDir, target, outputDir});
    return legionBuildToSpawnResult(outcome);
}

/** 运行 `legion test`（结构化 N-API）。 */
export function legionTestProject(projectDir: string, target?: string): LegionSpawnResult {
    const outcome = legionTest(projectDir, target);
    if (outcome.ok) {
        return {
            status: 0,
            stdout: `passed=${outcome.passed} failed=${outcome.failed} skipped=${outcome.skipped}`,
            stderr: '',
            route: ROUTE_TEST,
        };
    }
    return {
        status: 1,
        stdout: '',
        stderr: outcome.error ?? `legion test failed (${outcome.failed} failed)`,
        route: ROUTE_TEST,
    };
}

/** 兼容矩阵里 `spawnCli(['test', dir, '--target', …])` 形状。 */
export function legionSpawn(argv: string[]): LegionSpawnResult {
    const sub = argv[0];
    if (sub === 'test') {
        const projectDir = argv[1];
        if (!projectDir) {
            return {status: 2, stdout: '', stderr: 'legion test: missing project dir', route: ROUTE_TEST};
        }
        return legionTestProject(projectDir, parseTargetFlag(argv.slice(2)));
    }
    return {
        status: 1,
        stdout: '',
        stderr: `Legion 仅支持 plan/build/test N-API，不支持 argv 子命令: ${sub ?? '(empty)'}`,
        route: 'napi:unsupported',
    };
}

export function formatLegionSpawnError(label: string, outcome: LegionSpawnResult): string {
    const detail = outcome.stderr || outcome.stdout || `exit ${outcome.status}`;
    return `${label} failed (${outcome.route}): ${detail}`;
}
