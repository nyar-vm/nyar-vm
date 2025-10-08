import {defineConfig} from 'vite';
import {resolve} from 'path';

export default defineConfig({
    build: {
        lib: {
            entry: resolve(__dirname, 'src/index.ts'),
            name: 'NyarDiagnostics',
            formats: ['es', 'cjs'],
            fileName: (format) => `nyar-diagnostics.${format}.js`,
        },
        rollupOptions: {
            external: [],
            output: {
                globals: {},
            },
        },
        target: 'es2022',
        sourcemap: true,
        minify: false,
    }
});