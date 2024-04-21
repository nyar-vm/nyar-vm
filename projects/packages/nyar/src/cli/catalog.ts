import type { LocaleCatalog } from '@nyar/commander';

/** Nyar CLI 英文 catalog（产品层；`commander.*` 由框架 fallback 提供）。 */
export const NYAR_CLI_CATALOG: LocaleCatalog = {
    'cli.intro': 'Nyar VM host CLI — runtime dispatch and collect diagnostics',
    'cli.cmd.version': 'Print package version and host protocol',
    'cli.cmd.host-info': 'Print resolved platform collect and native host metadata',
    'cli.cmd.runtime': 'Runtime family helpers',
    'cli.cmd.runtime.list': 'List supported runtime families',
    'cli.cmd.runtime.prepare': 'Expand a runtime template without executing',
    'cli.cmd.runtime.run': 'Prepare and execute a runtime command',
    'cli.opt.family': 'Runtime family id (clr, jvm, node, wasi, nyar-vm, …)',
    'cli.opt.artifact': 'Artifact path (binary, jar, wasm module, …)',
    'cli.opt.classpath': 'Classpath for JVM family',
    'cli.opt.entry': 'Logical entry symbol',
    'cli.opt.wasi-p3': 'Enable wasmtime -S p3 for WASI family',
    'cli.opt.cwd': 'Working directory for runtime run',
};
