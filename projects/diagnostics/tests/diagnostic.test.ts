import {describe, expect, it} from 'vitest';
import {Diagnostic, DiagnosticSeverity} from '../src/diagnostic.js';
import {SourceFile} from '../src/source-file.js';
import {SourceSpan} from '../src/source-span.js';

describe('Diagnostic', () => {
    const createTestFile = () => new SourceFile('test.ts', 'line 1\nline 2\nline 3');
    const createTestSpan = () => new SourceSpan(0, 6, createTestFile());

    it('should create an error diagnostic', () => {
        const diagnostic = Diagnostic.error(
            'E001',
            'Test Error',
            'This is a test error',
            createTestSpan(),
            'Try fixing this'
        );

        expect(diagnostic.severity).toBe(DiagnosticSeverity.Error);
        expect(diagnostic.code).toBe('E001');
        expect(diagnostic.title).toBe('Test Error');
        expect(diagnostic.primaryMessage).toBe('This is a test error');
        expect(diagnostic.primarySuggestion).toBe('Try fixing this');
    });

    it('should create a warning diagnostic', () => {
        const diagnostic = Diagnostic.warning(
            'W001',
            'Test Warning',
            'This is a test warning'
        );

        expect(diagnostic.severity).toBe(DiagnosticSeverity.Warning);
        expect(diagnostic.code).toBe('W001');
        expect(diagnostic.title).toBe('Test Warning');
        expect(diagnostic.primaryMessage).toBe('This is a test warning');
    });

    it('should create an info diagnostic', () => {
        const diagnostic = Diagnostic.info(
            'I001',
            'Test Info',
            'This is test info'
        );

        expect(diagnostic.severity).toBe(DiagnosticSeverity.Info);
        expect(diagnostic.code).toBe('I001');
        expect(diagnostic.title).toBe('Test Info');
        expect(diagnostic.primaryMessage).toBe('This is test info');
    });

    it('should create a hint diagnostic', () => {
        const diagnostic = Diagnostic.hint(
            'H001',
            'Test Hint',
            'This is a test hint'
        );

        expect(diagnostic.severity).toBe(DiagnosticSeverity.Hint);
        expect(diagnostic.code).toBe('H001');
        expect(diagnostic.title).toBe('Test Hint');
        expect(diagnostic.primaryMessage).toBe('This is a test hint');
    });

    it('should add related diagnostics', () => {
        const main = Diagnostic.error('E001', 'Main Error', 'Main error message');
        const related = Diagnostic.info('I001', 'Related Info', 'Related information');

        const withRelated = main.withRelated(related);
        expect(withRelated.related).toContain(related);
    });

    it('should add multiple related diagnostics', () => {
        const main = Diagnostic.error('E001', 'Main Error', 'Main error message');
        const related1 = Diagnostic.info('I001', 'Related Info', 'Related information');
        const related2 = Diagnostic.hint('H001', 'Related Hint', 'Related hint');

        const withRelated = main.withRelatedMany([related1, related2]);
        expect(withRelated.related).toContain(related1);
        expect(withRelated.related).toContain(related2);
    });

    it('should add additional messages', () => {
        const diagnostic = Diagnostic.error('E001', 'Main Error', 'Main error message');
        const withMessage = diagnostic.withMessage('Additional context', 'Fix suggestion');

        expect(withMessage.messages).toHaveLength(2);
        expect(withMessage.messages[1]?.message).toBe('Additional context');
        expect(withMessage.messages[1]?.suggestion).toBe('Fix suggestion');
    });
});