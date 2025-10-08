import { describe, it, expect } from 'vitest';
import { SourceFile } from '../src/source-file.js';

describe('SourceFile', () => {
    const create_test_file = () => new SourceFile('test.ts', 'line 1\nline 2\nline 3');

    it('should create a source file', () => {
        const file = create_test_file();

        expect(file.filename).toBe('test.ts');
        expect(file.content).toBe('line 1\nline 2\nline 3');
        expect(file.language_id).toBe('typescript');
    });

    it('should get line by number', () => {
        const file = create_test_file();

        expect(file.get_line(1)).toBe('line 1');
        expect(file.get_line(2)).toBe('line 2');
        expect(file.get_line(3)).toBe('line 3');
        expect(file.get_line(4)).toBe('');
    });

    it('should get line count', () => {
        const file = create_test_file();

        expect(file.line_count).toBe(3);
    });

    it('should convert offset to position', () => {
        const file = create_test_file();

        expect(file.offset_to_position(0)).toEqual({ line: 1, column: 1 });
        expect(file.offset_to_position(7)).toEqual({ line: 2, column: 1 });
        expect(file.offset_to_position(14)).toEqual({ line: 3, column: 1 });
    });

    it('should convert position to offset', () => {
        const file = create_test_file();

        expect(file.position_to_offset(1, 1)).toBe(0);
        expect(file.position_to_offset(2, 1)).toBe(7);
        expect(file.position_to_offset(3, 1)).toBe(14);
    });
});
