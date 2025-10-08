import { SourceFile } from './source-file.js';

/**
 * Represents a span of text in a source file
 */
export class SourceSpan {
  constructor(
    public readonly start: number,
    public readonly end: number,
    public readonly sourceFile: SourceFile
  ) {
    if (start < 0 || end < start) {
      throw new Error(`Invalid source span: start=${start}, end=${end}`);
    }
  }

  /**
   * Get the text content of this span
   */
  get text(): string {
    return this.sourceFile.substring(this.start, this.end);
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
  get startPosition(): { line: number; column: number } {
    return this.sourceFile.offsetToPosition(this.start);
  }

  /**
   * Get the end position (line and column)
   */
  get endPosition(): { line: number; column: number } {
    return this.sourceFile.offsetToPosition(this.end);
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
    if (this.sourceFile !== other.sourceFile) {
      throw new Error('Cannot merge spans from different source files');
    }
    
    const newStart = Math.min(this.start, other.start);
    const newEnd = Math.max(this.end, other.end);
    
    return new SourceSpan(newStart, newEnd, this.sourceFile);
  }

  /**
   * Create a span from line and column positions
   */
  static fromLineColumn(
    startLine: number,
    startColumn: number,
    endLine: number,
    endColumn: number,
    sourceFile: SourceFile
  ): SourceSpan {
    const start = sourceFile.positionToOffset(startLine, startColumn);
    const end = sourceFile.positionToOffset(endLine, endColumn);
    return new SourceSpan(start, end, sourceFile);
  }
}