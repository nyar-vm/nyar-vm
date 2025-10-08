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
    const lastLine = lines[lines.length - 1];
    return {
      line: lines.length,
      column: lastLine ? lastLine.length + 1 : 1,
    };
  }

  /**
   * Convert line and column position to character offset
   */
  positionToOffset(line: number, column: number): number {
    const lines = this.content.split('\n');
    let offset = 0;
    
    // Add up the lengths of all lines before the target line
    for (let i = 0; i < line - 1 && i < lines.length; i++) {
      const lineContent = lines[i];
      if (lineContent) {
        offset += lineContent.length + 1; // +1 for newline
      }
    }
    
    // Add the column position on the target line
    if (line <= lines.length) {
      const lineContent = lines[line - 1];
      offset += Math.min(column - 1, lineContent?.length ?? 0);
    }
    
    return offset;
  }
}