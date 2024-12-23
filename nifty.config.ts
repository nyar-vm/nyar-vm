// Nifty project configuration for nyar-vm.rs (hybrid cargo + pnpm).
// Trusted Publisher contract: repo nyar-vm/nyar-vm, workflow publish-npm.yml, environment NPM_PUBLISH.
import { defineConfig } from "@doki-land/nifty";

export default defineConfig({
    format: {
        preset: "nifty",
        style: {
            indentStyle: "space",
            indentWidth: 4,
            lineWidth: 144,
            quoteStyle: "single",
        },
        includes: [
            "package.json",
            "pnpm-workspace.yaml",
            "nifty.config.ts",
            "scripts/**",
            "projects/nyar.ts/packages/**",
            "projects/nyar.ts/platforms/**",
            "projects/nyar.ts/emulators/**",
        ],
        excludes: [
            "**/node_modules/**",
            "**/dist/**",
            "**/target/**",
            "**/pnpm-lock.yaml",
            "**/*.md",
            "projects/nyar.ts/platforms/**/lib/**",
        ],
    },
    trust: {
        npm: {
            repo: "nyar-vm/nyar-vm",
            file: "publish-npm.yml",
            environment: "NPM_PUBLISH",
        },
    },
    publish: {
        packages: [
            "@nyar/nyar-win32-x64",
            "@nyar/nyar-linux-x64",
            "@nyar/nyar-darwin-x64",
            "@nyar/nyar-darwin-arm64",
            "@nyar/nyar-unknown-wasm32",
            "@nyar/nyar",
            "@nyar/valkyrie-win32-x64",
            "@nyar/valkyrie-linux-x64",
            "@nyar/valkyrie-darwin-x64",
            "@nyar/valkyrie-darwin-arm64",
            "@nyar/valkyrie",
        ],
    },
});
