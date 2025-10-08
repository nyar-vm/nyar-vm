import typescriptEslint from '@typescript-eslint/eslint-plugin';
import typescriptParser from '@typescript-eslint/parser';
import prettier from 'eslint-plugin-prettier';
import prettierConfig from 'eslint-config-prettier';

export default [
    {
        files: ['src/**/*.ts', 'tests/**/*.ts'],
        languageOptions: {
            parser: typescriptParser,
            parserOptions: {
                ecmaVersion: 2022,
                sourceType: 'module',
            },
        },
        plugins: {
            '@typescript-eslint': typescriptEslint,
            prettier: prettier,
        },
        rules: {
            // Prettier integration
            'prettier/prettier': 'error',

            // TypeScript specific rules
            '@typescript-eslint/no-unused-vars': 'error',
            '@typescript-eslint/no-explicit-any': 'warn',
            '@typescript-eslint/explicit-function-return-type': 'off',
            '@typescript-eslint/explicit-module-boundary-types': 'off',

            // Naming conventions - snake_case for functions and variables, camelCase allowed for existing code
            '@typescript-eslint/naming-convention': [
                'error', // 先使用 warn 级别，避免阻塞构建
                {
                    selector: 'function',
                    format: ['snake_case', 'camelCase'], // 同时允许 snake_case 和 camelCase
                },
                {
                    selector: 'variable',
                    format: ['snake_case', 'camelCase'],
                },
                {
                    selector: 'parameter',
                    format: ['snake_case', 'camelCase'],
                },
                {
                    selector: 'method',
                    format: ['snake_case', 'camelCase'],
                },
                {
                    selector: 'property',
                    format: ['snake_case', 'camelCase'],
                },
                {
                    selector: 'class',
                    format: ['PascalCase'],
                },
                {
                    selector: 'interface',
                    format: ['PascalCase'],
                },
                {
                    selector: 'typeAlias',
                    format: ['PascalCase'],
                },
                {
                    selector: 'enum',
                    format: ['PascalCase'],
                },
            ],

            // General best practices
            'no-console': 'warn',
            'prefer-const': 'error',
            'no-var': 'error',
        },
    },
    {
        files: ['*.js', '*.ts'],
        rules: {
            ...prettierConfig.rules,
        },
    },
];