import { describe, it, expect } from 'vitest';
import { SourceFile } from '../src/source-file.js';
import { SourceSpan } from '../src/source-span.js';

describe('SourceSpan', () => {
  const createTestFile = () => new SourceFile('test.ts', 'line 1\nline 2\nline 3');

  it('should create a valid source span', () => {
    const file = createTestFile();
    const span = new SourceSpan(0, 6, file);

    expect(span.start).toBe(0);
    expect(span.end).toBe(6);
    expect(span.length).toBe(6);
    expect(span.text).toBe('line 1');
  });

  it('should throw on invalid span', () => {
    const file = createTestFile();

    expect(() => new SourceSpan(-1, 6, file)).toThrow('Invalid source span');
    expect(() => new SourceSpan(6, 0, file)).toThrow('Invalid source span');
  });

  it('should get start and end positions', () => {
    const file = createTestFile();
    const span = new SourceSpan(0, 6, file);

    expect(span.start_position).toEqual({ line: 1, column: 1 });
    expect(span.end_position).toEqual({ line: 2, column: 1 });
  });

  it('should check if span contains another span', () => {
    const file = createTestFile();
    const outer = new SourceSpan(0, 12, file);
    const inner = new SourceSpan(6, 12, file);

    expect(outer.contains(inner)).toBe(true);
    expect(inner.contains(outer)).toBe(false);
  });

  it('should check if spans overlap', () => {
    const file = createTestFile();
    const span1 = new SourceSpan(0, 10, file);
    const span2 = new SourceSpan(5, 15, file);
    const span3 = new SourceSpan(15, 20, file);

    expect(span1.overlaps(span2)).toBe(true);
    expect(span1.overlaps(span3)).toBe(false);
  });

  it('should merge spans', () => {
    const file = createTestFile();
    const span1 = new SourceSpan(0, 6, file);
    const span2 = new SourceSpan(6, 12, file);

    const merged = span1.merge(span2);
    expect(merged.start).toBe(0);
    expect(merged.end).toBe(12);
    expect(merged.text).toBe('line 1\nline 2');
  });

  it('should create span from line and column', () => {
    const file = createTestFile();
    const span = SourceSpan.from_line_column(1, 1, 2, 1, file);

    expect(span.start).toBe(0);
    expect(span.end).toBe(6);
    expect(span.text).toBe('line 1');
  });
});
