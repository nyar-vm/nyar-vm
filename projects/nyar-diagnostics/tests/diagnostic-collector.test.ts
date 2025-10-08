import {describe, expect, it} from 'vitest';
import {Diagnostic} from '../src/diagnostic.js';
import {DiagnosticCollector} from '../src/diagnostic-collector.js';
import {SourceFile} from '../src/source-file.js';
import {SourceSpan} from '../src/source-span.js';

describe('DiagnosticCollector', () => {
    const create_test_diagnostic = (severity: 'error' | 'warning' | 'info' | 'hint' = 'error') => {
        const file = new SourceFile('test.ts', 'test content');
        const span = new SourceSpan(0, 4, file);

        switch (severity) {
            case 'error':
                return Diagnostic.error('E001', 'Test Error', 'Test error message', span);
            case 'warning':
                return Diagnostic.warning('W001', 'Test Warning', 'Test warning message', span);
            case 'info':
                return Diagnostic.info('I001', 'Test Info', 'Test info message', span);
            case 'hint':
                return Diagnostic.hint('H001', 'Test Hint', 'Test hint message', span);
        }
    };

    it('should add diagnostics', () => {
        const collector = new DiagnosticCollector();
        const diagnostic = create_test_diagnostic('error');

        collector.add(diagnostic);
        expect(collector.has_errors()).toBe(true);
        expect(collector.has_diagnostics()).toBe(true);
        expect(collector.count).toBe(1);
    });

    it('should add multiple diagnostics', () => {
        const collector = new DiagnosticCollector();
        const error1 = create_test_diagnostic('error');
        const error2 = create_test_diagnostic('error');
        const warning = create_test_diagnostic('warning');

        collector.add_many([error1, error2, warning]);
        expect(collector.has_errors()).toBe(true);
        expect(collector.has_diagnostics()).toBe(true);
        expect(collector.count).toBe(3);
        expect(collector.error_count).toBe(2);
        expect(collector.warning_count).toBe(1);
    });

    it('should get diagnostics by severity', () => {
        const collector = new DiagnosticCollector();
        const error = create_test_diagnostic('error');
        const warning = create_test_diagnostic('warning');
        const info = create_test_diagnostic('info');
        const hint = create_test_diagnostic('hint');

        collector.add_many([error, warning, info, hint]);

        expect(collector.get_errors()).toHaveLength(1);
        expect(collector.get_warnings()).toHaveLength(1);
        expect(collector.get_info()).toHaveLength(1);
        expect(collector.get_hints()).toHaveLength(1);
    });

    it('should clear diagnostics', () => {
        const collector = new DiagnosticCollector();
        collector.add(create_test_diagnostic('error'));

        expect(collector.has_errors()).toBe(true);

        collector.clear();
        expect(collector.has_errors()).toBe(false);
        expect(collector.has_diagnostics()).toBe(false);
        expect(collector.count).toBe(0);
    });

    it('should throw if there are errors', () => {
        const collector = new DiagnosticCollector();
        collector.add(create_test_diagnostic('error'));

        expect(() => collector.throw_if_errors()).toThrow('Compilation failed with 1 error(s)');
    });

    it('should not throw if there are only warnings', () => {
        const collector = new DiagnosticCollector();
        collector.add(create_test_diagnostic('warning'));

        expect(() => collector.throw_if_errors()).not.toThrow();
    });
});
