import {defineConfig} from 'vite';
import {resolve} from 'path';

export default defineConfig({
    resolve: {
        alias: {
            '@': resolve(__dirname, './src'),
        },
    },
    build: {
        lib: {
            entry: resolve(__dirname, 'src/index.ts'),
            name: 'NyarLir',
            formats: ['es', 'cjs'],
            fileName: (format) => `index.${format === 'es' ? 'js' : 'cjs'}`
        },
        rollupOptions: {
            external: ['@nyar/diagnostics'],
            output: {
                globals: {
                    '@nyar/diagnostics': 'NyarDiagnostics'
                }
            }
        },
        sourcemap: true
    }
});