import {defineConfig} from 'vite';
import {resolve} from 'path';

export default defineConfig({
    build: {
        lib: {
            entry: resolve(__dirname, 'src/index.ts'),
            name: 'NyarMir',
            formats: ['es', 'cjs'],
            fileName: (format) => `nyar-mir.${format}.js`
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