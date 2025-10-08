import { describe, it, expect } from 'vitest';
import { SourceFile } from '../src/source-file.js';

describe('SourceFile', () => {
    it('should create a source file', () => {
        const content = 'line 1\nline 2\nline 3';
        const file = new SourceFile('test.ts', content);

        expect(file.filename).toBe('test.ts');
        expect(file.content).toBe(content);
        expect(file.language_id).toBe('typescript');
    });

    it('should get line by number', () => {
        const content = 'line 1\nline 2\nline 3';
        const file = new SourceFile('test.ts', content);

        expect(file.get_line(1)).toBe('line 1');
        expect(file.get_line(2)).toBe('line 2');
        expect(file.get_line(3)).toBe('line 3');
        expect(file.get_line(4)).toBe('');
    });

    it('should get line count', () => {
        const content = 'line 1\nline 2\nline 3';
        const file = new SourceFile('test.ts', content);

        expect(file.line_count).toBe(3);
    });

    it('should convert offset to position', () => {
        const content = 'line 1\nline 2\nline 3';
        const file = new SourceFile('test.ts', content);

        expect(file.offsetToPosition(0)).toEqual({ line: 1, column: 1 });
        expect(file.offsetToPosition(7)).toEqual({ line: 2, column: 1 });
        expect(file.offsetToPosition(14)).toEqual({ line: 3, column: 1 });
    });

    it('should convert position to offset', () => {
        const content = 'line 1\nline 2\nline 3';
        const file = new SourceFile('test.ts', content);

        expect(file.positionToOffset(1, 1)).toBe(0);
        expect(file.positionToOffset(2, 1)).toBe(7);
        expect(file.positionToOffset(3, 1)).toBe(14);
    });
});
