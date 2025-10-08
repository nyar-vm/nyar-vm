/**
 * Represents a source file with its content and metadata
 */
export class SourceFile {
  constructor(
    public readonly filename: string,
    public readonly content: string,
    public readonly languageId: string = 'typescript'
  ) {}

  /**
   * Get a specific line from the source file
   */
  getLine(lineNumber: number): string {
    const lines = this.content.split('\n');
    return lines[lineNumber - 1] || '';
  }

  /**
   * Get the total number of lines in the source file
   */
  get lineCount(): number {
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
  offsetToPosition(offset: number): { line: number; column: number } {
    const lines = this.content.substring(0, offset).split('\n');
    return {
      line: lines.length,
      column: lines[lines.length - 1].length + 1,
    };
  }

  /**
   * Convert line and column position to character offset
   */
  positionToOffset(line: number, column: number): number {
    const lines = this.content.split('\n');
    let offset = 0;
    
    for (let i = 1; i < line && i <= lines.length; i++) {
      offset += lines[i - 1].length + 1; // +1 for newline
    }
    
    if (line <= lines.length) {
      offset += Math.min(column - 1, lines[line - 1]?.length ?? 0);
    }
    
    return offset;
  }
}