import { Diagnostic } from './diagnostic.js';
import { SourceSpan } from './source-span.js';
import { DiagnosticSeverity } from './diagnostic-severity';

/**
 * Formats diagnostics for display
 */
export class Formatter {
    private readonly useColors: boolean;

    constructor(useColors = true) {
        this.useColors = useColors && this.supportsColor();
    }

    /**
     * Format a single diagnostic
     */
    format(diagnostic: Diagnostic): string {
        const lines: string[] = [];

        // Header with severity and code
        const severityColor = this.getSeverityColor(diagnostic.severity);
        const header = `${severityColor}${this.formatSeverity(diagnostic.severity)}${this.resetColor()} ${diagnostic.code}: ${diagnostic.title}`;
        lines.push(header);

        // Primary message and location
        if (diagnostic.span) {
            lines.push('');
            lines.push(this.formatLocation(diagnostic.span));
            lines.push('');
            lines.push(this.formatSourceSnippet(diagnostic.span));
        }

        // Additional messages
        for (let i = 1; i < diagnostic.messages.length; i++) {
            const msg = diagnostic.messages[i];
            if (msg) {
                lines.push(`  ${msg.message}`);
                if (msg.suggestion) {
                    lines.push(`  ${this.dimColor()}help: ${msg.suggestion}${this.resetColor()}`);
                }
                if (msg.help) {
                    lines.push(`  ${this.dimColor()}note: ${msg.help}${this.resetColor()}`);
                }
            }
        }

        // Primary suggestion and help
        if (diagnostic.primarySuggestion) {
            lines.push(
                `  ${this.dimColor()}help: ${diagnostic.primarySuggestion}${this.resetColor()}`
            );
        }
        if (diagnostic.help) {
            lines.push(`  ${this.dimColor()}note: ${diagnostic.help}${this.resetColor()}`);
        }

        // Related diagnostics
        if (diagnostic.related.length > 0) {
            lines.push('');
            lines.push('  Related diagnostics:');
            for (const related of diagnostic.related) {
                const relatedLines = this.format(related).split('\n');
                for (const line of relatedLines) {
                    lines.push(`  ${line}`);
                }
            }
        }

        return lines.join('\n');
    }

    /**
     * Format multiple diagnostics
     */
    formatMany(diagnostics: Diagnostic[]): string {
        return diagnostics.map(d => this.format(d)).join('\n\n');
    }

    private formatSeverity(severity: DiagnosticSeverity): string {
        switch (severity) {
            case DiagnosticSeverity.Error:
                return 'error';
            case DiagnosticSeverity.Warning:
                return 'warning';
            case DiagnosticSeverity.Info:
                return 'info';
            case DiagnosticSeverity.Hint:
                return 'hint';
        }
    }

    private formatLocation(span: SourceSpan): string {
        const start = span.start_position;
        const end = span.end_position;
        const filename = span.source_file.filename;

        if (start.line === end.line) {
            return `  --> ${filename}:${start.line}:${start.column}`;
        } else {
            return `  --> ${filename}:${start.line}:${start.column}-${end.line}:${end.column}`;
        }
    }

    private formatSourceSnippet(span: SourceSpan): string {
        const start = span.start_position;
        const end = span.end_position;
        const sourceFile = span.source_file;

        // Get relevant lines
        const lines = sourceFile.content.split('\n');
        const startLine = Math.max(0, start.line - 2);
        const endLine = Math.min(lines.length, end.line + 1);

        const snippetLines: string[] = [];
        const gutterWidth = String(endLine).length;

        for (let i = startLine; i < endLine; i++) {
            const lineNumber = i + 1;
            const line = lines[i] ?? '';
            const gutter = String(lineNumber).padStart(gutterWidth);

            snippetLines.push(`${this.dimColor()}${gutter} |${this.resetColor()} ${line}`);

            // Add underline for the error line
            if (i + 1 === start.line) {
                const underline =
                    this.getSeverityColor(DiagnosticSeverity.Error) +
                    '^'.repeat(Math.min(span.length, line.length)) +
                    this.resetColor();
                snippetLines.push(
                    `${' '.repeat(gutterWidth)} | ${' '.repeat(start.column - 1)}${underline}`
                );
            }
        }

        return snippetLines.join('\n');
    }

    private getSeverityColor(severity: DiagnosticSeverity): string {
        if (!this.useColors) return '';

        switch (severity) {
            case DiagnosticSeverity.Error:
                return '\x1b[31m'; // Red
            case DiagnosticSeverity.Warning:
                return '\x1b[33m'; // Yellow
            case DiagnosticSeverity.Info:
                return '\x1b[34m'; // Blue
            case DiagnosticSeverity.Hint:
                return '\x1b[90m'; // Gray
        }
    }

    private dimColor(): string {
        return this.useColors ? '\x1b[90m' : '';
    }

    private resetColor(): string {
        return this.useColors ? '\x1b[0m' : '';
    }

    private supportsColor(): boolean {
        // Simple check for color support
        return (
            typeof process !== 'undefined' &&
            process.stdout &&
            process.stdout.isTTY &&
            process.env['COLORS'] !== 'false'
        );
    }
}
