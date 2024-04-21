/**
 * Filesystem locales loading for `@nyar/commander`.
 *
 * Generic: any product passes a `localesRoot`. This package does **not** ship
 * product language packs —only tiny English fallbacks for `commander.*` chrome/err ids.
 */

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import type { LocaleCatalog, LocalizePlugin } from './types.ts';

export type LocalesManifest = {
    defaultLocale: string;
    locales: Array<{ id: string; label?: string; direction?: string }>;
    fallback?: Record<string, string[]>;
};

/** Framework chrome / errors —products may override the same ids in their locales/. */
export const COMMANDER_FALLBACK_EN_US: LocaleCatalog = {
    'commander.ui.usage': 'Usage: {name} <command> [options]',
    'commander.ui.commands': 'Commands:',
    'commander.ui.options': 'Options:',
    'commander.opt.locale': 'Locale id',
    'commander.err.unknown_command': 'unknown command `{cmd}`',
    'commander.err.unknown_option': 'unknown option `{option}`',
    'commander.err.missing_option_value': 'missing value for `{option}`',
    'commander.err.localize_required': 'call .use(LocalizePlugin) or .locales(root) before parse()',
};

const catalogCache = new Map<string, LocaleCatalog>();
const manifestCache = new Map<string, LocalesManifest>();

export function loadLocalesManifest(root: string): LocalesManifest {
    const cached = manifestCache.get(root);
    if (cached) return cached;
    const file = path.join(root, 'locales.json');
    if (!existsSync(file)) {
        throw new Error(`@nyar/commander: missing locales manifest at ${file}`);
    }
    const raw = JSON.parse(readFileSync(file, 'utf8')) as LocalesManifest;
    if (!raw?.defaultLocale || !Array.isArray(raw.locales) || !raw.locales.length) {
        throw new Error(`@nyar/commander: invalid locales manifest ${file}`);
    }
    manifestCache.set(root, raw);
    return raw;
}

/**
 * Flatten nested JSON (`{ a: { b: "x" } }` 鈫?`a.b`) or pass through flat catalogs.
 */
export function flattenCatalog(node: unknown, prefix = ''): LocaleCatalog {
    const out: LocaleCatalog = {};
    if (node == null || typeof node !== 'object' || Array.isArray(node)) return out;
    for (const [key, value] of Object.entries(node as Record<string, unknown>)) {
        const id = prefix ? `${prefix}.${key}` : key;
        if (typeof value === 'string') {
            out[id] = value;
        } else if (value && typeof value === 'object' && !Array.isArray(value)) {
            Object.assign(out, flattenCatalog(value, id));
        }
    }
    return out;
}

function loadLocaleDir(root: string, localeId: string): LocaleCatalog {
    const dir = path.join(root, localeId);
    if (!existsSync(dir)) return {};
    const out: LocaleCatalog = {};
    for (const name of readdirSync(dir).sort()) {
        if (!name.endsWith('.json')) continue;
        const raw = JSON.parse(readFileSync(path.join(dir, name), 'utf8')) as unknown;
        Object.assign(out, flattenCatalog(raw));
    }
    return out;
}

/**
 * Load (and cache) a locale catalog. Missing locales fall back via manifest.fallback
 * then `defaultLocale`.
 */
export function loadCatalog(locale: string, root: string): LocaleCatalog {
    const cacheKey = `${root}::${locale}`;
    const hit = catalogCache.get(cacheKey);
    if (hit) return hit;

    const manifest = loadLocalesManifest(root);
    const chain = [locale, ...(manifest.fallback?.[locale] ?? []), manifest.defaultLocale];
    const merged: LocaleCatalog = {};
    for (const id of [...new Set(chain)].reverse()) {
        Object.assign(merged, loadLocaleDir(root, id));
    }
    if (!Object.keys(merged).length) {
        throw new Error(`@nyar/commander: empty catalog for locale ${JSON.stringify(locale)} under ${root}`);
    }
    catalogCache.set(cacheKey, merged);
    return merged;
}

/** Clear caches (tests). */
export function clearLocalesCache(): void {
    catalogCache.clear();
    manifestCache.clear();
}

export function translate(id: string, args: Record<string, string> | undefined, catalog: LocaleCatalog): string {
    const template = catalog[id];
    if (template == null) return `{{${id}}}`;
    return template.replace(/\{([a-zA-Z0-9_.-]+)\}/g, (_m, name: string) => {
        if (args && Object.prototype.hasOwnProperty.call(args, name)) {
            return args[name] ?? '';
        }
        return `{${name}}`;
    });
}

/**
 * Resolve locale from `--locale` / env / manifest.
 */
export function resolveLocale(opts: {
    argv?: string[];
    env?: NodeJS.ProcessEnv;
    manifest: LocalesManifest;
    /** Env keys checked in order (default: LOCALE, LANG, LC_ALL). */
    envKeys?: string[];
}): string {
    const env = opts.env ?? process.env;
    const argv = opts.argv ?? [];
    let fromFlag = '';
    for (let i = 0; i < argv.length; i++) {
        const a = argv[i]!;
        if (a === '--locale' && argv[i + 1] && !argv[i + 1]!.startsWith('-')) {
            fromFlag = argv[i + 1]!;
            break;
        }
        if (a.startsWith('--locale=')) {
            fromFlag = a.slice('--locale='.length);
            break;
        }
    }
    const keys = opts.envKeys ?? ['LOCALE', 'LANG', 'LC_ALL'];
    let fromEnv = '';
    for (const k of keys) {
        const v = env[k];
        if (v) {
            fromEnv = String(v).split('.')[0]?.replace(/_/g, '-') || '';
            if (fromEnv) break;
        }
    }
    const raw = fromFlag || fromEnv;
    if (!raw) return opts.manifest.defaultLocale;
    const lower = raw.toLowerCase();
    const known = opts.manifest.locales.map((l) => l.id);
    const exact = known.find((id) => id.toLowerCase() === lower);
    if (exact) return exact;
    const lang = lower.split('-')[0]!;
    const prefix = known.find((id) => id.toLowerCase().startsWith(lang));
    if (prefix) return prefix;
    return opts.manifest.defaultLocale;
}

/**
 * Catalog lookup with commander framework English fallbacks.
 */
export function translateWithFallback(id: string, args: Record<string, string> | undefined, catalog: LocaleCatalog): string {
    if (Object.prototype.hasOwnProperty.call(catalog, id)) {
        return translate(id, args, catalog);
    }
    if (Object.prototype.hasOwnProperty.call(COMMANDER_FALLBACK_EN_US, id)) {
        return translate(id, args, COMMANDER_FALLBACK_EN_US);
    }
    return `{{${id}}}`;
}

export type CreateLocalizeFromLocalesOptions = {
    root: string;
    locale?: string;
    env?: NodeJS.ProcessEnv;
    argv?: string[];
    /** Extra env keys prepended (e.g. `VMZ_LOCALE` for products). */
    envKeys?: string[];
    catalog?: LocaleCatalog;
};

/**
 * Build a LocalizePlugin from a locales directory.
 */
export function createLocalizeFromLocales(opts: CreateLocalizeFromLocalesOptions): LocalizePlugin {
    const env = opts.env ?? process.env;
    const argv = opts.argv ?? [];
    const manifest = loadLocalesManifest(opts.root);
    const locale = typeof opts.locale === 'string' && opts.locale ? opts.locale : resolveLocale({ argv, env, manifest, envKeys: opts.envKeys });
    const catalog = opts.catalog ?? loadCatalog(locale, opts.root);
    return {
        resolveLocale: () => locale,
        t: (id, args) => translateWithFallback(id, args, catalog),
    };
}
