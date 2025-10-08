import {Diagnostic} from './diagnostic.js';

/**
 * Collects and manages diagnostics during compilation
 */
export class DiagnosticCollector {
    private diagnostics: Diagnostic[] = [];
    private _hasErrors = false;

    /**
     * Add a diagnostic to the collector
     */
    add(diagnostic: Diagnostic): void {
        this.diagnostics.push(diagnostic);
        if (diagnostic.severity === 'error') {
            this._hasErrors = true;
        }
    }

    /**
     * Add multiple diagnostics
     */
    addMany(diagnostics: Diagnostic[]): void {
        for (const diagnostic of diagnostics) {
            this.add(diagnostic);
        }
    }

    /**
     * Check if there are any errors
     */
    hasErrors(): boolean {
        return this._hasErrors;
    }

    /**
     * Check if there are any diagnostics
     */
    hasDiagnostics(): boolean {
        return this.diagnostics.length > 0;
    }

    /**
     * Get all diagnostics
     */
    getDiagnostics(): Diagnostic[] {
        return [...this.diagnostics];
    }

    /**
     * Get only error diagnostics
     */
    getErrors(): Diagnostic[] {
        return this.diagnostics.filter(d => d.severity === 'error');
    }

    /**
     * Get only warning diagnostics
     */
    getWarnings(): Diagnostic[] {
        return this.diagnostics.filter(d => d.severity === 'warning');
    }

    /**
     * Get only info diagnostics
     */
    getInfo(): Diagnostic[] {
        return this.diagnostics.filter(d => d.severity === 'info');
    }

    /**
     * Get only hint diagnostics
     */
    getHints(): Diagnostic[] {
        return this.diagnostics.filter(d => d.severity === 'hint');
    }

    /**
     * Clear all diagnostics
     */
    clear(): void {
        this.diagnostics = [];
        this._hasErrors = false;
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
    get errorCount(): number {
        return this.getErrors().length;
    }

    /**
     * Get the number of warnings
     */
    get warningCount(): number {
        return this.getWarnings().length;
    }

    /**
     * Throw an error if there are any errors
     */
    throwIfErrors(): void {
        if (this._hasErrors) {
            const errorMessages = this.getErrors()
                .map(err => `${err.code}: ${err.title} - ${err.primaryMessage}`)
                .join('\n');
            throw new Error(
                `Compilation failed with ${this.errorCount} error(s):\n${errorMessages}`
            );
        }
    }
}
