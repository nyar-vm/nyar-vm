import { SourceFile } from './source-file.js';

/**
 * Represents a span of text in a source file
 */
export class SourceSpan {
    constructor(
        public readonly start: number,
        public readonly end: number,
        public readonly source_file: SourceFile
    ) {
        if (start < 0 || end < start) {
            throw new Error(`Invalid source span: start=${start}, end=${end}`);
        }
    }

    /**
     * Get the text content of this span
     */
    get text(): string {
        return this.source_file.substring(this.start, this.end);
    }

    /**
     * Get the length of this span
     */
    get length(): number {
        return this.end - this.start;
    }

    /**
     * Get the start position (line and column)
     */
    get start_position(): { line: number; column: number } {
        return this.source_file.offsetToPosition(this.start);
    }

    /**
     * Get the end position (line and column)
     */
    get end_position(): { line: number; column: number } {
        return this.source_file.offsetToPosition(this.end);
    }

    /**
     * Check if this span contains another span
     */
    contains(other: SourceSpan): boolean {
        return this.start <= other.start && this.end >= other.end;
    }

    /**
     * Check if this span overlaps with another span
     */
    overlaps(other: SourceSpan): boolean {
        return this.start < other.end && this.end > other.start;
    }

    /**
     * Merge two spans into a single span that covers both
     */
    merge(other: SourceSpan): SourceSpan {
        if (this.source_file !== other.source_file) {
            throw new Error('Cannot merge spans from different source files');
        }

        const newStart = Math.min(this.start, other.start);
        const newEnd = Math.max(this.end, other.end);

        return new SourceSpan(newStart, newEnd, this.source_file);
    }

    /**
     * Create a span from line and column positions
     */
    static from_line_column(
        start_line: number,
        start_column: number,
        end_line: number,
        end_column: number,
        source_file: SourceFile
    ): SourceSpan {
        const start = source_file.positionToOffset(start_line, start_column);
        const end = source_file.positionToOffset(end_line, end_column);
        return new SourceSpan(start, end, source_file);
    }
}
