import { Diagnostic } from './diagnostic.js';

/**
 * Collects and manages diagnostics during compilation
 */
export class DiagnosticCollector {
    private diagnostics: Diagnostic[] = [];
    private _has_errors = false;

    /**
     * Add a diagnostic to the collector
     */
    add(diagnostic: Diagnostic): void {
        this.diagnostics.push(diagnostic);
        if (diagnostic.severity === 'error') {
            this._has_errors = true;
        }
    }

    /**
     * Add multiple diagnostics
     */
    add_many(diagnostics: Diagnostic[]): void {
        for (const diagnostic of diagnostics) {
            this.add(diagnostic);
        }
    }

    /**
     * Check if there are any errors
     */
    has_errors(): boolean {
        return this._has_errors;
    }

    /**
     * Check if there are any diagnostics
     */
    has_diagnostics(): boolean {
        return this.diagnostics.length > 0;
    }

    /**
     * Get all diagnostics
     */
    get_diagnostics(): Diagnostic[] {
        return [...this.diagnostics];
    }

    /**
     * Get only error diagnostics
     */
    get_errors(): Diagnostic[] {
        return this.diagnostics.filter(d => d.severity === 'error');
    }

    /**
     * Get only warning diagnostics
     */
    get_warnings(): Diagnostic[] {
        return this.diagnostics.filter(d => d.severity === 'warning');
    }

    /**
     * Get only info diagnostics
     */
    get_info(): Diagnostic[] {
        return this.diagnostics.filter(d => d.severity === 'info');
    }

    /**
     * Get only hint diagnostics
     */
    get_hints(): Diagnostic[] {
        return this.diagnostics.filter(d => d.severity === 'hint');
    }

    /**
     * Clear all diagnostics
     */
    clear(): void {
        this.diagnostics = [];
        this._has_errors = false;
    }

    /**
     * Get the total number of diagnostics
     */
    get count(): number {
        return this.diagnostics.length;
    }

    /**
     * Get the number of errors
     */
    get error_count(): number {
        return this.get_errors().length;
    }

    /**
     * Get the number of warnings
     */
    get warning_count(): number {
        return this.get_warnings().length;
    }

    /**
     * Throw an error if there are any errors
     */
    throw_if_errors(): void {
        if (this._has_errors) {
            const error_messages = this.get_errors()
                .map(err => `${err.code}: ${err.title} - ${err.primaryMessage}`)
                .join('\n');
            throw new Error(
                `Compilation failed with ${this.error_count} error(s):\n${error_messages}`
            );
        }
    }
}
