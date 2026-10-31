import type {LocaleCatalog} from '@vmz/commander';

/** Legion CLI catalog（产品层文案）。 */
export const LEGION_CLI_CATALOG: LocaleCatalog = {
    'cli.intro': 'Legion build driver — plan, build, and test Valkyrie projects',
    'cli.cmd.version': 'Print @nyar/valkyrie version',
    'cli.cmd.plan': 'Resolve workspace dependencies and print a build plan',
    'cli.cmd.build': 'Compile a Legion project for a target',
    'cli.cmd.test': 'Run Legion tests for a target (structured N-API)',
    'cli.opt.target': 'Canonical target (node, ovm, …)',
    'cli.opt.output': 'Build output directory',
    'cli.arg.project': 'Project directory (default: cwd)',
};

/** Asgard CLI catalog（help/version；命令编排由 TS 层诊断，不透传 argv）。 */
export const ASGARD_CLI_CATALOG: LocaleCatalog = {
    'cli.intro': 'Asgard VOA toolchain — static delivery via `build --static <entry>`; other commands await config evaluation',
    'cli.cmd.version': 'Print @nyar/valkyrie version',
};
