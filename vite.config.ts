import {defineConfig} from 'vite';
import {resolve} from 'path';

export default defineConfig({
    build: {
        lib: {
            entry: resolve(__dirname, 'src/index.ts'),
            name: 'NyarFramework',
            formats: ['es', 'cjs'],
            fileName: (format) => `nyar-framework.${format}.js`,
        },
        rollupOptions: {
            external: ['node:fs', 'node:path', 'node:process'],
            output: {
                globals: {},
            },
        },
        target: 'es2022',
        sourcemap: true,
        minify: 'terser',
    },
    test: {
        globals: true,
        environment: 'node',
        include: ['tests/**/*.{test,spec}.{js,mjs,cjs,ts,mts,cts,jsx,tsx}'],
        exclude: ['node_modules', 'dist', 'coverage'],
        coverage: {
            reporter: ['text', 'json', 'html'],
            exclude: [
                'coverage/**',
                'dist/**',
                '**/[.]**',
                'packages/*/test?(s)/**',
                '**/*.d.ts',
                '**/virtual:*',
                '**/__x00__*',
                '**/\x00*',
                'cypress/**',
                'test?(s)/**',
                'test?(-*).?(c|m)[jt]s?(x)',
                '**/*{.,-}{test,spec}.?(c|m)[jt]s?(x)',
                '**/__tests__/**',
                '**/{karma,rollup,webpack,vite,vitest,jest,ava,babel,nyc,cypress,tsup,build}.config.*',
                '**/vitest.{workspace,projects}.[jt]s?(on)',
                '**/.{eslint,mocha,prettier}rc.{?(c|m)js,yml}',
            ],
        },
    },
    esbuild: {
        target: 'es2022',
        format: 'esm',
    },
    optimizeDeps: {
        include: ['@nyar/*'],
    },
});