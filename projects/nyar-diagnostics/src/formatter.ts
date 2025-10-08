import { Diagnostic } from './diagnostic.js';
import { SourceSpan } from './source-span.js';
import { DiagnosticSeverity } from './diagnostic-severity';

/**
 * Formats diagnostics for display
 */
export class Formatter {
    private readonly use_colors: boolean;

    constructor(use_colors = true) {
        this.use_colors = use_colors && this.supports_color();
    }

    /**
     * Format a single diagnostic
     */
    format(diagnostic: Diagnostic): string {
        const lines: string[] = [];

        // Header with severity and code
        const severity_color = this.get_severity_color(diagnostic.severity);
        const header = `${severity_color}${this.format_severity(diagnostic.severity)}${this.reset_color()} ${diagnostic.code}: ${diagnostic.title}`;
        lines.push(header);

        // Primary message and location
        if (diagnostic.span) {
            lines.push('');
            lines.push(this.format_location(diagnostic.span));
            lines.push('');
            lines.push(this.format_source_snippet(diagnostic.span));
        }

        // Additional messages
        for (let i = 1; i < diagnostic.messages.length; i++) {
            const msg = diagnostic.messages[i];
            if (msg) {
                lines.push(`  ${msg.message}`);
                if (msg.suggestion) {
                    lines.push(`  ${this.dim_color()}help: ${msg.suggestion}${this.reset_color()}`);
                }
                if (msg.help) {
                    lines.push(`  ${this.dim_color()}note: ${msg.help}${this.reset_color()}`);
                }
            }
        }

        // Primary suggestion and help
        if (diagnostic.primarySuggestion) {
            lines.push(
                `  ${this.dim_color()}help: ${diagnostic.primarySuggestion}${this.reset_color()}`
            );
        }
        if (diagnostic.help) {
            lines.push(`  ${this.dim_color()}note: ${diagnostic.help}${this.reset_color()}`);
        }

        // Related diagnostics
        if (diagnostic.related.length > 0) {
            lines.push('');
            lines.push('  Related diagnostics:');
            for (const related of diagnostic.related) {
                const related_lines = this.format(related).split('\n');
                for (const line of related_lines) {
                    lines.push(`  ${line}`);
                }
            }
        }

        return lines.join('\n');
    }

    /**
     * Format multiple diagnostics
     */
    format_many(diagnostics: Diagnostic[]): string {
        return diagnostics.map(d => this.format(d)).join('\n\n');
    }

    private format_severity(severity: DiagnosticSeverity): string {
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

    private format_location(span: SourceSpan): string {
        const start = span.start_position;
        const end = span.end_position;
        const filename = span.source_file.filename;

        if (start.line === end.line) {
            return `  --> ${filename}:${start.line}:${start.column}`;
        } else {
            return `  --> ${filename}:${start.line}:${start.column}-${end.line}:${end.column}`;
        }
    }

    private format_source_snippet(span: SourceSpan): string {
        const start = span.start_position;
        const end = span.end_position;
        const source_file = span.source_file;

        // Get relevant lines
        const lines = source_file.content.split('\n');
        const start_line = Math.max(0, start.line - 2);
        const end_line = Math.min(lines.length, end.line + 1);

        const snippet_lines: string[] = [];
        const gutter_width = String(end_line).length;

        for (let i = start_line; i < end_line; i++) {
            const line_number = i + 1;
            const line = lines[i] ?? '';
            const gutter = String(line_number).padStart(gutter_width);

            snippet_lines.push(`${this.dim_color()}${gutter} |${this.reset_color()} ${line}`);

            // Add underline for the error line
            if (i + 1 === start.line) {
                const underline =
                    this.get_severity_color(DiagnosticSeverity.Error) +
                    '^'.repeat(Math.min(span.length, line.length)) +
                    this.reset_color();
                snippet_lines.push(
                    `${' '.repeat(gutter_width)} | ${' '.repeat(start.column - 1)}${underline}`
                );
            }
        }

        return snippet_lines.join('\n');
    }

    private get_severity_color(severity: DiagnosticSeverity): string {
        if (!this.use_colors) return '';

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

    private dim_color(): string {
        return this.use_colors ? '\x1b[90m' : '';
    }

    private reset_color(): string {
        return this.use_colors ? '\x1b[0m' : '';
    }

    private supports_color(): boolean {
        // Simple check for color support
        return (
            typeof process !== 'undefined' &&
            process.stdout &&
            process.stdout.isTTY &&
            process.env['COLORS'] !== 'false'
        );
    }
}
