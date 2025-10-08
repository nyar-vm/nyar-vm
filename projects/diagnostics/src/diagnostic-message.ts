/**
 * Represents a diagnostic message with optional suggestions
 */
export interface DiagnosticMessage {
    message: string;
    suggestion?: string;
    help?: string;
}