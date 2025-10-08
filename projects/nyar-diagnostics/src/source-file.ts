/**
 * Represents a source file with its content and metadata
 */
export class SourceFile {
    constructor(
        public readonly filename: string,
        public readonly content: string,
        public readonly language_id: string = 'typescript'
    ) {}

    /**
     * Get a specific line from the source file
     */
    get_line(line_number: number): string {
        const lines = this.content.split('\n');
        return lines[line_number - 1] || '';
    }

    /**
     * Get the total number of lines in the source file
     */
    get line_count(): number {
        return this.content.split('\n').length;
    }

    /**
     * Get a substring from the source file
     */
    substring(start: number, end?: number): string {
        return this.content.substring(start, end);
    }

    /**
     * Convert a character offset to line and column position
     */
    offset_to_position(offset: number): { line: number; column: number } {
        const lines = this.content.substring(0, offset).split('\n');
        const last_line = lines[lines.length - 1];
        return {
            line: lines.length,
            column: last_line ? last_line.length + 1 : 1,
        };
    }

    /**
     * Convert line and column position to character offset
     */
    position_to_offset(line: number, column: number): number {
        const lines = this.content.split('\n');
        let offset = 0;

        // Add up the lengths of all lines before the target line
        for (let i = 0; i < line - 1 && i < lines.length; i++) {
            const line_content = lines[i];
            if (line_content) {
                offset += line_content.length + 1; // +1 for newline
            }
        }

        // Add the column position on the target line
        if (line <= lines.length) {
            const line_content = lines[line - 1];
            offset += Math.min(column - 1, line_content?.length ?? 0);
        }

        return offset;
    }
}
