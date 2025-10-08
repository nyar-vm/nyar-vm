import { describe, it, expect } from 'vitest';
import { Diagnostic } from '../src/diagnostic.js';
import { DiagnosticCollector } from '../src/diagnostic-collector.js';
import { SourceFile } from '../src/source-file.js';
import { SourceSpan } from '../src/source-span.js';

describe('DiagnosticCollector', () => {
  const createTestDiagnostic = (severity: 'error' | 'warning' | 'info' | 'hint' = 'error') => {
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
    const diagnostic = createTestDiagnostic('error');
    
    collector.add(diagnostic);
    expect(collector.hasErrors()).toBe(true);
    expect(collector.hasDiagnostics()).toBe(true);
    expect(collector.count).toBe(1);
  });

  it('should add multiple diagnostics', () => {
    const collector = new DiagnosticCollector();
    const error1 = createTestDiagnostic('error');
    const error2 = createTestDiagnostic('error');
    const warning = createTestDiagnostic('warning');
    
    collector.addMany([error1, error2, warning]);
    expect(collector.hasErrors()).toBe(true);
    expect(collector.count).toBe(3);
    expect(collector.errorCount).toBe(2);
    expect(collector.warningCount).toBe(1);
  });

  it('should get diagnostics by severity', () => {
    const collector = new DiagnosticCollector();
    const error = createTestDiagnostic('error');
    const warning = createTestDiagnostic('warning');
    const info = createTestDiagnostic('info');
    const hint = createTestDiagnostic('hint');
    
    collector.addMany([error, warning, info, hint]);
    
    expect(collector.getErrors()).toHaveLength(1);
    expect(collector.getWarnings()).toHaveLength(1);
    expect(collector.getInfo()).toHaveLength(1);
    expect(collector.getHints()).toHaveLength(1);
  });

  it('should clear diagnostics', () => {
    const collector = new DiagnosticCollector();
    collector.add(createTestDiagnostic('error'));
    
    expect(collector.hasErrors()).toBe(true);
    
    collector.clear();
    expect(collector.hasErrors()).toBe(false);
    expect(collector.hasDiagnostics()).toBe(false);
    expect(collector.count).toBe(0);
  });

  it('should throw if there are errors', () => {
    const collector = new DiagnosticCollector();
    collector.add(createTestDiagnostic('error'));
    
    expect(() => collector.throwIfErrors()).toThrow('Compilation failed with 1 error(s)');
  });

  it('should not throw if there are only warnings', () => {
    const collector = new DiagnosticCollector();
    collector.add(createTestDiagnostic('warning'));
    
    expect(() => collector.throwIfErrors()).not.toThrow();
  });
});