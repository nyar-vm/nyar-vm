/**
 * Shared types for `@nyar/commander` (kept separate so locales.ts can import without cycles).
 */

/** Message id 鈫?template. Owned by the localize plugin / product, not this package. */
export type LocaleCatalog = Record<string, string>;

/** Load messages for one locale (sugar for building a {@link LocalizePlugin}). */
export type CatalogLoader = (locale: string) => LocaleCatalog | Promise<LocaleCatalog>;

/**
 * Pluggable localization. Products and end users supply their own `t` / locale policy.
 * This package never ships official product language packs.
 */
export type LocalizePlugin = {
    resolveLocale?: (ctx: { argv: string[]; env: NodeJS.ProcessEnv }) => string;
    t: (id: string, args?: Record<string, string>) => string;
};
