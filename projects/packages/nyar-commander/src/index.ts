/**
 * `@nyar/commander` —i18n-first TypeScript CLI framework.
 *
 * Command / option second args are message ids. Natural language comes from a
 * pluggable Localize plugin or `.locales(root)`. Framework chrome/err ids are
 * `commander.*` with tiny English fallbacks —no product language packs here.
 */

export type { CatalogLoader, LocaleCatalog, LocalizePlugin } from './types.ts';

export {
    COMMANDER_FALLBACK_EN_US,
    clearLocalesCache,
    createLocalizeFromLocales,
    flattenCatalog,
    loadCatalog,
    loadLocalesManifest,
    resolveLocale,
    translate,
    translateWithFallback,
} from './locales.ts';
export type { CreateLocalizeFromLocalesOptions, LocalesManifest } from './locales.ts';

import type { CatalogLoader, LocaleCatalog, LocalizePlugin } from './types.ts';
import { COMMANDER_FALLBACK_EN_US, createLocalizeFromLocales, translate, translateWithFallback } from './locales.ts';

/** Options bag passed to actions (`_` = positionals). */
export type ParsedOptions = Record<string, string | boolean | string[]> & { _: string[] };

/** Command action after argv is parsed. */
export type ActionHandler = (options: ParsedOptions, ...args: string[]) => void | number | Promise<void | number>;

/** Registered option (structure only). */
export type OptionDef = {
    rawName: string;
    helpId: string;
    /** Canonical key in options bag (e.g. `out-dir`). */
    key: string;
    /** Long/short tokens without leading dashes (e.g. `out-dir`, `o`). */
    aliases: string[];
    /** True when the option takes a value (`<x>` / `[x]`). */
    takesValue: boolean;
    /** True when value is optional (`[x]`): bare flag 鈬?`true`. */
    optionalValue: boolean;
    /** True when the option may repeat (`--dirty <path>...` 鈬?`string[]`). */
    repeatable: boolean;
};

/** Registered command node. */
export type CommandDef = {
    rawName: string;
    helpId: string;
    /** Match tokens from `rawName` split on `|`. */
    names: string[];
    options: OptionDef[];
    action?: ActionHandler;
    children: CommandDef[];
    /** If true, do not parse options; remaining argv is positionals / raw rest. */
    passthrough: boolean;
};

export interface Command {
    option(rawName: string, helpId: string): this;
    action(handler: ActionHandler): this;
    command(rawName: string, helpId: string): Command;
    /** Leave remaining argv unparsed (for nested domain CLIs). */
    passthrough(): this;
}

export interface Cli {
    use(plugin: LocalizePlugin): this;
    catalog(loader: CatalogLoader): this;
    /**
     * Load catalogs from a locales directory (`locales.json` + `<locale>/*.json`).
     * Later `.use` overrides. Rebuilds at `parse` so `--locale` / env apply.
     */
    locales(root: string, opts?: { envKeys?: string[] }): this;
    /** Global option (parsed before the command; merged into action options). */
    option(rawName: string, helpId: string): this;
    /**
     * Optional short intro line(s) before derived command/option lists.
     * Do **not** paste full usage here —help is derived from the command tree.
     */
    intro(introId: string): this;
    /**
     * @deprecated Use {@link intro}. Kept as alias so older call sites compile.
     */
    help(helpId: string): this;
    command(rawName: string, helpId: string): Command;
    parse(argv?: string[]): Promise<number>;
}

class CommandBuilder implements Command {
    readonly def: CommandDef;

    constructor(rawName: string, helpId: string) {
        assertHelpId(helpId, 'command');
        this.def = {
            rawName,
            helpId,
            names: splitNames(rawName),
            options: [],
            children: [],
            passthrough: false,
        };
    }

    option(rawName: string, helpId: string): this {
        assertHelpId(helpId, 'option');
        this.def.options.push(parseOptionDef(rawName, helpId));
        return this;
    }

    action(handler: ActionHandler): this {
        this.def.action = handler;
        return this;
    }

    passthrough(): this {
        this.def.passthrough = true;
        return this;
    }

    command(rawName: string, helpId: string): Command {
        const child = new CommandBuilder(rawName, helpId);
        this.def.children.push(child.def);
        return child;
    }
}

class CliBuilder implements Cli {
    readonly name: string;
    private localize: LocalizePlugin | null = null;
    /** When set, `parse` rebuilds localize from this root (argv/env/`--locale`). */
    private localesRoot: string | null = null;
    private localesEnvKeys: string[] | undefined;
    private introId: string | null = null;
    private readonly roots: CommandBuilder[] = [];
    private readonly rootOptions: OptionDef[] = [];

    constructor(name: string) {
        this.name = name;
    }

    use(plugin: LocalizePlugin): this {
        if (!plugin || typeof plugin.t !== 'function') {
            throw new Error('@nyar/commander: LocalizePlugin.t is required');
        }
        this.localesRoot = null;
        this.localesEnvKeys = undefined;
        this.localize = wrapWithCommanderFallback(plugin);
        return this;
    }

    catalog(loader: CatalogLoader): this {
        if (typeof loader !== 'function') {
            throw new Error('@nyar/commander: catalog(loader) requires a function');
        }
        return this.use({
            t: (id, args) => {
                const table = loader('en-US');
                if (table && typeof (table as Promise<LocaleCatalog>).then === 'function') {
                    throw new Error('@nyar/commander: async CatalogLoader via .catalog() is not supported; use .use({ t })');
                }
                return translateWithFallback(id, args, table as LocaleCatalog);
            },
        });
    }

    locales(root: string, opts: { envKeys?: string[] } = {}): this {
        if (!root || typeof root !== 'string') {
            throw new Error('@nyar/commander: locales(root) requires a non-empty path');
        }
        this.localesRoot = root;
        this.localesEnvKeys = opts.envKeys;
        if (!this.rootOptions.some((o) => o.key === 'locale')) {
            this.rootOptions.push(parseOptionDef('--locale <id>', 'commander.opt.locale'));
        }
        // Eager plugin so missing-manifest fails early; parse rebuilds with argv.
        this.localize = wrapWithCommanderFallback(createLocalizeFromLocales({ root, envKeys: opts.envKeys }));
        return this;
    }

    option(rawName: string, helpId: string): this {
        assertHelpId(helpId, 'option');
        this.rootOptions.push(parseOptionDef(rawName, helpId));
        return this;
    }

    intro(introId: string): this {
        assertHelpId(introId, 'intro');
        this.introId = introId;
        return this;
    }

    help(helpId: string): this {
        return this.intro(helpId);
    }

    command(rawName: string, helpId: string): Command {
        const cmd = new CommandBuilder(rawName, helpId);
        this.roots.push(cmd);
        return cmd;
    }

    /** Help ids registered on this CLI (for {@link assertCatalogCoverage}). */
    collectHelpIds(): string[] {
        const ids: string[] = [];
        if (this.introId) ids.push(this.introId);
        for (const o of this.rootOptions) ids.push(o.helpId);
        const walk = (def: CommandDef) => {
            ids.push(def.helpId);
            for (const o of def.options) ids.push(o.helpId);
            for (const c of def.children) walk(c);
        };
        for (const r of this.roots) walk(r.def);
        return [...new Set(ids)];
    }

    async parse(argvInput: string[] = process.argv): Promise<number> {
        const argv0 = normalizeArgv(argvInput);
        const { options: globalOpts, rest: argv } = peelKnownOptions(argv0, this.rootOptions);

        let localize = this.localize;
        if (this.localesRoot) {
            const localeFlag = typeof globalOpts.locale === 'string' && globalOpts.locale ? globalOpts.locale : undefined;
            localize = wrapWithCommanderFallback(
                createLocalizeFromLocales({
                    root: this.localesRoot,
                    locale: localeFlag,
                    argv: argv0,
                    env: process.env,
                    envKeys: this.localesEnvKeys,
                }),
            );
        }
        if (!localize) {
            throw new Error(translate('commander.err.localize_required', undefined, COMMANDER_FALLBACK_EN_US));
        }
        const t = localize.t.bind(localize);

        if (argv.length === 0 || isHelpToken(argv[0]!)) {
            console.log(this.formatRootHelp(t));
            return 0;
        }

        const cmdToken = argv[0]!;
        const matched = this.roots.find((c) => c.def.names.includes(cmdToken));
        if (!matched) {
            console.error(t('commander.err.unknown_command', { cmd: cmdToken }));
            console.log(this.formatRootHelp(t));
            return 1;
        }

        return await this.dispatch(matched.def, argv.slice(1), t, [cmdToken], globalOpts);
    }

    /** Derived root help: optional intro + commands/options from the registration tree. */
    formatRootHelp(t: LocalizePlugin['t']): string {
        const lines: string[] = [];
        if (this.introId) {
            lines.push(t(this.introId), '');
        }
        lines.push(t('commander.ui.usage', { name: this.name }), '', t('commander.ui.commands'));
        for (const c of this.roots) {
            lines.push(`  ${c.def.rawName.padEnd(28)} ${t(c.def.helpId)}`);
        }
        const opts = [
            ...this.rootOptions,
            ...collectRootOptions(this.roots.map((c) => c.def)).filter((o) => !this.rootOptions.some((r) => r.key === o.key)),
        ];
        if (opts.length) {
            lines.push('', t('commander.ui.options'));
            for (const o of opts) {
                lines.push(`  ${o.rawName.padEnd(28)} ${t(o.helpId)}`);
            }
        }
        return lines.join('\n');
    }

    private async dispatch(
        def: CommandDef,
        rest: string[],
        t: LocalizePlugin['t'],
        path: string[],
        globalOpts: ParsedOptions,
    ): Promise<number> {
        if (rest.length && isHelpToken(rest[0]!)) {
            console.log(formatCommandHelp(this.name, def, t, path, this.rootOptions));
            return 0;
        }

        if (def.children.length && rest.length && !rest[0]!.startsWith('-')) {
            const childTok = rest[0]!;
            const child = def.children.find((c) => c.names.includes(childTok));
            if (child) {
                return await this.dispatch(child, rest.slice(1), t, [...path, childTok], globalOpts);
            }
            if (!def.action) {
                console.error(t('commander.err.unknown_command', { cmd: [...path, childTok].join(' ') }));
                console.log(formatCommandHelp(this.name, def, t, path, this.rootOptions));
                return 1;
            }
        }

        if (!def.action) {
            console.log(formatCommandHelp(this.name, def, t, path, this.rootOptions));
            return rest.length ? 1 : 0;
        }

        let options: ParsedOptions;
        try {
            options = def.passthrough ? { _: rest.slice() } : parseOptions(rest, def.options);
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            if (msg.startsWith('unknown_option:')) {
                console.error(t('commander.err.unknown_option', { option: msg.slice('unknown_option:'.length) }));
            } else if (msg.startsWith('missing_value:')) {
                console.error(
                    t('commander.err.missing_option_value', {
                        option: msg.slice('missing_value:'.length),
                    }),
                );
            } else {
                console.error(msg);
            }
            console.log(formatCommandHelp(this.name, def, t, path, this.rootOptions));
            return 1;
        }

        for (const [k, v] of Object.entries(globalOpts)) {
            if (k === '_') continue;
            if (options[k] === undefined) options[k] = v as string | boolean | string[];
        }

        const result = await def.action(options, ...options._);
        if (typeof result === 'number') return result;
        return 0;
    }
}

/** Derive command help from one node (children + options). */
export function formatCommandHelp(
    cliName: string,
    def: CommandDef,
    t: LocalizePlugin['t'],
    path: string[],
    rootOptions: OptionDef[] = [],
): string {
    const lines = [`${cliName} ${path.join(' ')} —${t(def.helpId)}`, ''];
    if (def.children.length) {
        lines.push(t('commander.ui.commands'));
        for (const c of def.children) {
            lines.push(`  ${c.rawName.padEnd(28)} ${t(c.helpId)}`);
        }
        lines.push('');
    }
    const opts = [...rootOptions, ...def.options.filter((o) => !rootOptions.some((r) => r.key === o.key))];
    if (opts.length) {
        lines.push(t('commander.ui.options'));
        for (const o of opts) {
            lines.push(`  ${o.rawName.padEnd(28)} ${t(o.helpId)}`);
        }
    }
    return lines.join('\n').trimEnd();
}

/** Union options declared on root commands (dedupe by key, stable first-seen order). */
function collectRootOptions(roots: CommandDef[]): OptionDef[] {
    const seen = new Set<string>();
    const out: OptionDef[] = [];
    for (const root of roots) {
        for (const o of root.options) {
            if (seen.has(o.key)) continue;
            seen.add(o.key);
            out.push(o);
        }
    }
    return out;
}

/**
 * Create a CLI named `name` (shown in usage).
 */
export function createCli(name: string): Cli {
    if (!name || typeof name !== 'string') {
        throw new Error('@nyar/commander: createCli(name) requires a non-empty program name');
    }
    return new CliBuilder(name);
}

/**
 * Dev/CI: every registered helpId must exist in `catalog` or commander English fallbacks.
 */
export function assertCatalogCoverage(cli: Cli, catalog: LocaleCatalog): void {
    const ids = typeof (cli as CliBuilder).collectHelpIds === 'function' ? (cli as CliBuilder).collectHelpIds() : [];
    const missing = ids.filter(
        (id) => !Object.prototype.hasOwnProperty.call(catalog, id) && !Object.prototype.hasOwnProperty.call(COMMANDER_FALLBACK_EN_US, id),
    );
    if (missing.length) {
        throw new Error(`@nyar/commander: catalog missing help ids:\n  ${missing.sort().join('\n  ')}`);
    }
}

function wrapWithCommanderFallback(plugin: LocalizePlugin): LocalizePlugin {
    return {
        resolveLocale: plugin.resolveLocale?.bind(plugin),
        t: (id, args) => {
            const fromPlugin = plugin.t(id, args);
            if (fromPlugin !== `{{${id}}}`) return fromPlugin;
            if (Object.prototype.hasOwnProperty.call(COMMANDER_FALLBACK_EN_US, id)) {
                return translate(id, args, COMMANDER_FALLBACK_EN_US);
            }
            return fromPlugin;
        },
    };
}

/**
 * Consume known root options; leave unknown flags and positionals in `rest`
 * (unlike {@link parseOptions}, which throws on unknown options).
 */
export function peelKnownOptions(argv: string[], optionDefs: OptionDef[]): { options: ParsedOptions; rest: string[] } {
    const byAlias = new Map<string, OptionDef>();
    for (const def of optionDefs) {
        for (const a of def.aliases) byAlias.set(a, def);
    }
    const options: ParsedOptions = { _: [] };
    const rest: string[] = [];
    for (let i = 0; i < argv.length; i++) {
        const a = argv[i]!;
        if (a === '--') {
            rest.push(...argv.slice(i));
            break;
        }
        if (a.startsWith('--')) {
            const eq = a.indexOf('=');
            const long = eq === -1 ? a.slice(2) : a.slice(2, eq);
            const def = byAlias.get(long);
            if (!def) {
                rest.push(a);
                continue;
            }
            if (def.takesValue) {
                if (eq !== -1) {
                    assignOption(options, def, a.slice(eq + 1));
                } else {
                    const next = argv[i + 1];
                    if (next != null && !next.startsWith('-')) {
                        assignOption(options, def, next);
                        i += 1;
                    } else if (def.optionalValue) {
                        assignOption(options, def, true);
                    } else {
                        throw new Error(`missing_value:--${long}`);
                    }
                }
            } else {
                assignOption(options, def, true);
            }
            continue;
        }
        if (a.startsWith('-') && a.length === 2) {
            const short = a.slice(1);
            const def = byAlias.get(short);
            if (!def) {
                rest.push(a);
                continue;
            }
            if (def.takesValue) {
                const next = argv[i + 1];
                if (next != null && !next.startsWith('-')) {
                    assignOption(options, def, next);
                    i += 1;
                } else if (def.optionalValue) {
                    assignOption(options, def, true);
                } else {
                    throw new Error(`missing_value:-${short}`);
                }
            } else {
                assignOption(options, def, true);
            }
            continue;
        }
        rest.push(a);
    }
    return { options, rest };
}

/** Strip `node` + script when callers pass full `process.argv`. */
export function normalizeArgv(argv: string[]): string[] {
    if (argv.length >= 2 && looksLikeNode(argv[0]!) && looksLikeScript(argv[1]!)) {
        return argv.slice(2);
    }
    return argv.slice();
}

function looksLikeNode(token: string): boolean {
    const base = token.replace(/\\/g, '/').split('/').pop() || '';
    return base === 'node' || base === 'node.exe' || base.startsWith('node');
}

function looksLikeScript(token: string): boolean {
    return /\.(c?js|mjs|ts)$/i.test(token) || token.includes(`${'node_modules'}`) || token.endsWith('vmz') || token.endsWith('nyar');
}

function isHelpToken(token: string): boolean {
    return token === 'help' || token === '-h' || token === '--help';
}

function splitNames(rawName: string): string[] {
    return rawName
        .split('|')
        .map((s) => s.trim())
        .filter(Boolean);
}

function assertHelpId(helpId: string, kind: string): void {
    if (!helpId || typeof helpId !== 'string' || !helpId.trim()) {
        throw new Error(`@nyar/commander: ${kind} helpId must be a non-empty catalog key`);
    }
    if (/\s/.test(helpId)) {
        throw new Error(`@nyar/commander: ${kind} helpId must be a catalog key (no spaces); got ${JSON.stringify(helpId)}`);
    }
}

/** Parse `--out-dir <dir>` / `-o, --out-dir <dir>` / `--dirty <path>...` into an OptionDef. */
export function parseOptionDef(rawName: string, helpId: string): OptionDef {
    const repeatable = /\.\.\.\s*$/.test(rawName) || /\.\.\.>/.test(rawName) || /\.\.\.]/.test(rawName);
    const optionalValue = /\[[^\]]+\]/.test(rawName);
    const takesValue = optionalValue || /<[^>]+>/.test(rawName) || repeatable;
    const cleaned = rawName
        .replace(/\.\.\./g, '')
        .replace(/<[^>]+>|\[[^\]]+\]/g, '')
        .trim();
    const parts = cleaned
        .split(/[,\s]+/)
        .map((p) => p.trim())
        .filter(Boolean);
    const aliases: string[] = [];
    for (const p of parts) {
        if (p.startsWith('--')) aliases.push(p.slice(2));
        else if (p.startsWith('-') && p.length > 1) aliases.push(p.slice(1));
    }
    if (!aliases.length) {
        throw new Error(`@nyar/commander: invalid option rawName ${JSON.stringify(rawName)}`);
    }
    const key = aliases.find((a) => a.length > 1) ?? aliases[0]!;
    return { rawName, helpId, key, aliases, takesValue, optionalValue, repeatable };
}

/**
 * Parse argv against registered options. Throws `unknown_option:鈥 / `missing_value:鈥.
 */
export function parseOptions(argv: string[], optionDefs: OptionDef[]): ParsedOptions {
    const byAlias = new Map<string, OptionDef>();
    for (const def of optionDefs) {
        for (const a of def.aliases) byAlias.set(a, def);
    }
    const out: ParsedOptions = { _: [] };
    for (let i = 0; i < argv.length; i++) {
        const a = argv[i]!;
        if (a === '--') {
            out._.push(...argv.slice(i + 1));
            break;
        }
        if (a.startsWith('--')) {
            const eq = a.indexOf('=');
            const long = eq === -1 ? a.slice(2) : a.slice(2, eq);
            const def = byAlias.get(long);
            if (!def) throw new Error(`unknown_option:--${long}`);
            if (def.takesValue) {
                if (eq !== -1) {
                    assignOption(out, def, a.slice(eq + 1));
                } else {
                    const next = argv[i + 1];
                    if (next != null && !next.startsWith('-')) {
                        assignOption(out, def, next);
                        i += 1;
                    } else if (def.optionalValue) {
                        assignOption(out, def, true);
                    } else {
                        throw new Error(`missing_value:--${long}`);
                    }
                }
            } else {
                assignOption(out, def, true);
            }
            continue;
        }
        if (a.startsWith('-') && a.length === 2) {
            const short = a.slice(1);
            const def = byAlias.get(short);
            if (!def) throw new Error(`unknown_option:-${short}`);
            if (def.takesValue) {
                const next = argv[i + 1];
                if (next != null && !next.startsWith('-')) {
                    assignOption(out, def, next);
                    i += 1;
                } else if (def.optionalValue) {
                    assignOption(out, def, true);
                } else {
                    throw new Error(`missing_value:-${short}`);
                }
            } else {
                assignOption(out, def, true);
            }
            continue;
        }
        out._.push(a);
    }
    return out;
}

function assignOption(out: ParsedOptions, def: OptionDef, value: string | boolean): void {
    if (def.repeatable) {
        const cur = out[def.key];
        const list = Array.isArray(cur) ? cur : [];
        list.push(String(value));
        out[def.key] = list;
        return;
    }
    out[def.key] = value;
}
